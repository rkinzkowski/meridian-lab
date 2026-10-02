"""Bounded Bonjour discovery and a selected, verified Wi-Fi userspace tunnel."""

import asyncio
from contextlib import AsyncExitStack

from .errors import fault
from .protocol import usable_address, validate_udid


async def discover():
    from pymobiledevice3.bonjour import browse_remotepairing

    candidates = {}
    for attempt in range(3):
        # Upstream's current browser enumerates interfaces; do not choose the
        # first NIC or infer device identity from an unauthenticated DNS label.
        for service in await browse_remotepairing(timeout=10):
            if not 1 <= service.port <= 65535:
                continue
            for address in service.addresses:
                if usable_address(address.full_ip):
                    key = (address.full_ip, service.port)
                    candidates[key] = key
        if candidates:
            break
        if attempt < 2:
            await asyncio.sleep(1)
    return list(candidates.values())


class WirelessTunnel:
    def __init__(self, remote_id: str):
        self.remote_id = validate_udid(remote_id)
        self.stack = None
        self.watcher = None
        self.rsd = None
        self.previous_factory = False

    async def open(self):
        from pymobiledevice3.remote import tunnel_service
        from pymobiledevice3.remote.remote_service_discovery import RemoteServiceDiscoveryService
        from pymobiledevice3.remote.userspace_tunnel import UserspaceDialPlane

        candidates = await discover()
        if not candidates:
            raise fault("WIRELESS_DEVICE_NOT_ADVERTISING")
        for host, port in candidates[:32]:
            provider = None
            # 11.9.1 still uses a one-second inner socket deadline. Bounded
            # retries avoid global monkeypatching; recorded as an upstream limit.
            for _attempt in range(3):
                try:
                    async with asyncio.timeout(10):
                        provider = await tunnel_service.create_core_device_tunnel_service_using_remotepairing(
                            self.remote_id, host, port, autopair=False
                        )
                    break
                except (OSError, TimeoutError, ConnectionError):
                    await asyncio.sleep(0.4)
                except Exception as exc:
                    raise fault("REMOTE_PAIR_VERIFY_REJECTED") from exc
            if provider is None:
                continue
            stack = AsyncExitStack()
            stack.push_async_callback(provider.close)
            self.previous_factory = tunnel_service.USE_USERSPACE_TUNNEL
            # This is upstream's explicit factory configuration (used by its
            # own UserspaceRsdTunnel). The app admits only one owned tunnel.
            tunnel_service.USE_USERSPACE_TUNNEL = True
            try:
                result = await stack.enter_async_context(provider.start_tcp_tunnel())
                tun = result.client.tun
                tun.set_peer(result.address)
                dial = await stack.enter_async_context(UserspaceDialPlane(tun, result.address))
                rsd = RemoteServiceDiscoveryService(
                    (result.address, result.port),
                    open_connection=dial.dial,
                    auxiliary_metadata=result.auxiliary_metadata,
                )
                stack.push_async_callback(rsd.close)
                await rsd.connect()
                self.stack, self.rsd = stack, rsd
                self.watcher = asyncio.create_task(self._watch(result.client))
                return rsd  # caller must verify RSD.udid before opening DVT
            except BaseException:
                await stack.aclose()
                tunnel_service.USE_USERSPACE_TUNNEL = self.previous_factory
                raise
        raise fault("WIRELESS_TUNNEL_UNREACHABLE")

    async def _watch(self, client):
        try:
            await client.wait_closed()
        finally:
            await self.close()

    async def close(self):
        from pymobiledevice3.remote import tunnel_service

        if self.watcher and self.watcher is not asyncio.current_task():
            self.watcher.cancel()
        self.watcher = None
        if self.stack is not None:
            stack, self.stack = self.stack, None
            try:
                await stack.aclose()
            finally:
                tunnel_service.USE_USERSPACE_TUNNEL = self.previous_factory
                self.rsd = None
