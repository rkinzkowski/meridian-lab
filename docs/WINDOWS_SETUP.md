# Windows setup

The packaged native Windows engine passed USB Teleport, routes, pause/resume, speed, joystick, Restore and shutdown on the recorded iPhone 17 Pro Max / iOS 27.0 beta. The owner confirmed the visible behavior. Installed graphical acceptance and clean-machine prerequisites remain separate checks; see the [audit](FINAL_AUDIT.md).

## Prepare the phone and Apple stack

1. Install Apple's current [Apple Devices application](https://support.apple.com/guide/devices-windows/welcome/windows) if the machine has no working Apple device stack, then finish its first-run setup.
2. Connect the intended iPhone using a data-capable USB cable. Unlock it and complete **Trust This Computer** on the phone when prompted.
3. Enable Developer Mode using Apple's [Developer Mode instructions](https://developer.apple.com/documentation/xcode/enabling-developer-mode-on-a-device). Complete any phone restart and confirmation that Apple requires.
4. In Meridian's **Devices** page, select the intended phone and choose **Connect USB**. If trust is required, use **Trust Computer**, accept the phone prompt, then connect again.
5. If a typed developer-support error requires it, disconnect and choose **Prepare Developer Support**. A mounted image does not itself make the app Ready; connect again to open actual developer services.

Enumeration uses the usbmux protocol. A detected Apple package or an open TCP port is not treated as a completed developer session. The current UI polls device enumeration but does not yet discover/launch Apple Devices through its installed package identity.

## Teleport and restore

Select a map point or enter latitude/longitude. Teleport is enabled for an open developer session or an explicitly selected demo. **Restore real location** sends Clear through the held session; **Disconnect** also closes owned resources. Check the phone's actual location display during acceptance tests.

If restoration fails, the backend keeps a failure state. Reconnect the intended device, retry restoration, and verify the phone. Do not interpret closing the UI or killing a process as a confirmed Clear.

## Current transport and host limits

- The current Windows USB path uses upstream's userspace tunnel for iOS 17.4 and newer. The iOS 17.0–17.3 USB path is deliberately rejected with `LEGACY_TUNNEL_REQUIRED`; a privileged compatibility tunnel has not been implemented.
- The actual recorded device is iPhone 17 Pro Max / iOS 27.0 beta. This is not a compatibility guarantee for all iOS 17+ devices or future betas.
- Diagnostics distinguish absent, present, and unreadable `DisabledComponents` values and report adapter address-family counts. They do not implement adapter-binding repair, tunnel-interface verification, or automatic reboot decisions.
- Meridian currently does not modify Windows Firewall or install firewall rules. The requested narrow firewall preflight/recovery service remains unfinished. Do not disable the firewall to make a test pass.
- A device seen in enumeration can still be locked, untrusted, or missing developer services. Follow the typed recovery action rather than assuming a connection failure is a driver failure.

## Development build requirements

Use [Development](DEVELOPMENT.md) for Node, pnpm, Python, and Rust commands. Standard Tauri Windows prerequisites include the native C++ toolchain and WebView2; see the [official Tauri prerequisite guide](https://v2.tauri.app/start/prerequisites/). The current local GNU-linker investigation is not an end-user installation method.
