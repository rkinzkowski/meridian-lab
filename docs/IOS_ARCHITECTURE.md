# Native iPhone architecture

`apps/ios` is SwiftUI/MapKit with a Rust static library from `crates/mobile-core`. It has location search and completion, coordinate/map selection, Teleport/Restore controls, road-route geometry from MapKit, route progress/pause/speed, stepped joystick controls, a local saved/history library and sanitized diagnostics.

The Swift actor uses an opaque Rust handle. Calls run in detached tasks so Stop can reach the Rust cancellation channel while connection setup is awaiting a reply. Rust owns a bounded command queue and one persistent session on a dedicated native thread. The dedicated thread accommodates the current upstream TLS-PSK handshake's non-Send future. No shell, subprocess or loopback HTTP controller is used by the mobile core.

Connection sequence for the verified local path: the desktop exports a complete USB Lockdown plist bound to the selected UDID as schema 2 (`localCoreDevice`, `10.83.0.2` ↔ `10.83.0.1`, TCP 62078); the iPhone validates that record, starts its narrowly routed Network Extension, opens CoreDeviceProxy, then performs Lockdown identity verification → RSD handshake → persistent DVT LocationSimulation. The legacy schema 1 RemotePairing path remains separately validated. Route points advance only after the library's Set call succeeds. Clear is attempted on stop/connection teardown. Library success is not a physical GPS observation.

`PairingStore` consumes only `Documents/meridian-device.json`, excludes it from backup, applies complete file protection, stores it in WhenUnlockedThisDeviceOnly Keychain and removes the inbox file. Rust checks schema, coordinates, local address and signing-key consistency. Operational pair-verify still determines whether the record is accepted by the phone. No actual pairing record is shipped.

`apps/ios-bridge` includes the companion NWPathMonitor/Bonjour diagnostics app and the `MeridianLocalTunnel` Network Extension source used by the main target. The extension only configures the two-address local route and reflects bounded IPv4/TCP packets; it does not forward to an internet server or establish developer readiness by itself. Self-device access on iOS 27.0 beta has not been demonstrated.

Build: `bash apps/ios/build.sh simulator`, `archive`, or `bridge` on a Mac. XcodeGen produces the Xcode project. Build both app and Rust for the matching architecture. Apple-platform compilation, lifecycle/background behavior and physical mobile simulation are unverified. See IOS_SUPPORT.md and SIDELOAD.md.
