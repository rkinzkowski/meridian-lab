# Privacy and local data

There is no Meridian account, analytics SDK, telemetry service, advertising, or location-upload endpoint in the current code. Simulation commands go to the selected phone through the local developer connection. Online resources remain optional for maps but may be required for initial developer-support preparation.

## Data kept locally

| Data | Current storage and controls |
|---|---|
| Saved places and routes | Native app-data `library.json`; selected places/GPX can be exported by the user |
| Recent Teleports | Same library, up to 500 entries; **Settings → Keep recent test locations** prevents new entries when disabled |
| Existing history | **History → Clear history** removes entries from the library; disabling history alone does not erase them |
| Browser preview library | Separate `meridian.preview.library.v1` local-storage record for that browser origin |
| Runtime pairing index | `MeridianLab/runtime/pairing-index.json` under runtime application data; contains remote identifiers |
| Pairing credentials and developer images | Upstream/Apple-managed local storage and caches |

Saved places/history are ordinary local JSON, without application encryption. Deleting entries does not promise forensic erasure from filesystem/browser backups. No cloud synchronization is implemented. Device nicknames, complete settings persistence, and a native-library migration system are still missing.

## Network use

The default map is a local coordinate grid. Offline landmark search is a small bundled list, not a complete geocoding database.

Choosing **Enable street maps** or the corresponding Settings option loads OpenFreeMap. The provider receives map-resource requests associated with the visible area and the client's network address. Selecting a distant destination can move the map and therefore change those requests. Turning maps off returns to the local grid. There is no general public geocoding endpoint enabled by default; a provider abstraction exists for future configured services.

**Prepare Developer Support** delegates necessary image acquisition to the pinned upstream implementation. It may contact upstream/Apple image services. USB RemotePairing preparation changes the phone's Wi-Fi developer-connection preference; wireless mode uses local-network discovery and device communication.

## Diagnostics and sharing

The normal diagnostics export omits coordinates, precise history, raw UDIDs, pair records, passwords, and two-factor codes. It can include platform/version information, adapter names and address-family counts, a stable device pseudonym, connection state, and typed errors. Stable redacted device labels are not anonymous. Review an export before sending it to anyone.

Physical test reports distinguish protocol replies from user observations. The helper's `--show-identities` option intentionally reveals real device identifiers locally; do not use that output as a public diagnostic artifact.

Provider information: [OpenFreeMap](https://openfreemap.org/), [OpenStreetMap attribution and data license](https://www.openstreetmap.org/copyright).
