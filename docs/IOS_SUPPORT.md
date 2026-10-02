# iOS support and evidence

Only the source desktop USB protocol path has a physical acceptance report: iPhone 17 Pro Max, iOS 27.0 beta, build 24A5424a, 7 September 2026. The user observed the longer test move to Times Square and restore real location. This does not establish compatibility with every beta, model, transport or app.

The native iPhone UI, embedded Rust session and companion helper have not been built with Xcode or installed on a phone in this Windows workspace. Mobile Rust tests and signing checks pass on Windows, but no development-signed IPA is included. The repository now provides explicit Mac-only `simulator`, `simulator-test`, `device-archive`, and signed `archive` paths in `apps/ios/build.sh`; none has executed on this host.

A static linkage review on 10 September 2026 found the generated target source paths, bridging header, extension entry point, Rust static-library flags, and five declared FFI symbols aligned. The unused `_remoted._tcp` Bonjour declaration was removed from the project specification. This is a source review only and is not an Apple-platform compile result.

Required next environment: a Mac with Xcode and the relevant Apple SDK, XcodeGen, Rust Apple targets, and an authorized development signing team. A simulator can validate Swift compilation/layout but cannot prove physical location simulation. The bridge companion only inspects local networking; the MeridianLab target includes a narrowly routed packet-tunnel extension and schema 2 Lockdown/CoreDevice path, but Apple entitlement approval, self-device routing and physical behavior remain unverified product evidence.

Phone testing was paused by the owner for the night of 7–8 September. Do not run physical probes or infer a later beta test from automated results.

Native acceptance must separately demonstrate correct-device pair-verify, a real local tunnel and RSD identity, persistent retarget, route/joystick, interrupted connection recovery, Clear, and observed real-location return. Do not call the mobile product supported until that evidence exists.
