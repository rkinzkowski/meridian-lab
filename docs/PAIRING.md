# Pairing and device identity

## Lockdown trust

The initial USB relationship is established with Apple's ordinary **Trust This Computer** flow. Enumeration and ordinary connection attempts use `autopair=False`; listing devices must not silently prompt or choose a different phone.

The explicit **Trust Computer** action uses the selected UDID. If the session is not paired, it creates one fresh legitimate host identity, calls upstream pairing with a deadline, and validates that pairing. It does not delete unrelated host records or loop after denial. An active location session blocks this preparation path.

This bounded stale-record repair is source implemented. The successful USB test used the owner's existing trusted device; a complete reset/InvalidHostID/denied-trust physical matrix remains unverified.

## RemotePairing

RemotePairing for modern wireless developer tunnels is independent of Lockdown trust. **Prepare Wi-Fi** uses a working USB session, verifies the selected identity, completes the RemotePairing handshake, and then reopens with `autopair=False` to verify the saved relationship. See [Wireless](WIRELESS.md).

RSD peer mismatch returns `DEVICE_IDENTITY_MISMATCH` before DVT access or remote-index persistence. An explicit RSD endpoint never falls back to another device or discovery method. This rejection is covered by a Python fake-adapter test; physical wrong-device testing is not claimed.

## Local records

The Python runtime records remote-identifier mappings in:

- Windows: `%LOCALAPPDATA%\MeridianLab\runtime\pairing-index.json`
- Default non-Windows fallback: `~/.local/share/MeridianLab/runtime/pairing-index.json`

Keys are short SHA-256-derived device labels; values are actual remote pairing identifiers. These labels are pseudonyms, not anonymization. The actual pairing credentials remain in the locations used by the pinned upstream/Apple stack. Do not include either credential files or the identifier index in support exports.

The index is written through an exclusive temporary file, flushed and replaced after operational verification. Current record storage does not implement an application keychain wrapper, DPAPI encryption, audited Windows ACL installation, or a secure delete flow. OS account access protections still matter.

## Native application bootstrap

The bootstrap crate is a separate implementation boundary for installed-app identity and app-container operations. The desktop does not yet provide Apple account authentication, 2FA/team/certificate selection, a completed signing workflow, or automatic pairing delivery/verification through the normal UI. A source scaffold is not an installed, trusted iPhone application.

Primary references: [Apple trust guidance](https://support.apple.com/en-us/109054), [pymobiledevice3 Lockdown implementation](https://github.com/doronz88/pymobiledevice3/blob/master/pymobiledevice3/lockdown.py).
