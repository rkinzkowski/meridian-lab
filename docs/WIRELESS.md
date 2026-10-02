# Desktop wireless developer connection

This page describes a laptop-to-phone developer connection on a reachable local network. It is separate from the independent iPhone app, whose required behavior is operation without a running computer or a shared LAN. See [iPhone architecture](IOS_ARCHITECTURE.md).

The packaged Python 3.13.15 runtime passes an actual native TLS-PSK handshake self-test, including rejection of an incorrect key. The former Python 3.12 TLS extension failure is resolved. USB RemotePairing preparation passed on the selected iPhone 17 Pro Max / iOS 27.0 beta on 8 September. After USB removal, the connection failed with `WIRELESS_DEVICE_NOT_ADVERTISING` and sent no location changes.

Separate bounded discovery checks received zero Apple developer-service advertisements. The owner reported main Wi-Fi and no VPN. Exact helper allow rules existed on the laptop. The cause remains undetermined; no firewall protections were disabled or broadened. Further phone tests are paused at the owner's request. Reports: [preparation](../tests/physical-device/wireless-preparation-2026-09-08.json), [cable-free attempt](../tests/physical-device/packaged-wifi-2026-09-08.json).

## Explicit preparation

The desktop exposes **Prepare Wi-Fi** for an open, trusted USB session with no held simulated point. Restore location first. Preparation:

1. Verifies the selected USB identity and enables the device's Wi-Fi connection preference through Lockdown.
2. Opens a separate RSD connection over the existing transport so the pairing service cannot close the persistent location session.
3. Verifies the RSD identity, performs legitimate RemotePairing with the phone's approval if required, and obtains the remote identifier.
4. Reopens with automatic pairing disabled to verify the existing relationship operationally.
5. Saves only the selected-device-to-remote-identifier index after both identity checks and verification succeed.

The index is not the pairing credential itself. The runtime does not declare a healthy relationship merely because a plist exists. Lockdown trust and RemotePairing are separate states.

## Connection path

`WirelessTunnel` browses `_remotepairing._tcp.local.` in up to three ten-second windows and uses upstream interface enumeration. Candidates are bounded and unusable addresses rejected. For a stored remote identifier it attempts pair verification, opens a TCP userspace CoreDevice tunnel, establishes RSD through the userspace dial plane, and returns it for selected-device identity validation before DVT opens.

The UI currently selects a device through USB enumeration. A known-device picker that persists across relaunch without a cable is missing, so the normal experimental sequence is prepare while on USB, retain the selected device, disconnect the session, then explicitly request Wi-Fi. Switching a healthy active session between transports is not automatic.

## Failure and recovery limits

Typed failures distinguish unprepared Wi-Fi, no advertisements, pair-verification rejection, tunnel reachability, RSD failure, identity mismatch, and dropped developer sessions.

The pinned upstream has a one-second inner socket connection deadline on the audited RemotePairing path. This implementation uses bounded attempts and an outer deadline; it does not replace upstream's inner deadline. Slow first-use/local-network conditions remain a physical-test risk.

The desktop health loop has a confirmation ratchet: a never-confirmed session has a 30-second limit, and a confirmed session requires four consecutive failed probes before entering recovery. A successful probe resets the failure count. These checks do not establish physical Wi-Fi compatibility.

After connection loss, the renderer's **Reconnect session** action retries the selected device and reapplies its last acknowledged point; recovered routes remain paused. After a failed Restore, **Reconnect and restore** reconnects only to issue Clear and does not replay the retained point or route. Automatic bounded restart and persisted checkpoints across application restart remain incomplete.

The Python adapter configures upstream's process-global `USE_USERSPACE_TUNNEL` factory flag while owning a single tunnel and restores it on close. The one-tunnel ownership rule must remain in place until upstream supplies an instance-scoped equivalent.

## Physical acceptance still required

Record USB preparation and phone approval, verified Wi-Fi open, Teleport/Clear, cable removal, multi-adapter discovery, Wi-Fi loss/reconnect, asleep/waking phone behavior, delayed discovery, denied pairing, route checkpoint recovery, and shutdown without owned processes. Record service responses separately from observed location behavior. Do not use a successful Bonjour browse or demo route as this evidence.

Primary references: [pymobiledevice3 source and releases](https://github.com/doronz88/pymobiledevice3), [upstream remote services](https://github.com/doronz88/pymobiledevice3/tree/master/pymobiledevice3/remote).
