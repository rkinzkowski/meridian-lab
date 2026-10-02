import asyncio
import json

from location_runtime.server import Server


class FakeDriver:
    def __init__(self):
        self.calls = []
        self.entered = asyncio.Event()
        self.release = asyncio.Event()

    async def dispatch(self, method, params):
        self.calls.append(method)
        if method == "device.trust":
            self.entered.set()
            await self.release.wait()
        return {"TEST": True}

    async def close(self):
        return {"closed": True}


def line(request_id, method, params=None):
    return json.dumps({"requestId": request_id, "method": method, "params": params or {}}).encode()


async def test_duplicate_request_is_idempotent():
    emitted = []
    driver = FakeDriver()
    server = Server(driver, emitted.append)
    request = line("a", "location.clear")
    await server.accept(request)
    await asyncio.gather(*server.active.values())
    await server.accept(request)
    assert driver.calls == ["location.clear"]
    assert emitted[0] == emitted[1]
    await server.accept(line("a", "runtime.info"))
    assert emitted[-1]["error"]["code"] == "INVALID_REQUEST"


async def test_cancellation_reaches_waiting_operation():
    emitted = []
    driver = FakeDriver()
    server = Server(driver, emitted.append)
    await server.accept(line("pair", "device.trust", {"udid": "12345678-1234567890123456"}))
    await driver.entered.wait()
    await server.accept(line("cancel", "operation.cancel", {"targetRequestId": "pair"}))
    await server.finish()
    assert any(r.get("error", {}).get("code") == "CANCELLED" for r in emitted)


async def test_invalid_input_cannot_cancel_live_work():
    driver = FakeDriver()
    emitted = []
    server = Server(driver, emitted.append)
    await server.accept(b"not json")
    assert not driver.calls
    assert emitted[0]["error"]["code"] == "INVALID_REQUEST"
