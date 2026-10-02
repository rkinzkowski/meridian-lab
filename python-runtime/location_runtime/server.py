"""Private-pipe JSONL dispatcher with bounded work and responsive cancellation."""

import asyncio
import contextlib
import hashlib
import json
import sys
import threading
from collections import OrderedDict

from .errors import RuntimeFault, classify, fault
from .protocol import MAX_LINE, Request, parse_request

DEADLINES = {
    "session.open": 70,
    "pairing.prepare": 50,
    "device.prepare": 180,
    "device.trust": 40,
    "wireless.discover": 35,
    "session.close": 25,
    "runtime.shutdown": 25,
    "device.list": 60,
}


class Server:
    def __init__(self, driver, emit=None):
        self.driver = driver
        self.emit = emit or self._write
        self.lock = asyncio.Lock()
        self.active = {}
        self.cache = OrderedDict()
        self.shutdown = False

    @staticmethod
    def _write(value):
        sys.stdout.write(
            json.dumps(value, allow_nan=False, ensure_ascii=True, separators=(",", ":")) + "\n"
        )
        sys.stdout.flush()

    async def execute(self, request: Request, fingerprint: str):
        try:
            async with self.lock:
                async with asyncio.timeout(DEADLINES.get(request.method, 12)):
                    result = await self.driver.dispatch(request.method, request.params)
            response = {"requestId": request.request_id, "ok": True, "result": result}
        except asyncio.CancelledError:
            response = {
                "requestId": request.request_id,
                "ok": False,
                "error": fault("CANCELLED").wire(),
            }
        except Exception as exc:
            error = classify(exc)
            self.driver.last_error = error
            # Exception messages may contain pair keys or a device identity. Retain
            # bounded class names and source module names only for packaging diagnosis.
            chain = []
            cause = exc
            while cause is not None and len(chain) < 4:
                chain.append(type(cause).__name__)
                cause = cause.__cause__
            self.driver.last_failure_classes = chain
            response = {"requestId": request.request_id, "ok": False, "error": error.wire()}
        self.cache[request.request_id] = (fingerprint, response)
        while len(self.cache) > 256:
            self.cache.popitem(last=False)
        self.emit(response)
        self.active.pop(request.request_id, None)

    async def accept(self, line: bytes):
        try:
            request = parse_request(line)
        except RuntimeFault as exc:
            self.emit({"requestId": None, "ok": False, "error": exc.wire()})
            return
        fingerprint = hashlib.sha256(
            json.dumps(
                {"method": request.method, "params": request.params},
                sort_keys=True,
                separators=(",", ":"),
            ).encode()
        ).hexdigest()
        if request.request_id in self.cache:
            original, response = self.cache[request.request_id]
            self.emit(
                response
                if original == fingerprint
                else {
                    "requestId": request.request_id,
                    "ok": False,
                    "error": fault("INVALID_REQUEST").wire(),
                }
            )
            return
        if request.request_id in self.active:
            self.emit({"requestId": request.request_id, "ok": False, "error": fault("BUSY").wire()})
            return
        if request.method == "operation.cancel":
            target = self.active.get(request.params["targetRequestId"])
            if target:
                target.cancel()
            self.emit(
                {
                    "requestId": request.request_id,
                    "ok": True,
                    "result": {"cancelled": target is not None},
                }
            )
            return
        if request.method == "runtime.shutdown":
            self.shutdown = True
            await self.finish()
            try:
                async with asyncio.timeout(25):
                    result = await self.driver.close()
                self.emit({"requestId": request.request_id, "ok": True, "result": result})
            except Exception as exc:
                self.emit(
                    {"requestId": request.request_id, "ok": False, "error": classify(exc).wire()}
                )
            return
        if len(self.active) >= 32:
            self.emit({"requestId": request.request_id, "ok": False, "error": fault("BUSY").wire()})
            return
        task = asyncio.create_task(self.execute(request, fingerprint))
        self.active[request.request_id] = task

    async def finish(self):
        tasks = list(self.active.values())
        for task in tasks:
            task.cancel()
        if tasks:
            await asyncio.gather(*tasks, return_exceptions=True)

    async def run(self, input_stream=None):
        loop = asyncio.get_running_loop()
        queue = asyncio.Queue(maxsize=32)
        input_stream = input_stream or sys.stdin.buffer

        def read_pipe():
            # Daemon thread cannot hold Python shutdown hostage after cancellation.
            # Bounded readline prevents an untrusted parent exhausting memory.
            try:
                while True:
                    line = input_stream.readline(MAX_LINE + 1)
                    future = asyncio.run_coroutine_threadsafe(queue.put(line), loop)
                    future.result()
                    if not line or len(line) > MAX_LINE:
                        break
            except (RuntimeError, OSError):
                pass

        threading.Thread(target=read_pipe, name="meridian-parent-pipe", daemon=True).start()
        try:
            while not self.shutdown:
                line = await queue.get()
                if not line:
                    break
                await self.accept(line)
                if len(line) > MAX_LINE:
                    break
        finally:
            await self.finish()
            with contextlib.suppress(Exception):
                async with asyncio.timeout(25):
                    await self.driver.close()
