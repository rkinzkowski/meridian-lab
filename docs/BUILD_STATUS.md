# Build checkpoint — 11 September 2026

The Windows development application has verified runtime, native-engine, and selected graphical results. The independent iPhone product and the complete requested specification remain unfinished. Results apply to the recorded build and device.

| Check | Recorded result |
|---|---|
| Python unit/session/protocol and offline TLS tests | 30 passed on Python 3.13.15 |
| Rust core, IPC, route, storage, cleanup and bootstrap tests | 45 passed in optimized builds; clippy PASS |
| Mobile Rust tests | 10 passed on Windows; no Apple-platform execution is implied |
| Frontend tests | 21 passed after adding pending-restore close dialog failure/retry coverage |
| Strict TypeScript and Vite production build | PASS after correcting audit-test types |
| Packaged interface without a Vite development server | PASS on 11 September; `custom-protocol` embeds the production assets |
| Street map in the actual Windows window | PASS on 11 September after bundling the MapLibre worker explicitly |
| Window close | PASS: explicit Close with restore pending exited within 621 ms; physical follow-up retained the journal with zero owned portable processes until reconnect and Restore/Clear |
| Frozen runtime prerequisites | PASS: Python 3.13.15, OpenSSL 3.5.8, TLS 1.2 PSK negotiation, incorrect-key rejection, isolated userspace stack |
| Desktop-owned helper IPC and explicit demo Teleport/Clear | Offline self-test PASS |
| Packaged native engine controlling the selected iPhone over USB | Prior physical PASS: Teleport, route, pause, faster resume, joystick, Restore and shutdown; owner confirmed the visible phases |
| Packaged two-location retargeting | Prior physical PASS with Python 3.12.14; owner noticed no jump in that run. This is not a general no-GPS-flash guarantee |
| Current graphical phone-control sequence | PASS within the user-operated follow-up scope: Times Square, disconnect/explicit pending close, journal retention, reopen, USB reconnect, Restore/Clear and Ready/banner disappearance were observed |
| Pending-restore close choice | PASS: Keep open dismissal, X reopening the dialog, explicit close within 621 ms, relaunch reminder load, physical journal retention and reconnect/Restore/Clear cleanup were observed |
| Desktop Wi-Fi preparation | USB preparation PASS in the recorded run |
| Cable-free desktop Wi-Fi control | UNVERIFIED: no developer-service advertisements found; connection failed before any location change |
| Windows installer | Final installer build, existing-host binding and offline verification PASS; clean-machine prerequisites and acceptance remain unverified |
| macOS application | No native build, signed/notarized package or physical acceptance |
| Independent native iPhone application | Mobile core 10 tests and signing check PASS; Xcode build, signed iOS app, device execution and cellular/unrelated-Wi-Fi behavior remain unverified |

The current desktop executable has SHA-256 `57f9893c22014410549883bc724abe020d837f61678ed5d9d4fc023dd04e2bf9`; the current installer has SHA-256 `d939afafb07cb35d181237babec4c30c51b158cf13889b686e6e2b03b5903c9e`. These hashes apply to the 11 September build.

Physical evidence: [Python 3.13 native-engine USB sequence](../tests/physical-device/desktop-usb-2026-09-08.json), [earlier packaged retarget observation](../tests/physical-device/packaged-usb-2026-09-08.json), [wireless preparation](../tests/physical-device/wireless-preparation-2026-09-08.json), and [cable-free attempt](../tests/physical-device/packaged-wifi-2026-09-08.json). Historical filenames are retained; some reports have a later UTC timestamp than the date in the filename.

The former Python 3.12 TLS extension failure is fixed by the native Python 3.13 path. It must not be used to explain the later Wi-Fi discovery failure. The cause of the missing advertisements remains undetermined. Demo, offline tests and service acknowledgements are distinct from observed phone behavior.

The live acceptance matrix is [AUDIT_MATRIX.md](AUDIT_MATRIX.md). The final GUI evidence is [windows-gui-2026-09-11.json](../tests/bundle/windows-gui-2026-09-11.json), with the user-operated follow-up in [portable-gui-followup-2026-09-11.json](../tests/physical-device/portable-gui-followup-2026-09-11.json).

See [Final audit](FINAL_AUDIT.md) and [Known issues](KNOWN_ISSUES.md) for remaining product work and evidence limits.
