# Releasing

No public signed installer, notarized macOS package, updater feed, or product website is accepted at this checkpoint. Tauri bundle configuration and a runtime freeze script are present; release validation remains work.

## Build inputs

Use the committed `pnpm-lock.yaml`, `Cargo.lock`, the appropriate platform Python hash lock, and the versions in [Dependency decisions](DEPENDENCY_DECISIONS.md). Build native artifacts on the target platform. The current Python lock is Windows-specific; create and review a separate one for macOS.

For a Windows development bundle:

```powershell
pnpm install --frozen-lockfile
pnpm test
pnpm build
$env:PYTEST_DISABLE_PLUGIN_AUTOLOAD = '1'
.\.venv\Scripts\python.exe -m pytest -p pytest_asyncio.plugin python-runtime/tests -q
.\.venv\Scripts\python.exe tools/build_runtime.py
.\.venv\Scripts\python.exe tools/collect_notices.py --output apps/desktop/src-tauri/runtime/notices
pnpm tauri build --bundles nsis
```

The Python environment and native prerequisites must already be prepared using [Development](DEVELOPMENT.md). NSIS is configured for current-user installation. On a prepared Mac, freeze the runtime there and use `pnpm tauri build --bundles dmg`. These commands are intended build paths, not a statement that either installer has passed acceptance.

## Acceptance gates

1. Run source tests, then exercise the frozen runtime through its actual stdin/stdout protocol. Verify `runtime.info`, enumeration, selected-device open, repeated Set through one channel, Clear, health, and a zero-exit cleanup. A source interpreter pass does not cover PyInstaller missing modules/native DLLs.
2. Run the Tauri app with the bundled runtime, without `MERIDIAN_PYTHON`. Verify device selection and state rollback, point/route/joystick handoffs, validation, restoration failure, process death, and application exit.
3. Test on a clean Windows installation without Python/Rust/Node. Verify required Apple support guidance, WebView2, first-run permissions, normal-user launch, installation, upgrade, uninstall, and preservation/removal of local data as documented.
4. Record actual phone observations separately from service replies. Include no-GPS-flash retargeting, route pause/resume, speed changes, Clear, and physical return. Update a dated compatibility report for each supported iOS/device/transport combination.
5. Complete physical Wi-Fi acceptance before advertising Wi-Fi support. Complete signed native iPhone app/helper installation and real on-device operation before advertising mobile-only simulation.
6. Inspect the final files for credentials and unexpected resources; generate an artifact-derived SBOM and full third-party notices. Resolve original-source license declarations and source-distribution obligations.
7. Sign the final Windows binaries/installer with the owner's chosen certificate. For macOS, sign the app and bundled native components, verify entitlements/hardened runtime, notarize, and staple where required. Do not embed signing keys in source or ordinary CI logs.
8. Publish versioned hashes and only the compatibility claims actually established by those artifacts. A website may be built after the physical/product gates; no download buttons should point to absent installers.

## Current packaging blockers

The packaged native engine and bundled runtime now have a passing USB Teleport/route/joystick/Restore observation. The earlier phone-absent failure remains historical evidence. Same-LAN Wi-Fi, independent iPhone operation, installed graphical acceptance and clean-machine tests are still release gates. Further phone tests are paused by the owner.

A signed updater, update manifest/key management, rollback policy, application self-update UI, and release-upload automation are not implemented. `.github/workflows/ci.yml` is read-only build/test CI; it does not publish artifacts or use Apple credentials.

Primary guidance: [Tauri Windows installer](https://v2.tauri.app/distribute/windows-installer/), [Tauri signing](https://v2.tauri.app/distribute/sign/), [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution), and [PyInstaller operating mode](https://pyinstaller.org/en/stable/operating-mode.html).

The unsigned Windows NSIS development installer includes WebView2Loader.dll and the Python runtime. Its existence is a packaging result; clean-machine graphical installation and broader physical compatibility remain pending. See [BUILD_STATUS.md](BUILD_STATUS.md).
