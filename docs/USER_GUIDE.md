# Using the Windows development build

Install with `MeridianLab-0.1.0-Windows-Setup.exe`, or open `Meridian Lab.exe` in the complete portable folder. Keep its runtime and loader alongside it. This is an unsigned development build.

## Explore without a phone

Open **Settings → Enter Demo Device**, or use the mode control in the header. Demo changes only simulated application state. Choose a landmark or enter coordinates, then Teleport. Restore clears the demo location. Routes accepts waypoints or GPX and supports Run, Pause, Resume, speed and loop settings. Saved routes can be reopened, edited and deleted. Joystick starts from an applied location and stops keyboard movement when the key is released or the window loses focus.

Saved places and history remain local. Invalid coordinate drafts cannot be applied or saved. If saved data is corrupt, the app keeps controls accessible and offers explicit preservation and recovery; recovery keeps the damaged bytes in a separate file before creating an empty library.

Street maps are optional and request the visible area from OpenFreeMap. The coordinate grid and small bundled landmark list are available offline. General address search and a complete offline street atlas are not included.

## Use USB when phone testing resumes

Exit Demo Device, open **Devices**, select the development-enabled iPhone and connect over USB. Trust and developer support have separate actions. Choose a destination, then Teleport. **Restore Real Location** sends Clear; check the phone's location app for visible restoration. The packaged native engine passed Teleport, route, pause/resume, speed, joystick and Restore on one iPhone 17 Pro Max / iOS 27.0 beta. Graphical end-to-end acceptance of the installed app remains separate.

After a failed Restore, use **Reconnect and restore**. This reconnects to clear the previous simulation without replaying its retained position. If you close while restoration is still pending, Meridian requires an explicit **Close with restore pending** choice and persists the reminder for the next launch. This records the obligation; it does not confirm that the phone restored.

## Desktop Wi-Fi and the independent iPhone app

Desktop Wi-Fi needs the laptop and phone on a reachable local network. Preparation succeeded in the recorded run, but the cable-free connection could not discover the phone's advertised developer service. The updated runtime's TLS dependency passes its self-test; that does not establish a working wireless session.

The independent native iPhone application is a different path: its intended operation requires no running laptop or shared LAN after initial provisioning. It is still in development and is not yet supplied as an accepted signed IPA. See [iOS support](IOS_SUPPORT.md), [audit](FINAL_AUDIT.md) and [build status](BUILD_STATUS.md).
