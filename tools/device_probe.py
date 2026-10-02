"""Explicitly selected physical-device compatibility probe. Never run by unit tests."""

import argparse
import asyncio
import json
import logging
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python-runtime"))
from location_runtime.driver import PmdDriver, redacted
from location_runtime.errors import classify
from location_runtime.protocol import validate_point, validate_udid


async def probe(args):
    driver = PmdDriver()
    if args.list:
        result = await driver.list_devices()
        # Explicit --show-identities is needed to obtain a CLI selection identity.
        if not args.show_identities:
            for row in result["devices"]:
                row["id"] = redacted(row["id"])
                row["name"] = "iPhone (name withheld)"
        print(json.dumps(result, ensure_ascii=True))
        return 0
    if not args.udid or os.environ.get("ENABLE_IOS_DEVICE_TESTS") != "1":
        raise SystemExit("Set ENABLE_IOS_DEVICE_TESTS=1 and supply --udid for developer-service tests.")
    validate_udid(args.udid)
    a = validate_point(*args.a)
    b = validate_point(*args.b)
    report = {"date": datetime.now(timezone.utc).isoformat(), "deviceIdRedacted": redacted(args.udid), "runtime": driver.info(), "steps": [], "physicalGpsObservation": "UNVERIFIED", "noGpsFlashObservation": "UNVERIFIED"}
    code = 1
    try:
        async with asyncio.timeout(70):
            opened = await driver.open(args.udid, args.transport, args.rsd_address, args.rsd_port)
        report["productVersion"] = opened["productVersion"]
        report["steps"].append({"name": "selected-device-rsd-dvt-location-open", "result": "PASS"})
        if args.exercise:
            response_a = await driver.set(*a)
            report["steps"].append({"name": "set-a-service-acknowledged", "result": "PASS"})
            print(json.dumps({"testPhase": "Point A applied; observing", "holdSeconds": args.hold_seconds}), flush=True)
            await asyncio.sleep(args.hold_seconds)
            response_b = await driver.set(*b)
            report["steps"].append({"name": "set-b-same-session", "result": "PASS" if response_a["sessionId"] == response_b["sessionId"] else "FAIL"})
            print(json.dumps({"testPhase": "Point B applied through same session; observing", "holdSeconds": args.hold_seconds}), flush=True)
            await asyncio.sleep(args.hold_seconds)
            await driver.clear()
            report["steps"].append({"name": "clear-sent-service-responsive", "result": "PASS"})
        code = 0
    except Exception as exc:
        report["error"] = classify(exc).wire()
        # The class name is useful and contains no exception message/secret.
        report["failureClass"] = type(exc).__name__
        cause = exc.__cause__
        if cause:
            report["upstreamFailureClass"] = type(cause).__name__
    finally:
        try:
            async with asyncio.timeout(25):
                await driver.close()
            report["steps"].append({"name": "cleanup", "result": "PASS"})
        except Exception as exc:
            report["cleanupError"] = classify(exc).wire()
            code = 1
        report["protocolResult"] = "PASS" if code == 0 else "FAIL"
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(report, ensure_ascii=True))
    return code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--show-identities", action="store_true")
    parser.add_argument("--udid")
    parser.add_argument("--transport", choices=["usb", "wifi", "rsd"], default="usb")
    parser.add_argument("--rsd-address")
    parser.add_argument("--rsd-port", type=int)
    parser.add_argument("--exercise", action="store_true", help="Send two test coordinates then clear")
    parser.add_argument("--hold-seconds", type=int, choices=range(1, 61), default=3)
    parser.add_argument("--a", type=float, nargs=2, default=[40.758, -73.9855])
    parser.add_argument("--b", type=float, nargs=2, default=[40.7581, -73.9854])
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    logging.disable(logging.CRITICAL)
    raise SystemExit(asyncio.run(probe(args)))


if __name__ == "__main__":
    main()
