"""Verify packaged prerequisites and IPC without any physical device access."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import sys


DESKTOP_PASS = 'Meridian desktop offline self-test: PASS (no phone accessed)'


def bundle_fingerprint(bundle: Path) -> str:
    """Bind verification to runtime DLLs, resources, and notices as well as executables."""
    if not bundle.is_dir():
        raise FileNotFoundError(f'Bundle folder does not exist: {bundle}')
    digest = hashlib.sha256()
    for path in sorted(bundle.rglob('*')):
        if path.is_symlink():
            raise ValueError('Bundle verification does not accept symbolic links.')
        if path.is_file():
            digest.update(path.relative_to(bundle).as_posix().encode('utf-8') + b'\0')
            with path.open('rb') as source:
                digest.update(hashlib.file_digest(source, 'sha256').digest())
    return digest.hexdigest()


def desktop_executable(bundle: Path, platform: str = sys.platform) -> Path:
    names = ('Meridian Lab.exe', 'meridian-desktop.exe') if platform == 'win32' else ('meridian-desktop',)
    for name in names:
        candidate = bundle / name
        if candidate.is_file():
            return candidate
    raise FileNotFoundError(f'No desktop executable found in {bundle}; expected {", ".join(names)}')


def self_test_passed(name: str, result: subprocess.CompletedProcess) -> bool:
    if result.returncode != 0 or not isinstance(result.stdout, str):
        return False
    if name == 'desktop-owned-runtime-and-demo':
        return DESKTOP_PASS in result.stdout.splitlines()
    if name != 'runtime-prerequisites':
        return False
    try:
        report = json.loads(result.stdout)
    except (json.JSONDecodeError, TypeError):
        return False
    return (
        isinstance(report, dict)
        and report.get('result') == 'PASS'
        and report.get('phoneAccessed') is False
        and report.get('wirelessTlsPskAvailable') is True
        and isinstance(report.get('tlsPsk'), dict)
        and report['tlsPsk'].get('protocol') == 'TLSv1.2'
        and report['tlsPsk'].get('incorrectKeyRejected') is True
    )


def verify_step(name: str, executable: Path):
    step = {'name': name, 'executable': executable.name, 'result': 'FAIL'}
    try:
        before = hashlib.sha256(executable.read_bytes()).hexdigest()
        result = subprocess.run(
            [str(executable.resolve()), '--self-test'], capture_output=True,
            text=True, timeout=60,
            creationflags=subprocess.CREATE_NO_WINDOW if sys.platform == 'win32' else 0,
        )
        after = hashlib.sha256(executable.read_bytes()).hexdigest()
        passed = before == after and self_test_passed(name, result)
        step.update({
            'result': 'PASS' if passed else 'FAIL', 'exitCode': result.returncode,
            'output': result.stdout.strip(), 'error': result.stderr.strip(), 'sha256': after,
        })
        if before != after:
            step['error'] = 'Executable changed during verification; rerun after packaging finishes.'
    except (OSError, subprocess.TimeoutExpired) as exc:
        step.update({'error': str(exc), 'failureClass': type(exc).__name__})
    return step


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--report', type=Path, required=True)
    args = parser.parse_args()
    runtime = args.bundle / 'runtime/meridian-runtime' / ('meridian-runtime.exe' if sys.platform == 'win32' else 'meridian-runtime')
    report = {'date':datetime.now(timezone.utc).isoformat(), 'phoneAccessed':False, 'steps':[]}
    try:
        before = bundle_fingerprint(args.bundle)
    except (OSError, ValueError) as exc:
        before = None
        report['error'] = str(exc)
    report['steps'].append(verify_step('runtime-prerequisites', runtime))
    try:
        desktop = desktop_executable(args.bundle)
        report['steps'].append(verify_step('desktop-owned-runtime-and-demo', desktop))
    except OSError as exc:
        report['steps'].append({'name': 'desktop-owned-runtime-and-demo', 'result': 'FAIL', 'error': str(exc)})
    try:
        report['bundleSha256'] = bundle_fingerprint(args.bundle)
    except (OSError, ValueError) as exc:
        report['bundleSha256'] = None
        report['error'] = str(exc)
    stable = before is not None and before == report['bundleSha256']
    if not stable and 'error' not in report:
        report['error'] = 'Bundle changed during verification; rerun after packaging finishes.'
    report['result'] = 'PASS' if stable and all(s['result']=='PASS' for s in report['steps']) else 'FAIL'
    args.report.parent.mkdir(parents=True,exist_ok=True)
    args.report.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report,indent=2))
    return 0 if report['result']=='PASS' else 1


if __name__=='__main__':
    raise SystemExit(main())
