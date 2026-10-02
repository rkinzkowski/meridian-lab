# Troubleshooting

- Start with the Windows installer or keep the entire portable folder together. `Meridian Lab.exe`, `WebView2Loader.dll` and the `runtime` folder are all required. The bundled application does not need a separately installed Python, Rust or Node.
- No iPhone: when device testing resumes, connect the intended phone over USB, unlock it, and finish Apple Devices setup. Select that phone on Devices.
- Trust required: choose Trust Computer and approve the prompt on that phone. Do not delete unrelated Apple pairing records.
- Developer Mode disabled: enable it on the phone, finish the restart/confirmation, and connect again. If developer support is missing, use Prepare Developer Support. The backend must actually open developer services before Ready.
- Runtime component missing: keep the bundle intact and run its offline self-test. The initial missing vendor DLL has been included in the current build.
- Wi-Fi TLS unavailable: this Python 3.12 Windows bundle cannot load the upstream TLS-PSK extension. Use USB. Building/testing a Python 3.13+ runtime is a remaining task; no global TLS patch or security setting change is used.
- Interrupted session: route progress pauses at the last acknowledged point. Reconnect Session retries the selected phone; a recovered route remains paused until Resume. Restore Real Location remains available.
- Restore failed: the app cannot claim that the phone returned to real GPS. Reconnect and retry Restore when phone testing is available. If the device cannot accept Clear, restarting the phone is the recovery described by the runtime.
- History save failed: location may already have been applied. The message explicitly distinguishes this from failure to apply location. Restore still controls the active device session.

Use Diagnostics to export the allowlisted snapshot. It omits raw pairing material, full device identities and coordinate history. A stable redacted device label and adapter names can still identify a machine, so review before sharing.
