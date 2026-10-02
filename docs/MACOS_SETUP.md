# macOS setup

No macOS physical-device or signed/notarized desktop release has been validated in this workspace. The Python adapter has a macOS branch using upstream `NativeRemotedTunnel`; that source branch is not proof of target-version compatibility.

## Provisional source-development path

Install [Tauri's macOS prerequisites](https://v2.tauri.app/start/prerequisites/), Node.js 24.19.0, pnpm 11.19.0, Rust 1.98.1, and Python 3.12. Use Xcode/Finder to complete ordinary Apple trust and developer setup. Do not install the Windows Apple Devices application on a Mac.

The current hashed Python lock includes Windows-only dependencies. Until a reviewed macOS hash lock is added, this is a provisional development environment, with transitive dependencies resolved on that Mac:

```sh
python3.12 -m venv .venv
.venv/bin/python -m pip install pymobiledevice3==11.9.1 setuptools==84.0.0 pytest==8.4.2 pytest-asyncio==1.2.0 pyinstaller==6.16.0
.venv/bin/python -m pip install --no-deps --no-build-isolation -e python-runtime
.venv/bin/python -m pytest python-runtime/tests -q
pnpm install --frozen-lockfile
pnpm test
pnpm build
export MERIDIAN_PYTHON="$PWD/.venv/bin/python"
pnpm tauri dev
```

Connect an unlocked development-enabled phone over USB, select its exact identity, and use the desktop setup controls. Confirm actual developer-service access before Teleport. The source USB path rejects iOS below 17.4; broader legacy coverage has not been implemented.

## Packaging gates

Freeze the runtime on the target Mac; do not copy the Windows `.exe` bundle. After the frozen runtime passes open/retarget/Clear/cleanup on hardware, a local DMG build can be attempted with `pnpm tauri build --bundles dmg`.

The Tauri configuration declares macOS 12 as a packaging minimum. That is a manifest setting, not a tested support floor for every runtime dependency or native developer tunnel. Check current upstream macOS requirements on the actual release Mac.

Developer ID signing, entitlements, hardened-runtime compatibility of bundled libraries, notarization, clean-machine installation, and uninstall/upgrade behavior remain release work. No credentials or signing identities are embedded in this repository. See [Tauri macOS distribution](https://v2.tauri.app/distribute/macos-application-bundle/) and [Apple notarization](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
