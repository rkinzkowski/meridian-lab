# Meridian Lab

An original location-simulation application for development-enabled iPhones. The Windows app uses React/TypeScript in Tauri, with a bundled Python runtime and one persistent Apple developer location session. The native iPhone application is intended to operate on the phone itself after initial setup, without a running computer or a shared LAN.

## Current result — 11 September 2026

The rebuilt Windows application opens its embedded interface without a development server, and street maps render in the actual Windows window. Its bundled Python 3.13.15 runtime passes native TLS-PSK and offline helper/demo checks. The current desktop executable SHA-256 is `57f9893c22014410549883bc724abe020d837f61678ed5d9d4fc023dd04e2bf`; the installer SHA-256 is `d939afafb07cb35d181237babec4c30c51b158cf13889b686e6e2b03b5903c9e`.

The Windows native engine and bundled Python 3.13 runtime passed a physical USB run on the owner's iPhone 17 Pro Max / iOS 27.0 beta: Teleport, northbound route, pause, faster resume, eastward joystick movement, Restore and clean shutdown. The owner confirmed every visible phase. An earlier packaged retarget test moved to Times Square and Central Park, restored real location, and produced no jump noticed by the owner. These are observations from specific runs, not a guarantee for all iOS versions or future builds.

The USB tests exercised the real packaged runtime and native coordinator; they did not automate graphical clicks in the installed Tauri window. [Physical native-engine report](tests/physical-device/desktop-usb-2026-09-08.json), [retarget observation](tests/physical-device/packaged-usb-2026-09-08.json).

The independent iPhone product is still under development. It has no accepted signed IPA or on-device test. A desktop Wi-Fi connection is a separate feature and does not satisfy phone-only operation. See [iPhone architecture](docs/IOS_ARCHITECTURE.md) and [iOS support](docs/IOS_SUPPORT.md).

The complete original product specification is not finished. [Build status](docs/BUILD_STATUS.md), [audit](docs/FINAL_AUDIT.md), [audit matrix](docs/AUDIT_MATRIX.md), and [known issues](docs/KNOWN_ISSUES.md) separate verified results from remaining work. The current GUI loaded a persisted real pending restore and passed Keep open, X, explicit close and relaunch reminder checks. The user-operated follow-up also passed Times Square, pending-close journal retention, reconnect/Restore/Clear and Ready/banner disappearance for the tested session. Broader device and transport coverage remains open.

## Windows application

Use the setup executable or keep the entire portable folder together and open **Meridian Lab.exe**. The runtime is bundled; the user does not need to install Python, Rust or Node. The current packages are unsigned development builds.

For interface-only exploration, choose **Settings → Enter Demo Device** after resolving any pending restore reminder. Demo never operates a phone. For real USB use, follow [Windows setup](docs/WINDOWS_SETUP.md). The current same-LAN Wi-Fi attempt could not discover the phone's service; do not treat that transport as verified.

## Development

Use [Development](docs/DEVELOPMENT.md) for pinned tools and hash-locked dependencies. Frontend commands from the repository root:

```sh
pnpm install --frozen-lockfile
pnpm test
pnpm build
pnpm dev
```

The browser preview starts disconnected and cannot access USB. Online street maps are optional; the offline coordinate grid and bundled landmark list work without them.

## Documentation

- [User guide](docs/USER_GUIDE.md), [pairing](docs/PAIRING.md), [desktop wireless](docs/WIRELESS.md)
- [Architecture](docs/ARCHITECTURE.md), [iPhone architecture](docs/IOS_ARCHITECTURE.md), [signing](docs/SIDELOAD.md)
- [Security](docs/SECURITY.md), [privacy](docs/PRIVACY.md), [dependencies](docs/DEPENDENCY_DECISIONS.md), [third-party notices](docs/THIRD_PARTY_LICENSES.md)
- [Release gates](docs/RELEASING.md), [known issues](docs/KNOWN_ISSUES.md)

There are no product accounts, payments, analytics, or extracted reference-application assets. Pairing credentials are private setup inputs and are never included in the source or packages.
