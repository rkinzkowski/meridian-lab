# Development

Commands below run from the repository root. They are source-development commands, not end-user installation instructions.

## Toolchain

| Tool | Current build choice |
|---|---|
| Node.js | 24.19.0 |
| pnpm | 11.19.0, matching `packageManager` |
| Python | 3.13.15 x64 with native TLS-PSK; current Windows source tests and freeze |
| Rust | 1.98.1 installed for the current native build work |
| Windows desktop linker | Prefer the standard MSVC/C++ build environment for reproducible Tauri builds |
| macOS desktop/iOS | Xcode and Apple SDKs on a Mac; not validated from the Windows development host |

The local development host used Rust 1.98.1 GNU with LLVM-MinGW 20260826 because MSVC was unavailable. Optimized core/mobile tests passed, and the Windows application and NSIS installer were produced. See [BUILD_STATUS.md](BUILD_STATUS.md) for dated counts and acceptance evidence. An earlier debug binary encountered a host file-write/execution failure; no protection settings were changed. CI is configured to exercise hosted environments separately and has not run yet. The GNU build also needs the matching WebView2Loader.dll beside the executable; the Windows bundle configuration includes it.

## Frontend

```sh
pnpm install --frozen-lockfile
pnpm test
pnpm build
pnpm dev
```

`pnpm dev` serves `127.0.0.1:1420`. Demo is an explicit UI choice. The browser cannot use real device commands. Vite's output is `apps/desktop/dist`; there is no hosted product website in this implementation.

## Python on Windows

```powershell
py -3.13 -m venv .venv
.\.venv\Scripts\python.exe -m pip install --require-hashes -r python-runtime/requirements.lock
.\.venv\Scripts\python.exe -m pip install setuptools==84.0.0 pytest==8.4.2 pytest-asyncio==1.2.0 pyinstaller==6.16.0
.\.venv\Scripts\python.exe -m pip install --no-deps --no-build-isolation -e python-runtime
$env:PYTEST_DISABLE_PLUGIN_AUTOLOAD = '1'
.\.venv\Scripts\python.exe -m pytest -p pytest_asyncio.plugin python-runtime/tests -q
```

Use Python 3.13.15 to reproduce the recorded build. The Windows/Python 3.13 hash lock includes `pywin32` and a relative path to the vendored CPython 3.13 x64 `lzfse` wheel, so installation must run from the repository root. The wheel's [provenance and rebuild instructions](../python-runtime/wheels/README.md) explain why it was built locally. This lock is not portable to another interpreter, architecture, or macOS. Build and review a separate platform lock before a macOS release; [macOS setup](MACOS_SETUP.md) gives a provisional source-development path.

Python 3.13 uses native TLS-PSK callbacks for the pinned pymobiledevice3 TCP tunnel. The earlier Python 3.12 freeze could not load its optional PSK extension. Run `.\.venv\Scripts\python.exe -m location_runtime --self-test` to verify imports, an in-memory TLS-PSK handshake and incorrect-key rejection, and the isolated userspace stack. This test accesses no phone and does not establish Wi-Fi reachability.

## Tauri with the source runtime

After installing [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) and the frontend/Python dependencies:

```powershell
$env:MERIDIAN_PYTHON = (Resolve-Path .\.venv\Scripts\python.exe).Path
pnpm tauri dev
```

The environment override is honored only in Rust debug builds. The selected path must be an absolute existing executable. Tauri launches `python -m location_runtime` from the runtime source directory. Do not launch a separate runtime manually for this flow.

Core Rust checks, when the native toolchain is operational:

```sh
cargo +1.98.1 test --locked --release --workspace --exclude meridian-desktop
```

This includes no physical phone tests. The desktop shell is a separate target requiring platform GUI prerequisites and built frontend assets.

## Freeze and package

```powershell
.\.venv\Scripts\python.exe tools/build_runtime.py
.\.venv\Scripts\python.exe tools/collect_notices.py --output apps/desktop/src-tauri/runtime/notices
pnpm tauri build --bundles nsis
```

The first command creates `apps/desktop/src-tauri/runtime/meridian-runtime/` and a build-environment dependency inventory. Freezing successfully is not enough: run the offline checks on the assembled bundle, then record separately authorized physical acceptance for the final build. The inventory is explicitly not a complete release SBOM. See [Releasing](RELEASING.md).

## Explicit physical probe

Enumeration is read-only and redacts identities by default:

```powershell
.\.venv\Scripts\python.exe tools/device_probe.py --list
```

To obtain the exact selection identifier, use `--list --show-identities` locally; do not paste raw identifiers into public logs. A write test requires both explicit identity and the opt-in environment flag:

```powershell
$env:ENABLE_IOS_DEVICE_TESTS = '1'
.\.venv\Scripts\python.exe tools/device_probe.py --udid '<selected-device-udid>' --transport usb --exercise --hold-seconds 15 --report tests/physical-device/my-usb-test.json
```

This changes the selected phone's simulated location, applies two nearby Times Square test points by default, then sends Clear and attempts cleanup. `--a LAT LON` and `--b LAT LON` choose other points. A human must separately record observed movement, return to real location, and any GPS flash. Do not run it against an unselected device or in ordinary CI.

## Continuous integration

`.github/workflows/ci.yml` contains frontend build/tests, Windows Python tests, Rust core tests on Windows/macOS, and a macOS unsigned iOS simulator build/XCTest job. It uses no connected hardware, Apple credentials or release uploads. This workflow has not been executed on GitHub in the supplied workspace; a valid file is not a CI pass.

## Offline bundle check

Run `python tools/verify_bundle.py "<portable or installed folder>" --report tests/bundle/local-offline.json` to exercise bundled imports, native TLS-PSK with synthetic keys, local userspace-network prerequisites, desktop-owned helper IPC, and explicit demo Teleport/Clear. Windows folders may contain `Meridian Lab.exe` or the installed name `meridian-desktop.exe`. Save the report outside the checked folder; the check hashes every bundled file before and after execution.

This check never discovers or connects a phone. It does not validate the native graphical interface, installer, or physical restoration. Run it after assembling the final files, then pass the portable folder's report to `python tools/package_delivery.py --output "<delivery output folder>" --verification-report tests/bundle/local-offline.json`. The archiver rejects incomplete reports, missing PSK evidence, and changed bundle files. See [BUILD_STATUS.md](BUILD_STATUS.md) for the current evidence.
