"""Opt-in physical test of the exact packaged runtime; never used by ordinary CI."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import queue
import subprocess
import threading
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--device-json', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--transport', choices=['usb', 'wifi'], default='usb')
    parser.add_argument('--hold-seconds', type=int, choices=range(5, 61), default=20)
    parser.add_argument('--prepare-wireless', action='store_true')
    parser.add_argument('--exercise', action='store_true')
    args = parser.parse_args()
    if os.environ.get('ENABLE_IOS_DEVICE_TESTS') != '1':
        raise SystemExit('Physical testing requires explicit ENABLE_IOS_DEVICE_TESTS=1.')
    selection = json.loads(args.device_json.read_text(encoding='utf-8-sig'))
    if isinstance(selection, list):
        if len(selection) != 1:
            raise SystemExit('Select exactly one device in the private selection file.')
        selection = selection[0]
    udid = selection['id']
    executable = args.bundle.resolve() / 'runtime/meridian-runtime/meridian-runtime.exe'
    report = {'date': datetime.now(timezone.utc).isoformat(), 'artifact': 'packaged Windows runtime',
              'runtimeSha256': hashlib.sha256(executable.read_bytes()).hexdigest(),
              'transport': args.transport, 'steps': [], 'physicalGpsObservation': 'UNVERIFIED',
              'noGpsFlashObservation': 'UNVERIFIED'}
    process = subprocess.Popen([str(executable)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, text=True, creationflags=subprocess.CREATE_NO_WINDOW)
    replies = queue.Queue()
    def read_lines():
        for line in process.stdout:
            replies.put(line)
        replies.put(None)
    threading.Thread(target=read_lines, daemon=True).start()
    serial = 0
    def call(method, params):
        nonlocal serial
        serial += 1
        identity = f'physical-{serial}'
        process.stdin.write(json.dumps({'requestId': identity, 'method': method, 'params': params}) + '\n')
        process.stdin.flush()
        line = replies.get(timeout=105)
        if line is None:
            raise RuntimeError('RUNTIME_EXITED')
        value = json.loads(line)
        if value.get('requestId') != identity:
            raise RuntimeError('PROTOCOL_INVALID')
        step = {'name': method, 'result': 'PASS' if value.get('ok') is True else 'FAIL'}
        if value.get('ok') is not True:
            step['error'] = value.get('error')
        report['steps'].append(step)
        print(json.dumps(step), flush=True)
        if value.get('ok') is not True:
            raise RuntimeError(value.get('error', {}).get('code', 'UNKNOWN'))
        return value['result']
    failed = False
    try:
        report['runtime'] = call('runtime.info', {})
        if args.transport == 'usb':
            found = call('device.list', {})
            if not any(d['id'] == udid for d in found['devices']):
                raise RuntimeError('SELECTED_DEVICE_NOT_FOUND')
        opened = call('session.open', {'udid': udid, 'transport': args.transport})
        report['deviceIdRedacted'] = opened['deviceIdRedacted']
        report['productVersion'] = opened['productVersion']
        session = opened['sessionId']
        if call('session.health', {})['sessionId'] != session:
            raise RuntimeError('SESSION_CHANGED')
        if args.prepare_wireless:
            call('pairing.prepare', {'udid': udid})
        if args.exercise:
            for label, lat, lon in [('Times Square', 40.758, -73.9855), ('Central Park', 40.771133, -73.974187)]:
                applied = call('location.set', {'latitude': lat, 'longitude': lon})
                if applied['sessionId'] != session:
                    raise RuntimeError('SESSION_CHANGED')
                print(json.dumps({'phase': label, 'holdSeconds': args.hold_seconds}), flush=True)
                time.sleep(args.hold_seconds)
            cleared = call('location.clear', {})
            if cleared.get('cleared') is not True or cleared['sessionId'] != session:
                raise RuntimeError('RESTORE_UNCONFIRMED')
            print(json.dumps({'phase': 'Clear sent; observe return to real location'}), flush=True)
    except Exception as exc:
        failed = True
        report['failureClass'] = type(exc).__name__
        if isinstance(exc, RuntimeError) and str(exc).replace('_', '').isalnum():
            report['failureCode'] = str(exc)
    finally:
        try:
            closed = call('runtime.shutdown', {})
            if closed.get('closed') is not True or closed.get('cleared') is not True:
                raise RuntimeError('CLEANUP_UNCONFIRMED')
        except Exception as exc:
            failed = True
            report['cleanupFailureClass'] = type(exc).__name__
        process.stdin.close()
        try:
            process.wait(timeout=35)
        except subprocess.TimeoutExpired:
            failed = True
            process.kill()
            process.wait()
        report['exitCode'] = process.returncode
        report['protocolResult'] = 'PASS' if not failed and process.returncode == 0 else 'FAIL'
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(report, indent=2), flush=True)
    return 0 if report['protocolResult'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
