# Remote access audit

Audit date: 2026-09-10. Scope: read-only inspection of the already configured Windows host and repository state for the authorized iPhone 17 Pro Max running iOS 27.0 beta. No USB retry, phone/network change, GPS operation, installation, credential read, or network scan was performed.

## Evidence

- [WIRELESS.md](WIRELESS.md) defines the supported desktop path as Apple's RemotePairing/CoreDevice developer tunnel over the same reachable local network. USB preparation succeeded, but the cable-free attempt failed before a session opened with `WIRELESS_DEVICE_NOT_ADVERTISING`; the recorded bounded discovery received zero usable Apple developer-service advertisements.
- [wireless preparation](../tests/physical-device/wireless-preparation-2026-09-08.json) records successful USB preparation and RemotePairing verification on the selected device. It does not prove that the phone continues advertising after cable removal.
- [packaged Wi-Fi attempt](../tests/physical-device/packaged-wifi-2026-09-08.json) records no location operation and an unverified physical GPS result. This is the last relevant wireless result.
- [PAIRING.md](PAIRING.md) states that the Windows pairing index is only a device-to-remote-identifier index. It is not the pairing credential and its presence is not readiness evidence. Actual Apple/upstream pairing records were not read.
- [ARCHITECTURE.md](ARCHITECTURE.md) and the runtime requirements identify `pymobiledevice3` 11.9.1 and Apple `idevice` RemotePairing/CoreDevice support. The UI's selected-device flow still starts from USB enumeration.

## Host state observed

The local Meridian pairing-index path exists. Its contents were not opened or copied. The Apple Lockdown directory exists, but no raw records were inspected.

Read-only command and service checks found no available `tailscale`, `zerotier-cli`, `wireguard`, `adb`, `idevice_id`, `idevicepair`, `pymobiledevice3`, `dns-sd`, `avahi-browse`, or `iproxy` command. No matching Tailscale, ZeroTier, WireGuard, Apple Mobile Device, or Bonjour service was present. The VPN-profile query was unavailable; no VPN profile or endpoint was established from this check. These checks show only that no directly callable command/service was observed on this host; they do not prove that software is absent or that an existing Apple pairing is invalid.

The repository contains no Android target or ADB workflow. Its relevant remote access is Apple's RemotePairing/Bonjour service `_remotepairing._tcp.local.` and the userspace CoreDevice tunnel.

## Viable authorized paths

1. If the phone is already prepared, awake, unlocked as required, and advertising `_remotepairing._tcp.local.` on the same reachable private network, the packaged runtime can attempt the stored selected-device relationship. The current evidence does not show that these prerequisites hold, and the UI does not yet provide a cable-independent known-device picker.
2. If the existing Apple trust and RemotePairing records remain valid, no renewed physical authorization is inherently required; the bounded next step is simply a future Wi-Fi attempt when the phone is awake and advertising on the reachable network. If those records are stale, missing, or rejected, a future physical session must reconnect the phone by USB, unlock and approve Apple's trust/RemotePairing prompts if shown, run **Prepare Wi-Fi**, verify the relationship, then remove the cable and retry Wi-Fi. The existing index file alone cannot distinguish these cases.
3. A separate independent iPhone app is not a remote-control fallback. The repository documents that its signed installation, entitlements, and physical on-device acceptance remain incomplete.

Because the phone cannot currently be reconnected and no known authorized endpoint exists, no bounded network probe was run. The prior failed discovery does not establish pairing invalidity; it leaves advertisement, reachability, sleep state, and local network behavior unresolved.

## Sanitization

This audit intentionally omits raw UDIDs, remote identifiers, pairing records, addresses, credentials, and service-instance names. Evidence references retain only repository paths and typed outcomes.
