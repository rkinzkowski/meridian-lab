# Third-party license inventory

This is an initial inventory, not a completed distribution notice bundle or legal compatibility determination. Original reference-application source, assets, identifiers, keys, and certificates are not application dependencies.

## Direct dependencies

| Component | Declared terms | Source |
|---|---|---|
| pymobiledevice3 11.9.1 | GPL-3.0-or-later | [Upstream license](https://github.com/doronz88/pymobiledevice3/blob/master/LICENSE) |
| CPython 3.13.15 | PSF license and included third-party notices; packaged from Astral python-build-standalone | [Python license](https://docs.python.org/3.13/license.html) and [build distribution](https://github.com/astral-sh/python-build-standalone) |
| OpenSSL 3.5.8 | Apache-2.0 and included notices | [OpenSSL source/license](https://github.com/openssl/openssl/blob/openssl-3.5.8/LICENSE.txt) |
| lzfse 0.4.2 local CPython 3.13 Windows wheel | MIT wrapper; bundled Apple LZFSE implementation has its own BSD-style notice | [Wheel provenance and included notice files](../python-runtime/wheels/README.md) |
| PyInstaller 6.16.0 | GPLv2-or-later with special exception; see exact bundled license | [PyInstaller license](https://pyinstaller.org/en/stable/license.html) |
| Tauri | Apache-2.0 OR MIT | [Tauri repository](https://github.com/tauri-apps/tauri) |
| idevice 0.1.65 | MIT, per downloaded crate metadata | [idevice repository](https://github.com/jkcoxson/idevice) |
| isideload 0.3.17 | MIT, per downloaded crate metadata; optional signing adapter | [isideload repository](https://github.com/nab138/isideload) |
| React / React DOM | MIT | [React license](https://github.com/facebook/react/blob/main/LICENSE) |
| MapLibre GL JS | BSD-3-Clause | [MapLibre license](https://github.com/maplibre/maplibre-gl-js/blob/main/LICENSE.txt) |
| Lucide | ISC | [Lucide license](https://github.com/lucide-icons/lucide/blob/main/LICENSE) |
| TypeScript | Apache-2.0 | [TypeScript license](https://github.com/microsoft/TypeScript/blob/main/LICENSE.txt) |
| Vite | MIT | [Vite license](https://github.com/vitejs/vite/blob/main/LICENSE) |
| Tokio | MIT | [Tokio license](https://github.com/tokio-rs/tokio/blob/master/LICENSE) |
| Serde | MIT OR Apache-2.0 | [Serde repository](https://github.com/serde-rs/serde) |

Exact versions and hashes for transitive code are in `Cargo.lock`, `pnpm-lock.yaml`, and `python-runtime/requirements.lock`. The frozen-runtime build also emits `apps/desktop/src-tauri/runtime/build-dependency-inventory.json`. That file includes packages in the build environment, some excluded from the final executable. Conversely, native libraries and vendored resources need artifact inspection beyond Python package metadata.

`tools/collect_notices.py` collects installed Python distribution license/notice files into the sibling `runtime/notices/` folder and records their origin in `inventory.json`. The inventory includes some packages excluded from the executable; its package count changes with the build environment. This collector does not establish complete coverage for CPython itself, OpenSSL, native libraries, frontend packages, or the WebView2 loader. A successful freeze or installer build does not clear that release requirement.

The Windows GNU build includes the x64 `WebView2Loader.dll` supplied by the locked `webview2-com-sys` 0.38.2 crate. The shipped DLL's SHA-256 is `8427b1fc58ec707813e5c0a51eb5d69397bb333250a7b891be4d3b123f1e0f1c`. The loader is a Microsoft WebView2 component; the Rust wrapper license does not replace Microsoft's component terms. Review and retain the exact SDK redistribution notices before public release. No WebView2 browser runtime installer is included in the portable folder.

## Map data and services

Online maps use [OpenFreeMap](https://openfreemap.org/quick_start/) and OpenStreetMap-derived data. Keep visible [OpenStreetMap attribution](https://www.openstreetmap.org/copyright) and the provider's style/source attribution. Remote fonts, sprites, styles, and underlying map data have their own terms; do not treat the MapLibre code license as a license for every map resource.

The offline coordinate grid and product styling are original. No Natural Earth dataset is currently bundled. No extracted commercial map API key or proprietary tile URL is used. Satellite imagery is not included.

## Original project licensing is not finalized

The root Cargo workspace declares `AGPL-3.0-or-later`; every current application/crate Cargo manifest, including mobile-core, inherits it through `license.workspace = true`. There is no remaining MIT declaration in those original Rust manifests. There is not yet a complete top-level license/notice set covering every original source directory. Verify coverage and add the selected project license texts before publishing a source or binary release. This records existing manifest declarations without making a new licensing decision, relicensing upstream code, or deciding license compatibility by process separation.

Before distribution, retain all applicable notices, establish the corresponding-source and license obligations for the actual bundle, and make the release artifact's inventory reproducible. See [Releasing](RELEASING.md).
