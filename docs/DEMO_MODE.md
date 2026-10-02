# Demo Device

Demo is an explicit user choice for interface exploration and automated UI checks. It never proves physical-device compatibility.

## Use it

1. Start the browser preview with `pnpm dev`, or open a development desktop build.
2. Open **Settings** and choose **Enter Demo Device**, or use the mode control in the header.
3. Confirm the **DEMO DEVICE** banner before testing Teleport, routes, GPX, joystick, saved places, or history.
4. Choose **Exit Demo Device** to return to hardware mode. Hardware operations then require a real desktop developer connection.

The browser starts with no connected phone and rejects Teleport in hardware mode. It cannot access USB, pair a phone, or spawn the developer runtime. Native hardware errors remain errors; they do not enable demo automatically.

The native demo transition first attempts to restore/close any real session. If restoration fails, the transition fails so a simulated UI state cannot conceal an unconfirmed physical restoration.

## Differences from hardware

The browser adapter keeps its own simulated state and route timer. The native demo adapter exercises the Rust coordinator without talking to Apple services. Neither models every lock, trust, DDI, Wi-Fi, process-crash, or network failure. A preview result cannot stand in for those integration tests.

`physicalDeviceVerified` remains false. Demo history entries are labeled `demo`; the native and browser libraries are separate stores. Online street maps, if explicitly enabled, still make real provider requests even though the device is fake.

Before taking screenshots, keep the demo banner visible. Do not label a demo screenshot as a connected physical iPhone or count it in the compatibility matrix.
