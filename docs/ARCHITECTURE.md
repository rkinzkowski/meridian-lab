# Architecture

## Execution boundary

```text
React desktop UI
  → typed Tauri commands / runtime-status events
  → Rust Coordinator + Route + LibraryStore
  → owned JSON-lines child process
  → pymobiledevice3 selected-device transport
  → RSD → DVT → one LocationSimulation channel
```

`apps/desktop/src/api.ts` selects the native Tauri adapter inside the desktop shell. The renderer receives high-level state; it has no shell, process, pairing-file, usbmux, or tunnel API. `packages/shared-types` describes the command and state contracts. Rust's corresponding types live in `crates/shared-types`.

The browser preview has a separate `BrowserPreviewApi`. It starts in hardware mode with no device and rejects device actions until the user explicitly selects demo. It is not a fallback after native hardware errors.

## State and writers

`crates/device-core` owns connection status, selected device/transport, operation serialization, generation IDs, and acknowledged route state. The desktop shell emits `runtime-status` every 200 ms, requests route ticks every 200 ms, and attempts health checks every five seconds. These intervals are implementation details, not physical-device reliability guarantees.

`crates/route-engine` validates points/speeds, calculates distance, proposes sampled positions, and advances acknowledged progress only after a successful write. Pause holds the last acknowledged point. Retarget/Clear retires the route writer. Explicit recovery retains in-memory checkpoint information and leaves a recovered route paused. There is no persisted crash checkpoint across application restart or automatic bounded restart supervisor yet.

Setup stages are derived from backend measurements. A successful DVT/location-service open is the readiness gate; image mount and trust alone are insufficient. Some requested prerequisite measurements are still absent; see [Known issues](KNOWN_ISSUES.md).

## Python process contract

The child communicates only over owned stdin/stdout JSON lines. Requests use `requestId`, `method`, and `params`. Replies preserve the request ID and contain either `ok: true` with `result`, or `ok: false` with a typed error. Human logs are not protocol state. Messages have a 1 MiB cap, request validation, deadlines, and duplicate-request handling; cancellation is a separate protocol method.

The production adapter accepts an explicit UDID and transport. On the current Windows USB path it uses upstream `UserspaceRsdTunnel`; on macOS it selects `NativeRemotedTunnel`. Explicit RSD is available to the runtime/probe, with identity verification and no automatic fallback. The UI currently exposes USB and Wi-Fi only.

RSD identity must match the intended device before DVT is opened. The Python object retains RSD, DVT, LocationSimulation, and a DeviceInfo health service until teardown. Set replies and Clear-plus-health checks describe developer-service communication; they do not observe CoreLocation in another application on the phone.

## Ownership and persistence

Rust spawns the configured bundled executable directly without a shell. Debug builds alone accept `MERIDIAN_PYTHON`. Windows child containment uses a job object with kill-on-close; orderly shutdown first requests restoration and cleanup. Forceful process death can prevent a Clear message from reaching the phone, so forced termination is not reported as confirmed restoration.

Desktop places, routes, history, and the history preference are stored in application-data `library.json`. The browser uses its own versioned local-storage record. SQLite, schema migrations for the native library, encryption at rest, device nicknames, and full settings persistence are not implemented.

The Python runtime keeps a remote-pairing identifier index in its own application-data directory; upstream owns actual pairing records. These are separate from the places/history library.

## Native iPhone boundary

`crates/mobile-core` contains an embedded Rust service path and FFI-oriented types. `crates/ios-bootstrap` contains a separate installation/container boundary. They do not turn the desktop application into a verified native iPhone app. Consult the mobile documentation supplied with the iOS work for its exact build and provisioning status.

Primary references: [Tauri commands](https://v2.tauri.app/develop/calling-rust/), [pymobiledevice3](https://github.com/doronz88/pymobiledevice3), [idevice](https://github.com/jkcoxson/idevice).
