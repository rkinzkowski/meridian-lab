# Presigned iPhone bootstrap

On a Mac with Xcode, a legitimate development team, and an already trusted USB pairing, build/export the IPA and its JSON `ArtifactManifest`. Then run the optional device feature:

```text
cargo run --locked --release -p meridian-ios-bootstrap --features device -- list
cargo run --locked --release -p meridian-ios-bootstrap --features device -- install --ipa MeridianLab.ipa --manifest MeridianLab.manifest.json --udid <explicit-id>
```

`list` prints only numbered aliases and transport kinds. The explicit identifier is input-only and is never printed. `install` verifies the manifest, installs the presigned IPA, exports a same-device validated local CoreDevice configuration, and commits it through House Arrest with readback. For an already installed app, use `setup --manifest ... --udid ...`; its manifest bundle identifier must match the installed app.

This CLI does not authenticate an Apple account, sign binaries, invent pairing records, launch the app, or change location state. A trusted physical connection and authorized provisioning remain required.
