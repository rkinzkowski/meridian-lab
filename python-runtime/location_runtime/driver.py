"""Version-audited pymobiledevice3 adapter; no proprietary reference implementation.

All operations target an explicit UDID. One process owns one userspace tunnel,
one DVT provider and one location channel. Only session teardown closes them.
"""

import asyncio
import contextlib
import hashlib
import importlib.metadata
import json
import os
import platform
import sys
import uuid
from contextlib import AsyncExitStack
from pathlib import Path

from . import PMD_VERSION, __version__
from .diagnostics import system_snapshot
from .errors import RuntimeFault, classify, fault
from .protocol import usable_address, validate_point, validate_udid


def redacted(udid: str) -> str:
    return "device-" + hashlib.sha256(udid.encode()).hexdigest()[:8]


class PmdDriver:
    def __init__(self, data_dir: Path | None = None):
        base = Path(os.environ.get("LOCALAPPDATA", Path.home() / ".local" / "share"))
        self.data_dir = data_dir or base / "MeridianLab" / "runtime"
        self.stack: AsyncExitStack | None = None
        self.rsd = self.dvt = self.loc = self.info_service = None
        self.udid: str | None = None
        self.transport: str | None = None
        self.session_id: str | None = None
        self.session_metadata: dict = {}
        self.last_error: RuntimeFault | None = None
        self.last_failure_classes: list[str] = []
        self.dirty = False
        self.cleanup_errors: list[str] = []

    def info(self):
        try:
            version = importlib.metadata.version("pymobiledevice3")
        except importlib.metadata.PackageNotFoundError:
            version = "unavailable"
        return {
            "version": __version__,
            "pythonVersion": platform.python_version(),
            "pymobiledevice3Version": version,
            "physicalDeviceVerified": False,
        }

    def check_dependency(self):
        if self.info()["pymobiledevice3Version"] != PMD_VERSION:
            raise fault("DEPENDENCY_MISMATCH")

    async def _close_resource(self, callback):
        try:
            async with asyncio.timeout(3):
                await callback()
        except Exception as exc:
            self.cleanup_errors.append(type(exc).__name__)

    def _own(self, stack: AsyncExitStack, callback):
        stack.push_async_callback(self._close_resource, callback)

    async def list_devices(self):
        self.check_dependency()
        from pymobiledevice3.lockdown import create_using_usbmux
        from pymobiledevice3.usbmux import list_devices

        try:
            async with asyncio.timeout(6):
                mux_devices = await list_devices()
        except Exception as exc:
            raise fault("USBMUX_UNAVAILABLE") from exc
        devices = []
        for device in mux_devices[:16]:
            row = {
                "id": device.serial,
                "name": "iPhone",
                "productVersion": "unknown",
                "connectionType": device.connection_type,
            }
            client = None
            try:
                async with asyncio.timeout(3):
                    client = await create_using_usbmux(
                        serial=device.serial, connection_type=device.connection_type, autopair=False
                    )
                    if client.udid != device.serial:
                        raise fault("DEVICE_IDENTITY_MISMATCH")
                    row.update(
                        name=client.all_values.get("DeviceName") or "iPhone",
                        productVersion=client.product_version,
                        productType=client.product_type,
                        lockdownTrusted=client.paired,
                    )
            except Exception as exc:
                row["error"] = classify(exc).wire()
            finally:
                if client:
                    await self._close_resource(client.close)
            devices.append(row)
        return {"devices": devices, "usbmuxReachable": True}

    async def _lockdown(self, udid: str, *, trusted=True):
        from pymobiledevice3.lockdown import create_using_usbmux

        validate_udid(udid)
        client = await create_using_usbmux(
            serial=udid, connection_type="USB", autopair=False, label="Meridian Lab"
        )
        if client.udid != udid:
            await self._close_resource(client.close)
            raise fault("DEVICE_IDENTITY_MISMATCH")
        if trusted and not client.paired:
            stale = client.pair_record is not None
            await self._close_resource(client.close)
            raise fault("STALE_PAIRING_RECORD" if stale else "TRUST_REQUIRED")
        return client

    async def trust(self, udid: str):
        self.check_dependency()
        if self.stack:
            raise fault("SESSION_ALREADY_OPEN")
        client = await self._lockdown(udid, trusted=False)
        try:
            if not client.paired:
                # Mint one new legitimate host identity, even for an existing rejected record.
                # Do not delete unrelated Apple pair records or loop on denied Trust.
                client.host_id = str(uuid.uuid4()).upper()
                client.pair_record = None
                await client.pair(timeout=30)
                if not await client.validate_pairing():
                    raise fault("STALE_PAIRING_RECORD")
            return {"lockdownTrusted": True}
        finally:
            await self._close_resource(client.close)

    async def open(self, udid: str, transport: str, rsd_address=None, rsd_port=None):
        self.check_dependency()
        validate_udid(udid)
        if self.stack is not None:
            if self.udid == udid and self.transport == transport:
                await self.health()
                return self.session_metadata
            raise fault("SESSION_ALREADY_OPEN")
        from pymobiledevice3.remote.remote_service_discovery import RemoteServiceDiscoveryService
        from pymobiledevice3.services.dvt.instruments.device_info import DeviceInfo
        from pymobiledevice3.services.dvt.instruments.dvt_provider import DvtProvider
        from pymobiledevice3.services.dvt.instruments.location_simulation import LocationSimulation

        stack = AsyncExitStack()
        stage = "rsd"
        self.cleanup_errors.clear()
        try:
            if transport == "usb":
                client = await self._lockdown(udid)
                try:
                    version = tuple(int(p) for p in client.product_version.split(".")[:2])
                    if version < (17, 4):
                        raise fault("LEGACY_TUNNEL_REQUIRED")
                    developer_mode = None
                    with contextlib.suppress(Exception):
                        developer_mode = await client.get_developer_mode_status()
                    if developer_mode is False:
                        raise fault("DEVELOPER_MODE_DISABLED")
                finally:
                    await self._close_resource(client.close)
                # Re-enumerate immediately before bootstrap; do not wait through a tunnel
                # timeout if the selected phone has left the USB bus.
                from pymobiledevice3.usbmux import list_devices

                present = await list_devices()
                if not any(d.serial == udid and d.connection_type == "USB" for d in present):
                    raise fault("DEVICE_DISCONNECTED")
                if sys.platform == "darwin":
                    from pymobiledevice3.remote.native_tunnel import NativeRemotedTunnel

                    tunnel = NativeRemotedTunnel(serial=udid)
                else:
                    from pymobiledevice3.remote.userspace_tunnel import UserspaceRsdTunnel

                    tunnel = UserspaceRsdTunnel(
                        serial=udid, autopair=False, remotepairing_fallback=False
                    )
                self._own(stack, tunnel.aclose)
                rsd = await tunnel.aopen()
            elif transport == "wifi":
                if sys.version_info < (3, 13):
                    from pymobiledevice3.remote.tunnel_service import SSLPSKContext

                    if SSLPSKContext is None:
                        raise fault("WIRELESS_TLS_UNAVAILABLE")
                from .wireless import WirelessTunnel

                remote_id = self._pairing_ids().get(redacted(udid))
                if not remote_id:
                    raise fault("WIRELESS_NOT_PREPARED")
                tunnel = WirelessTunnel(remote_id)
                self._own(stack, tunnel.close)
                rsd = await tunnel.open()
            elif transport == "rsd":
                if (
                    not isinstance(rsd_address, str)
                    or not usable_address(rsd_address, allow_loopback=True)
                    or type(rsd_port) is not int
                    or not 1 <= rsd_port <= 65535
                ):
                    raise fault("INVALID_REQUEST")
                rsd = RemoteServiceDiscoveryService((rsd_address, rsd_port))
                self._own(stack, rsd.close)
                await rsd.connect()
            else:
                raise fault("INVALID_REQUEST")
            if rsd.udid != udid:
                raise fault("DEVICE_IDENTITY_MISMATCH")
            stage = "dvt"
            dvt = DvtProvider(rsd)
            self._own(stack, lambda: dvt.__aexit__(None, None, None))
            await dvt.__aenter__()
            loc = LocationSimulation(dvt)
            self._own(stack, lambda: loc.__aexit__(None, None, None))
            await loc.__aenter__()
            info_service = DeviceInfo(dvt)
            self._own(stack, lambda: info_service.__aexit__(None, None, None))
            await info_service.__aenter__()
            await info_service.mach_time_info()
            self.stack, self.rsd, self.dvt, self.loc, self.info_service = (
                stack,
                rsd,
                dvt,
                loc,
                info_service,
            )
            self.udid, self.transport, self.session_id = udid, transport, str(uuid.uuid4())
            self.session_metadata = {
                "deviceIdRedacted": redacted(udid),
                "deviceName": rsd.all_values.get("DeviceName") or "iPhone",
                "productVersion": rsd.product_version,
                "transport": transport,
                "developerServicesReady": True,
                "sessionId": self.session_id,
            }
            self.last_error = None
            return self.session_metadata
        except BaseException as exc:
            await stack.aclose()
            if isinstance(exc, (asyncio.CancelledError, KeyboardInterrupt)):
                raise
            self.last_error = classify(exc, stage)
            raise self.last_error from exc

    async def health(self):
        if self.loc is None or self.info_service is None:
            raise fault("LOCATION_SESSION_FAILED")
        async with asyncio.timeout(4):
            await self.info_service.mach_time_info()
        return {"healthy": True, "sessionId": self.session_id}

    async def set(self, latitude, longitude):
        latitude, longitude = validate_point(latitude, longitude)
        if self.loc is None:
            raise fault("LOCATION_SESSION_FAILED")
        self.dirty = True  # A failed call may already have reached the phone.
        async with asyncio.timeout(8):
            await self.loc.set(latitude, longitude)
        return {
            "latitude": latitude,
            "longitude": longitude,
            "sessionId": self.session_id,
            "acknowledgement": "device-service-reply",
        }

    async def clear(self):
        if self.loc is None:
            raise fault("LOCATION_SESSION_FAILED")
        try:
            async with asyncio.timeout(8):
                await self.loc.clear()
                # Apple's stop selector has no reply. A subsequent read proves a live
                # channel, not that CoreLocation on the physical phone has been observed.
                await self.info_service.mach_time_info()
        except Exception as exc:
            self.last_error = fault("RESTORE_FAILED")
            raise self.last_error from exc
        self.dirty = False
        return {
            "cleared": True,
            "sessionId": self.session_id,
            "acknowledgement": "clear-sent-and-service-responsive",
            "physicalObservation": "UNVERIFIED",
        }

    async def close(self):
        if self.stack is None:
            if self.dirty:
                raise fault("RESTORE_FAILED")
            return {"closed": True, "cleared": not self.dirty}
        restore_error = None
        try:
            if self.loc is not None:
                await self.clear()
        except BaseException as exc:
            restore_error = exc
        finally:
            stack, self.stack = self.stack, None
            await stack.aclose()
            self.rsd = self.dvt = self.loc = self.info_service = None
            self.udid = self.transport = self.session_id = None
            self.session_metadata = {}
        if restore_error:
            raise fault("RESTORE_FAILED") from restore_error
        if self.cleanup_errors:
            raise fault("CLEANUP_FAILED")
        return {"closed": True, "cleared": True}

    def _pairing_ids(self) -> dict:
        try:
            result = json.loads((self.data_dir / "pairing-index.json").read_text("utf-8"))
            if isinstance(result, dict):
                return result
        except (FileNotFoundError, ValueError):
            pass
        return {}

    async def prepare_pairing(self, udid: str):
        if self.udid != udid or self.transport != "usb" or self.rsd is None:
            raise fault("WIRELESS_NOT_PREPARED")
        if self.dirty:
            raise RuntimeFault(
                "BUSY",
                "Restore location before preparing Wi-Fi.",
                "Choose Restore Real Location, then Prepare Wi-Fi.",
            )
        from pymobiledevice3.remote.remote_service_discovery import RemoteServiceDiscoveryService
        from pymobiledevice3.remote.tunnel_service import (
            create_core_device_tunnel_service_using_rsd,
        )

        # CoreDeviceTunnelService owns and closes its RSD. Give it a separate
        # connection so preparation cannot close our persistent location session.
        async with AsyncExitStack() as stack:
            client = await self._lockdown(udid)
            self._own(stack, client.close)
            await client.set_enable_wifi_connections(True)
            peer = RemoteServiceDiscoveryService(
                self.rsd.service.address, open_connection=self.rsd.open_connection
            )
            self._own(stack, peer.close)
            await peer.connect()
            if peer.udid != udid:
                raise fault("DEVICE_IDENTITY_MISMATCH")
            service = await create_core_device_tunnel_service_using_rsd(peer, autopair=True)
            self._own(stack, service.close)
            remote_id = validate_udid(service.remote_identifier)
        # Re-open with autopair=False to prove existing-record pair verification.
        async with AsyncExitStack() as verify_stack:
            peer = RemoteServiceDiscoveryService(
                self.rsd.service.address, open_connection=self.rsd.open_connection
            )
            self._own(verify_stack, peer.close)
            await peer.connect()
            if peer.udid != udid:
                raise fault("DEVICE_IDENTITY_MISMATCH")
            verified = await create_core_device_tunnel_service_using_rsd(peer, autopair=False)
            self._own(verify_stack, verified.close)
            if verified.remote_identifier != remote_id:
                raise fault("DEVICE_IDENTITY_MISMATCH")
        index = self._pairing_ids()
        index[redacted(udid)] = remote_id
        self.data_dir.mkdir(parents=True, exist_ok=True, mode=0o700)
        temporary = self.data_dir / ("pairing-index-" + uuid.uuid4().hex + ".tmp")
        try:
            with temporary.open("x", encoding="utf-8") as stream:
                json.dump(index, stream)
                stream.flush()
                os.fsync(stream.fileno())
            temporary.replace(self.data_dir / "pairing-index.json")
        finally:
            temporary.unlink(missing_ok=True)
        return {"remotePairingState": "verified", "deviceIdRedacted": redacted(udid)}

    async def prepare_image(self, udid: str):
        if self.stack:
            raise fault("SESSION_ALREADY_OPEN")
        from pymobiledevice3.services.mobile_image_mounter import (
            PersonalizedImageMounter,
            auto_mount,
        )

        client = await self._lockdown(udid)
        try:
            # Current upstream caches by image build. Network download is only in
            # this explicit preparation method, never in map appearance/retarget.
            async with PersonalizedImageMounter(client) as mounter:
                if await mounter.is_image_mounted("Personalized"):
                    return {"developerImageState": "mounted"}
            await auto_mount(client)
            async with PersonalizedImageMounter(client) as mounter:
                if not await mounter.is_image_mounted("Personalized"):
                    raise fault("DEVELOPER_SUPPORT_MOUNT_FAILED")
            return {"developerImageState": "mounted"}
        finally:
            await self._close_resource(client.close)

    async def dispatch(self, method: str, params: dict):
        if method == "runtime.info":
            return self.info()
        if method == "device.list":
            return await self.list_devices()
        if method == "device.trust":
            return await self.trust(params["udid"])
        if method == "session.open":
            return await self.open(
                params["udid"], params["transport"], params.get("rsdAddress"), params.get("rsdPort")
            )
        if method == "session.health":
            return await self.health()
        if method == "location.set":
            return await self.set(params["latitude"], params["longitude"])
        if method == "location.clear":
            return await self.clear()
        if method in ("session.close", "runtime.shutdown"):
            return await self.close()
        if method == "pairing.prepare":
            return await self.prepare_pairing(params["udid"])
        if method == "device.prepare":
            return await self.prepare_image(params["udid"])
        if method == "wireless.discover":
            from .wireless import discover

            candidates = await discover()
            return {"candidateCount": len(candidates), "serviceType": "_remotepairing._tcp.local."}
        if method == "diagnostics.get":
            return {
                **self.info(),
                **system_snapshot(),
                "deviceIdRedacted": redacted(self.udid) if self.udid else None,
                "transport": self.transport,
                "sessionOpen": self.loc is not None,
                "lastError": self.last_error.wire() if self.last_error else None,
                "failureClasses": self.last_failure_classes,
                "cleanupFailures": self.cleanup_errors,
            }
        raise fault("INVALID_REQUEST")
