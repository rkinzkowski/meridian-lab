# Owned developer-service protocol

The desktop Rust process launches one private Python child with piped JSONL. On Windows, an owned Job Object contains the child tree. No shell commands or public listener expose device controls. The parent and child agree on request IDs, typed errors and bounded line sizes. Requests are `{ "requestId": "...", "method": "...", "params": {} }`; responses contain exactly a matching identity and success result or typed error. Duplicate request IDs with the same payload return a cached reply; conflicting payloads fail. Pending work and the replay cache are bounded.

Explicit methods cover runtime info/shutdown, device enumeration/trust/developer support, session open/health/close, Set/Clear, wireless discovery and pairing preparation. Cancellation is dispatched independently of the child's serialized device work. Parent deadlines exceed child operation/cleanup deadlines.

An explicit USB identity is checked in Lockdown, then again at RSD before DVT opens. On Windows/Linux iOS 17.4+, the selected path is UserspaceRsdTunnel with implicit pairing and RemotePairing fallback disabled. macOS has a separate native-remoted path in source. Wi-Fi and explicit RSD also validate the selected identity; they do not silently fall back to an arbitrary device or transport.

DVT, LocationSimulation and DeviceInfo remain open across Set calls. A Set reply advances the acknowledged checkpoint. Apple's Clear selector is one-way; a subsequent service-health reply shows that the channel responded, while physical restoration requires a separate user observation. If restoration fails, that uncertainty remains visible. Shutdown attempts Clear before closing owned resources; cable loss or forced process death can prevent restoration.

The offline runtime `--self-test` creates only an isolated synthetic userspace stack. The desktop `--self-test` tests the bundled child boundary and explicit demo controls. Neither enumerates nor connects to a phone.
