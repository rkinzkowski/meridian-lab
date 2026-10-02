"""Archive an already verified Windows bundle and a source snapshot. No device access."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
from subprocess import CompletedProcess
import zipfile

from verify_bundle import bundle_fingerprint, self_test_passed

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED_DIRS = {
    '.git', 'node_modules', 'target', 'dist', '.venv', '__pycache__',
    '.pytest_cache', '.ruff_cache', 'test-results', 'work', 'xcuserdata',
}
GENERATED = ('apps/desktop/src-tauri/runtime/', 'apps/desktop/src-tauri/gen/',
             'apps/ios/build/', 'apps/ios/Generated/')


def validated_offline_report(path: Path, bundle: Path):
    report = json.loads(path.read_text(encoding='utf-8'))
    if report.get('result') != 'PASS' or report.get('phoneAccessed') is not False:
        raise ValueError('A passing offline bundle report is required.')
    if report.get('bundleSha256') != bundle_fingerprint(bundle):
        raise ValueError('Bundle files changed or the report predates full-bundle hashing; recheck it first.')
    expected = {
        'runtime-prerequisites': bundle / 'runtime/meridian-runtime/meridian-runtime.exe',
        'desktop-owned-runtime-and-demo': bundle / 'Meridian Lab.exe',
    }
    steps = report.get('steps', [])
    if not isinstance(steps, list) or len(steps) != len(expected):
        raise ValueError('The report must contain both required offline checks exactly once.')
    if any(not isinstance(step, dict) for step in steps) or {step.get('name') for step in steps} != set(expected):
        raise ValueError('The report must contain both required offline checks exactly once.')
    for step in steps:
        result = CompletedProcess([], step.get('exitCode'), step.get('output', ''), step.get('error', ''))
        if step.get('result') != 'PASS' or not self_test_passed(step['name'], result):
            raise ValueError('Both offline checks must pass, including native TLS-PSK negotiation.')
        if hashlib.sha256(expected[step['name']].read_bytes()).hexdigest() != step.get('sha256'):
            raise ValueError('Bundle changed after its offline verification; recheck it first.')
    return report


def source_files():
    # Prune before descending: build/dependency trees can be several gigabytes.
    def descend(folder: Path):
        for path in sorted(folder.iterdir()):
            rel = path.relative_to(ROOT).as_posix()
            if path.is_symlink() or path.name in EXCLUDED_DIRS:
                continue
            if path.is_dir() and path.name.endswith(('.egg-info', '.dist-info')):
                continue
            if path.name == '.env' or path.name == 'private.json' or path.name.endswith(('.private.json', '-private.json')):
                continue
            if any((rel + '/').startswith(prefix) for prefix in GENERATED):
                continue
            if path.is_dir():
                yield from descend(path)
            elif path.is_file() and path.suffix not in {'.pyc', '.local', '.xcuserstate'}:
                yield path
    yield from descend(ROOT)


def archive(path: Path, files, base: Path, prefix: str):
    count = 0
    with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=6) as output:
        for item in files:
            if item.is_symlink():
                raise ValueError('Delivery must not include symbolic links.')
            output.write(item, f'{prefix}/{item.relative_to(base).as_posix()}')
            count += 1
    with zipfile.ZipFile(path) as output:
        damaged = output.testzip()
        if damaged:
            raise ValueError(f'Archive integrity check failed: {damaged}')
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--verification-report', type=Path,
                        default=ROOT / 'tests/bundle/windows-offline-2026-09-08.json',
                        help='Fresh portable-bundle report from verify_bundle.py; executable hashes must match.')
    args = parser.parse_args()
    output = args.output.resolve(strict=True)
    bundle = output / 'Meridian Lab Windows'
    installer = output / 'MeridianLab-0.1.0-Windows-Setup.exe'
    report = validated_offline_report(args.verification_report, bundle)
    if not installer.is_file() or not (bundle / 'WebView2Loader.dll').is_file():
        raise ValueError('The installer and Windows loader are required.')
    source_zip = output / 'MeridianLab-0.1.0-Source.zip'
    portable_zip = output / 'MeridianLab-0.1.0-Windows-Portable.zip'
    source_count = archive(source_zip, source_files(), ROOT, 'meridian-lab')
    portable_count = archive(portable_zip, sorted(p for p in bundle.rglob('*') if p.is_file()), bundle, bundle.name)
    artifacts = []
    for path in [installer, portable_zip, source_zip]:
        artifacts.append({'file': path.name, 'bytes': path.stat().st_size,
                          'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    manifest = {
        'createdAt': datetime.now(timezone.utc).isoformat(),
        'version': '0.1.0', 'channel': 'unsigned-development',
        'sourceFiles': source_count, 'portableFiles': portable_count,
        'offlineBundle': report,
        'limits': ['Offline checks access no phone; physical observations apply only to their recorded device and build.',
                   'Native TLS-PSK prerequisites pass. Cable-free Wi-Fi location control remains unverified: the recorded attempt found no device advertisement and sent no location change.',
                   'Native iOS source has not been built with Xcode.',
                   'Clean-machine installation remains unverified; installing on the development host does not establish it.',
                   'Complete third-party notices and release signing remain public-release gates.'],
        'artifacts': artifacts,
    }
    (output / 'BUILD_MANIFEST.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    (output / 'SHA256SUMS.txt').write_text(''.join(f"{a['sha256']}  {a['file']}\n" for a in artifacts), encoding='ascii')
    print(json.dumps({'sourceFiles': source_count, 'portableFiles': portable_count, 'artifacts': artifacts}, indent=2))


if __name__ == '__main__':
    main()
