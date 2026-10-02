# Dependency decisions

Updated 9 September 2026 from installed package metadata, lockfiles, inspected upstream source, and physical reports. Version choices below are exact build inputs, not a claim that all combinations are release validated.

## Device and native stack

| Dependency | Choice / declared license | Reason and verification | Upgrade or release caveat |
|---|---|---|---|
| Python | 3.13.15 x64; Python Software Foundation terms | Native TLS-PSK callbacks work with OpenSSL 3.5.8; source and frozen offline checks pass. The application requires Python ≥3.13 | Current self-contained interpreter came from Astral python-build-standalone. Build each platform's frozen runtime on that platform; complete clean-machine testing |
| pymobiledevice3 | 11.9.1; GPL-3.0-or-later | Current audited APIs for managed userspace USB transport, RSD/DVT, pairing, image preparation; source Set A/Set B/Clear passed on iPhone 17 Pro Max / iOS 27.0 beta | Runtime rejects other versions; rerun identity, persistence, cleanup, Wi-Fi, and physical retarget tests on upgrade |
| PyInstaller | 6.16.0; GPL terms with bootloader/distribution exception | Build-environment pin used by `tools/build_runtime.py` | This is the chosen build version, not a latest-version claim. Frozen device open/cleanup remains its own acceptance gate |
| Rust | 1.98.1 | Installed current native development toolchain; the root manifest declares Rust 1.89 as the minimum for the standard file-lock API used by restoration journaling | The current checks use 1.98.1; Rust 1.89 compatibility was not separately built on this host |
| Tauri Rust | 2.11.5 in Cargo.lock; Apache-2.0 OR MIT | Native command boundary, local webview, Windows NSIS packaging | Manifests allow Tauri 2; use `--locked`. Clean-machine installation remains unverified |
| idevice | exactly 0.1.65; MIT | Embedded mobile/bootstrap protocol boundary with selected features | Pre-1.0 APIs need per-upgrade source audit. No signed physical native-app result |
| Tokio | 1.53.1 in Cargo.lock; MIT | Async process/route coordination | Rerun cancellation and teardown tests after upgrades |
| Serde | 1.0.229 in Cargo.lock; MIT OR Apache-2.0 | Native wire/state serialization | Preserve strict input limits and Rust/TypeScript contract parity |

The pymobiledevice3 distribution metadata declares Python ≥3.9; Meridian sets its own floor at 3.13 to use native TLS-PSK. The old Python 3.12 build failed to load `sslpsk-pmd3`; Python 3.13 takes upstream's native callback branch and does not require that extension. The self-test negotiates TLS 1.2 PSK in memory, exchanges a synthetic payload, and verifies rejection of an incorrect key. No certificate policy, firewall setting, or system TLS configuration is changed by that check.

`lzfse==0.4.2`, an upstream transitive dependency, had no CPython 3.13 Windows wheel in its published release. Its unchanged source was built with the existing isolated LLVM-MinGW compiler. The Windows hash lock references that vendored wheel directly; see [source, hashes, notices, and build provenance](../python-runtime/wheels/README.md). Other runtime dependency version pins were retained while resolving for Python 3.13.

The Python lock includes unconditional `pywin32==312` and a Windows x64/CPython 3.13 wheel; it is not a macOS/Linux release lock. The freeze script excludes several broad upstream CLI/interactive dependencies from analysis, but the generated inventory describes the build environment rather than the exact frozen files. Do not substitute that inventory for an artifact-derived SBOM.

The packaged desktop coordinator and bundled Python 3.13 runtime passed the recorded USB route/control sequence, with the owner confirming movement, pause, resume, joystick movement, and restoration. This report did not automate graphical clicks. USB preparation for wireless pairing passed; the cable-free attempt found no usable advertisement and sent no location change. Native PSK capability therefore fixes a prerequisite without establishing physical Wi-Fi acceptance. Evidence: [desktop USB report](../tests/physical-device/desktop-usb-2026-09-08.json) and [Wi-Fi attempt](../tests/physical-device/packaged-wifi-2026-09-08.json).

## Frontend

| Dependency | Exact pin | License | Selection / validation |
|---|---|---|---|
| React / React DOM | 19.2.8 | MIT | Current React APIs; TypeScript/Vite build passed |
| TypeScript | 7.0.2 | Apache-2.0 | Strict renderer and shared command types |
| Vite | 8.2.2 | MIT | Current build system, Node 24 compatible |
| Vite React plugin | 6.1.1 | MIT | React integration for the selected Vite version |
| Tauri JS API | 2.11.1 | Apache-2.0 OR MIT | Typed invoke and runtime-status listener |
| Tauri CLI | 2.11.4 | Apache-2.0 OR MIT | Native build entry point |
| MapLibre GL JS | 6.7.0 | BSD-3-Clause | Independent map provider boundary; opt-in map network use |
| Lucide React | 1.42.0 | ISC | Open-source icon set; original product branding |
| Vitest / jsdom | 5.0.0 / 30.0.1 | MIT | Frontend coordinate/GPX/demo-boundary and UI checks; current totals are recorded in BUILD_STATUS.md |
| pnpm | 11.19.0 | MIT | Matches root packageManager and lockfile tooling |

Additional frontend test/type dependencies are pinned in `apps/desktop/package.json` and fully resolved in `pnpm-lock.yaml`. The current frontend uses plain CSS. Tailwind, a commercial map service, and a copied component/layout system were not introduced.

OpenFreeMap is an optional hosted tile/style provider, not a vendored dataset. The current application exposes the dark street style. Alternative provider descriptors exist but light/street/satellite selection is not wired into the UI. The offline view is a coordinate grid; no Natural Earth basemap was downloaded or bundled.

## Primary sources

- [pymobiledevice3 repository](https://github.com/doronz88/pymobiledevice3), [PyPI release metadata](https://pypi.org/project/pymobiledevice3/11.9.1/), and [license](https://github.com/doronz88/pymobiledevice3/blob/master/LICENSE)
- [Python 3.13.15 release](https://www.python.org/downloads/release/python-31315/), [native PSK API](https://docs.python.org/3.13/library/ssl.html#ssl.SSLContext.set_psk_client_callback), [Astral managed Python provenance](https://docs.astral.sh/uv/concepts/python-versions/), and [Python license](https://docs.python.org/3/license.html)
- [lzfse 0.4.2 source and published files](https://pypi.org/project/lzfse/0.4.2/) and [PyInstaller license](https://pyinstaller.org/en/stable/license.html)
- [idevice repository](https://github.com/jkcoxson/idevice) and [crate documentation](https://docs.rs/idevice)
- [Tauri](https://v2.tauri.app/), [React versions](https://react.dev/versions), [Vite guide](https://vite.dev/guide/), [MapLibre GL JS](https://maplibre.org/maplibre-gl-js/docs/), and [OpenFreeMap integration](https://openfreemap.org/quick_start/)

No phone-dependent compatibility pass is inferred from dependency installation, TypeScript compilation, mock tests, or a package's own declared support range.
