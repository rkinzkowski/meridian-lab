# Meridian Lab audit matrix — 11 September 2026

This matrix records the current evidence boundary. `PASS` means the named check was observed for the stated artifact and scope. `NEEDS INVESTIGATION` means the result is incomplete or its cause is unknown. `BLOCKED — PHYSICAL DEVICE` means the next step requires the phone and was unavailable. `BLOCKED — REMOTE AUTHORIZATION REQUIRED` means a platform account, signing team or other external authorization is required. `NOT APPLICABLE` means the check is outside the artifact or platform being audited.

| Area | Status | Evidence and boundary |
|---|---|---|
| Python runtime, protocol, server and session tests | PASS | 30 tests pass with Python 3.13.15; no phone access. |
| Rust core, IPC, route, storage, cleanup and bootstrap tests | PASS | 45 optimized Windows tests and clippy pass in the current build. |
| Frontend tests and production TypeScript/Vite build | PASS | 21 frontend tests and strict production build pass after pending-restore modal failure/retry coverage; the workspace-level pnpm wrapper remains environment-blocked by registry/module cleanup. |
| Portable runtime prerequisites and offline helper/demo self-test | PASS | `tests/bundle/windows-offline-2026-09-10.json`; phoneAccessed is false and both executable hashes are recorded. |
| Portable GUI boot without Vite | PASS | Current portable build opened with the development server stopped. |
| Actual Windows street map rendering | PASS | Current portable build visibly rendered roads, labels and water. |
| GUI USB connect and developer-session readiness | PASS | The selected phone reached Ready/trusted developer session in the historical GUI observation; no new phone call was made for the 11 September delivery audit. |
| GUI Times Square Teleport state | PASS | UI/protocol reached Location active; there is no owner-confirmed phone-visible observation for this run. |
| GUI disconnect recovery | PASS | UI reported the selected USB iPhone was no longer connected and retained the last applied location. |
| Pending-restore close dialog | PASS | 21 frontend tests/build pass; Keep open dismissed, X reopened the actual dialog, explicit Close with restore pending exited within 621 ms, the second close via Close options passed, and relaunch loaded the reminder. |
| Crash or lost-reply restoration obligation | PASS | Current source/build includes durable journal persistence, synthetic empty-helper Clear handling, and the X/Exit trap; no phone command was sent. |
| Current desktop artifact | PASS | Current executable SHA-256 `57f9893c22014410549883bc724abe020d837f61678ed5d9d4fc023dd04e2bf9`; installer SHA-256 `d939afafb07cb35d181237babec4c30c51b158cf13889b686e6e2b03b5903c9e`; build checks pass. |
| GUI retarget, Restore/Clear and phone-visible restoration | PASS | User-operated current-release follow-up covered Times Square, pending-close journal retention with zero owned portable processes, reopen, USB reconnect, Restore/Clear, Ready status and banner disappearance. The owner observed real home location after unplug; this remains scoped to the tested device/session. |
| Active-session close latency and physical cleanup | PASS | Explicit pending-close choice exited within 621 ms in the current portable flow; the physical follow-up confirmed journal retention with zero owned portable processes. |
| Native-engine USB sequence | PASS | Prior owner-confirmed packaged run covers Teleport, route, pause, faster resume, joystick, Restore and shutdown for the recorded iPhone/iOS combination. |
| Desktop Wi-Fi preparation | PASS | USB preparation and RemotePairing verification are recorded in `wireless-preparation-2026-09-08.json`. |
| Desktop Wi-Fi control | NEEDS INVESTIGATION | Cable-free attempt returned `WIRELESS_DEVICE_NOT_ADVERTISING`; no location operation occurred and the cause is undetermined. |
| Windows installer build artifact | PASS | Final installer build, workspace binding and offline verification pass; SHA-256 `d939afafb07cb35d181237babec4c30c51b158cf13889b686e6e2b03b5903c9e`. |
| Windows clean install/upgrade and installed GUI parity | NEEDS INVESTIGATION | Existing-host install/upgrade evidence passes in [windows-installed-2026-09-11.json](../tests/bundle/windows-installed-2026-09-11.json); clean-machine prerequisites and acceptance remain unverified. |
| macOS package and physical acceptance | BLOCKED — REMOTE AUTHORIZATION REQUIRED | Requires Mac/Xcode, Apple SDK and authorized signing team; no Apple-platform execution is available here. |
| Independent native iPhone build, signing and physical execution | BLOCKED — REMOTE AUTHORIZATION REQUIRED | Mobile core 10 tests and signing check pass; Xcode build, signed iOS app, device execution and cellular/unrelated-Wi-Fi behavior remain pending. |
| Cellular and unrelated-Wi-Fi phone-only operation | BLOCKED — REMOTE AUTHORIZATION REQUIRED | Depends on the signed independent iPhone app and Apple entitlements; no accepted IPA exists. |
| Full release notices, signing, updater and website | NEEDS INVESTIGATION | Runtime notices exist, but complete artifact-derived review and public release gates remain open. |

The 11 September offline bundle report records the current portable fingerprint. Any later artifact edit requires a new verification report before packaging.
