# Delivery audit — 11 September 2026

The Windows development build has passed offline runtime checks, a prior physical USB sequence through its native engine, and selected checks in the actual Windows interface. This does not establish completion of the full product, physical desktop Wi-Fi support, or an independently operating iPhone application.

The refreshed portable executable opened without Vite and rendered street maps. A persisted real pending-restore journal loaded after the legacy upgrade; Keep open dismissed the dialog, X reopened it, explicit Close with restore pending exited within 621 ms, and relaunch loaded the reminder again. The user-operated follow-up then covered Times Square, disconnect/pending close, journal retention, reopen, USB reconnect, Restore/Clear, Ready status and banner disappearance. This evidence is scoped to the tested device and session.

## Corrections included in the Windows build

- The desktop now enables Tauri's `custom-protocol` feature by default. Its production interface opens without a Vite development server. The desktop self-test checks that the embedded interface and map worker exist.
- MapLibre's worker is bundled explicitly. Street tiles rendered in the actual Windows window during the 11 September check.
- The frozen runtime uses Python 3.13.15 and OpenSSL 3.5.8. Its native TLS-PSK path replaces the unavailable Python 3.12 extension. The offline test completes an in-memory handshake, exchanges synthetic data, and rejects an incorrect key.
- The Python 3.13 Windows dependency lock includes the locally built, hash-pinned `lzfse` wheel. Its [source and build provenance](../python-runtime/wheels/README.md) are recorded.
- Cleanup and recovery distinguish an acknowledged location change, an uncertain Restore, and a terminated process. Restoration-only recovery reconnects to Clear without replaying the retained point or route. Repeated close requests do not bypass the pending cleanup.
- A pending restoration now has an explicit close path: the UI explains that the phone's location is unconfirmed, offers **Keep open** or **Close with restore pending**, and the native command persists the obligation before exit. This is a durable reminder contract; it is not physical proof that the phone restored.
- Bundle verification accepts both Windows executable names, requires the expected runtime and desktop checks, and hashes the complete folder before and after execution. Packaging rejects missing checks or changed bundle files.

## Evidence and its limits

| Evidence | What it establishes | What it does not establish |
|---|---|---|
| 30 Python, 45 core Rust plus clippy, 10 mobile Rust and 21 frontend tests; production TypeScript/Vite build | The recorded automated source checks pass on the Windows development host | Every device, transport, platform or physical failure mode beyond the recorded Windows/iPhone session |
| [11 September offline bundle report](../tests/bundle/windows-offline-2026-09-11.json) | Frozen prerequisites, native PSK, owned helper IPC, explicit demo control and the current bundle fingerprint | A phone connection or observed GPS movement |
| [11 September GUI report](../tests/bundle/windows-gui-2026-09-11.json) | Keep open, X dialog, explicit close timing, relaunch reminder, process cleanup and journal presence | The separate user-operated physical follow-up |
| [11 September physical follow-up](../tests/physical-device/portable-gui-followup-2026-09-11.json) | Times Square, disconnect/pending close journal retention, reopen, USB reconnect, Restore/Clear and Ready/banner disappearance in the tested session | Other devices, transports, iOS versions or a general no-flash guarantee |
| [Packaged native-engine USB report](../tests/physical-device/desktop-usb-2026-09-08.json) | On the selected iPhone 17 Pro Max / iOS 27.0 beta, the owner confirmed Teleport, route movement, pause, faster resume, joystick movement and real-location return | Automated clicks in the current graphical build, all iOS versions, or a general no-flash guarantee |
| [Earlier packaged retarget report](../tests/physical-device/packaged-usb-2026-09-08.json) | The owner confirmed both destinations and restoration and noticed no jump during that Python 3.12 run | Universal absence of a real-GPS flash or validation of every later binary |
| [USB wireless preparation](../tests/physical-device/wireless-preparation-2026-09-08.json) and [cable-free attempt](../tests/physical-device/packaged-wifi-2026-09-08.json) | Preparation succeeded; the later connection found no advertisement and failed before Set | A successful Wi-Fi tunnel, Wi-Fi location control, or a proven firewall/router cause |

The current desktop executable is `57f9893c22014410549883bc724abe020d837f61678ed5d9d4fc023dd04e2bf9`; the current installer is `d939afafb07cb35d181237babec4c30c51b158cf13889b686e6e2b03b5903c9e` (SHA-256). Both build checks passed.

## Product scope still open

The native iPhone product needs an Apple-platform build, valid provisioning and a signed installation, followed by physical proof of operation without a running computer or shared LAN. Current Rust tests and Swift/Network Extension source do not establish entitlement availability, packet routing, self-device access, background behavior or real location control. [iOS support](IOS_SUPPORT.md) records that work separately.

Desktop Wi-Fi remains unverified after zero usable developer-service advertisements. Clean-machine Windows installation, macOS packaging, the broader physical interruption/recovery matrix beyond the recorded USB unplug/reopen path, and support beyond the tested iPhone/iOS combination are also open.

The final installer passed existing-host binding and offline verification; [windows-installed-2026-09-11.json](../tests/bundle/windows-installed-2026-09-11.json) is evidence for that host only and does not establish clean-machine acceptance.

Product gaps include the complete Apple-account/two-factor/signing/install setup flow, automated transport migration and restart checkpoints, broader Windows prerequisite repair, full offline street mapping and address search, advanced route playback, and the public website/updater. The local library and route features are useful development features, but do not cover every requested persistence or simulation mode. [Known issues](KNOWN_ISSUES.md) lists the limits in detail.

The pending-restore work addresses a high-severity crash/lost-reply obligation with a durable journal and explicit close choice. Persisted-journal load, dialog behavior, 621 ms explicit close, relaunch reminder load, physical journal retention, and reconnect/Restore/Clear cleanup are observed in the recorded Windows/iPhone session. Mobile core tests and signing checks pass, while Apple-platform build, signed app, device and cellular checks remain pending.

The packages remain unsigned development artifacts. The original project license texts and complete artifact-derived third-party notice/SBOM review are unfinished. Existing Cargo declarations are recorded in [Third-party licenses](THIRD_PARTY_LICENSES.md); this audit makes no new licensing decision.

## Final assembly rule

Complete all runtime, frontend and delivery-note changes before running `tools/verify_bundle.py`. Save its report outside the portable folder. Use that fresh portable report with `tools/package_delivery.py --verification-report`; separately verify the installed executable and final graphical behavior. A passing offline check must remain labeled offline, and new physical results must record the actual build and the owner's observation.

See [AUDIT_MATRIX.md](AUDIT_MATRIX.md) for status labels and the complete current evidence boundary.
