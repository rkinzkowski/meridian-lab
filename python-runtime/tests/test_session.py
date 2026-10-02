import pytest

from location_runtime.driver import PmdDriver
from location_runtime.errors import RuntimeFault

UDID = "12345678-1234567890123456"


@pytest.fixture
def adapter(monkeypatch, tmp_path):
    from pymobiledevice3.remote import remote_service_discovery
    from pymobiledevice3.services.dvt.instruments import (
        device_info,
        dvt_provider,
        location_simulation,
    )

    calls = []

    class FakeRsd:
        udid = UDID
        product_version = "27.0"
        all_values = {"DeviceName": "TEST DEVICE"}
        fail = False

        def __init__(self, *args, **kwargs):
            pass

        async def connect(self):
            calls.append("rsd.open")
            if self.fail:
                raise ConnectionRefusedError()

        async def close(self):
            calls.append("rsd.close")

    class FakeDvt:
        def __init__(self, *args):
            pass

        async def __aenter__(self):
            calls.append("dvt.open")
            return self

        async def __aexit__(self, *args):
            calls.append("dvt.close")

    class FakeLoc(FakeDvt):
        fail_clear = False

        async def __aenter__(self):
            calls.append("loc.open")
            return self

        async def __aexit__(self, *args):
            calls.append("loc.close")

        async def set(self, lat, lon):
            calls.append(("set", lat, lon, id(self)))

        async def clear(self):
            calls.append("clear")
            if self.fail_clear:
                raise ConnectionResetError()

    class FakeInfo(FakeDvt):
        async def mach_time_info(self):
            calls.append("health")
            return {1: 1}

    monkeypatch.setattr(remote_service_discovery, "RemoteServiceDiscoveryService", FakeRsd)
    monkeypatch.setattr(dvt_provider, "DvtProvider", FakeDvt)
    monkeypatch.setattr(location_simulation, "LocationSimulation", FakeLoc)
    monkeypatch.setattr(device_info, "DeviceInfo", FakeInfo)
    return PmdDriver(tmp_path), calls, FakeRsd, FakeLoc


async def test_same_channel_for_retarget_and_clear(adapter):
    driver, calls, _, _ = adapter
    opened = await driver.open(UDID, "rsd", "::1", 1234)
    a = await driver.set(40, -74)
    b = await driver.set(41, -75)
    assert opened["sessionId"] == a["sessionId"] == b["sessionId"]
    writes = [c for c in calls if isinstance(c, tuple)]
    assert writes[0][3] == writes[1][3]
    assert calls.count("loc.open") == 1
    assert "loc.close" not in calls
    await driver.close()
    assert calls.index("clear") < calls.index("loc.close") < calls.index("rsd.close")


async def test_wrong_device_never_opens_dvt(adapter):
    driver, calls, rsd, _ = adapter
    rsd.udid = "different-device"
    with pytest.raises(RuntimeFault, match="DEVICE_IDENTITY_MISMATCH"):
        await driver.open(UDID, "rsd", "::1", 1234)
    assert "dvt.open" not in calls
    assert "rsd.close" in calls


async def test_explicit_rsd_failure_does_not_fallback(adapter):
    driver, calls, rsd, _ = adapter
    rsd.fail = True
    with pytest.raises(RuntimeFault):
        await driver.open(UDID, "rsd", "::1", 1234)
    assert calls == ["rsd.open", "rsd.close"]


async def test_failed_clear_preserves_uncertainty_and_closes_resources(adapter):
    driver, calls, _, loc = adapter
    await driver.open(UDID, "rsd", "::1", 1234)
    await driver.set(1, 2)
    loc.fail_clear = True
    with pytest.raises(RuntimeFault, match="RESTORE_FAILED"):
        await driver.close()
    assert driver.dirty
    assert "loc.close" in calls and "rsd.close" in calls
    with pytest.raises(RuntimeFault, match="RESTORE_FAILED"):
        await driver.close()


async def test_validation_does_not_touch_working_session(adapter):
    driver, calls, _, _ = adapter
    await driver.open(UDID, "rsd", "::1", 1234)
    with pytest.raises(RuntimeFault):
        await driver.set(100, 0)
    assert not any(isinstance(c, tuple) for c in calls)
    assert driver.loc is not None
    await driver.close()


async def test_pairing_wrong_target_does_not_touch_session(adapter):
    driver, calls, _, _ = adapter
    await driver.open(UDID, "rsd", "::1", 1234)
    with pytest.raises(RuntimeFault):
        await driver.prepare_pairing("87654321-1234567890123456")
    assert "loc.close" not in calls
    await driver.close()
