# Known issues and verification limits

Checkpoint: 11 September 2026. The entire product specification is not complete. The [final audit](FINAL_AUDIT.md) records this pass; earlier reports remain historical evidence and must not override newer observations.

## Verified physical behavior

The packaged native Windows engine and Python 3.13 runtime passed USB Teleport, route progress, pause, faster resume, joystick handoff, Restore and shutdown. The owner confirmed all visible phases. An earlier packaged Python 3.12 test also passed two-location retargeting; the owner noticed no jump in that run. Neither test was an automated click-through of the installed Tauri UI. See the dated reports under `tests/physical-device`.

## Remaining acceptance gates

| Area | Remaining condition |
|---|---|
| Native iPhone, no computer/shared LAN | Validate the local self-device/Network Extension source on Apple platforms, legitimately provision/install, then prove cellular and unrelated-Wi-Fi behavior. No signed IPA or physical mobile acceptance is available |
| Desktop Wi-Fi | USB preparation passed; cable-free discovery returned no developer-service advertisements. Pair-verify/tunnel/Teleport/Clear over Wi-Fi remain unverified. The earlier Python 3.12 TLS import defect is fixed in the Python 3.13 bundle |
| Windows graphical acceptance | The current portable GUI opens its embedded UI, renders street maps, loads a persisted real pending restore after legacy upgrade, and the user-operated follow-up passed Times Square, explicit pending close/journal retention, reopen, USB reconnect, Restore/Clear and Ready/banner disappearance for the tested session |
| Clean Windows machine | Existing-host installer checks do not prove first-run prerequisites, Apple driver setup, permissions or behavior on a clean machine |
| macOS | No native build, frozen runtime, signed/notarized package or device acceptance on macOS; Python lock is Windows-specific |
| Interruption/recovery matrix | Automated failure handling passes, but no physical cable-loss, sleeping-device, crash, stale pairing or wrong-device matrix has been completed |

No reason for the missing Wi-Fi advertisements has been established; the phone was reported on the main Wi-Fi without VPN, and exact helper firewall allow rules existed. Do not label this a proven firewall or router defect.

The former packaged dependency on a Vite development server and the missing map-worker issue are fixed in the inspected 11 September executable. The refreshed installer and existing-host upgrade passed; clean-machine acceptance remains unverified.

The final existing-host installer evidence is recorded in `tests/bundle/windows-installed-2026-09-11.json`; clean-machine prerequisites and acceptance remain unverified.

## Product work still missing or limited

- The USB adapter rejects iOS 17.0–17.3; a compatible legacy tunnel is absent. Evidence on one iOS 27 beta does not prove all versions or models.
- Desktop Apple-account, two-factor, team/certificate selection, complete signing/install/bootstrap and app repair screens are absent. The optional native signing adapter compiles but has not performed an account operation.
- Automatic recovery budgets and seamless transport migration remain incomplete. The current build includes the durable pending-restore journal, synthetic empty-helper Clear handling and Windows X/Exit trap. The tested user-operated session confirmed real-location return after unplug and explicit reconnect/Restore/Clear cleanup; broader device and transport coverage remains open.
- Windows Apple Devices package launch/detection, full IPv6 repair, narrowly scoped firewall preflight and privilege/reboot workflows remain unfinished. Existing diagnostics are read-only.
- Local library storage is JSON. Explicit corrupt-data preservation/recovery is implemented; database migration, encryption, device nicknames, full settings persistence and a remembered-device picker across relaunch are absent.
- Offline mapping is a coordinate grid and a small landmark list. General address geocoding, style selection and licensed satellite imagery are not wired into the Windows UI.
- Windows route geometry uses straight segments or imported GPX. Road-aware calculation, timestamp playback, schedules, drag reordering and deterministic QA noise modes are absent. Saved-route identity, speed, loop and deletion are implemented.
- History persistence can fail after a successful location command. The app reports that distinction; a history error is not proof the phone rejected the command.
- Signed updater, public hosting, marketing website, full artifact-derived SBOM and release signing are unfinished. Runtime distribution notices are collected, but that is not a complete release license review.

## Restoration and privacy

A disconnected phone can prevent confirmed Clear. The app retains restoration uncertainty, persists an explicit pending-restore journal before an opted-in close, and attempts owned cleanup; process termination or a journal entry is never proof that real location returned.

Diagnostics omit raw device identities and coordinates but can contain stable device pseudonyms and adapter names. Pairing records/indexes remain private local data and must not enter exports. See [Security](SECURITY.md), [Privacy](PRIVACY.md), and [Third-party licenses](THIRD_PARTY_LICENSES.md).
