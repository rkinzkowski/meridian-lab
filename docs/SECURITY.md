# Security boundaries and remaining work

Meridian is intended for authorized development and QA on the owner's devices. It does not include jailbreaks, signing bypasses, anti-detection behavior, or copied pairing credentials.

## Implemented boundaries

- The renderer calls typed Tauri commands. No shell/filesystem/process plugin is exposed to it, and it cannot choose a Python executable in production.
- Rust starts one owned runtime directly with structured arguments and private pipes. JSON lines have a size limit, correlation checks, typed errors, and deadlines. There is no unauthenticated local HTTP control server.
- Real operations require an explicit device identity. RSD identity is checked before opening DVT and during RemotePairing preparation.
- Coordinates, route size/speed, GPX length, and explicit RSD inputs are validated. The GPX parser rejects document types/entities and separate track segments rather than joining gaps.
- Native generations and operation serialization coordinate writes. Stopped routes are retired before Clear. The browser preview cannot silently substitute for a failed native device.
- Upstream human logging is disabled in the production entry point; diagnostics use selected fields instead of raw exception text or pair records.
- Windows helpers use a job object and kill-on-close. Shutdown requests Clear/close first; forced helper termination is not proof of physical restoration.

The Tauri CSP limits executable content to the application and allows map networking to OpenFreeMap. Inline styles and blob workers are allowed for the map renderer. The capability file contains `core:default`; no shell, arbitrary filesystem, or generic HTTP plugin is registered. Review this boundary again when adding plugins or providers.

## Limitations

This repository has not received an independent security audit. Native command types are independently maintained in Rust and TypeScript rather than generated from a single schema. Release signing, installer verification, update signing, dependency-license completeness, and SBOM generation are unfinished.

Places/history and pairing indexes are not encrypted at rest. The index contains remote identifiers, and upstream credential storage is not wrapped in an application-specific OS secret store. Diagnostics can include adapter names and a stable redacted device label; review them before sharing.

Library writes are not a database transaction. The native JSON store syncs a same-directory temporary file before atomic replacement and deliberately keeps no duplicate-history backup. Explicit corrupt-library recovery preserves the damaged bytes before creating an empty library. General schema migration and a full power-loss test are absent. A local storage failure can occur after a device write succeeds; the current command path must not be used to infer that the device write failed solely because history saving failed.

The recovery supervisor is incomplete. Explicit recovery can reconstruct an in-memory held point/checkpoint; automatic retry budgets and every cancellation/teardown race need further validation. A crash or cable loss can leave physical restoration unconfirmed.

## Release review

Before distribution, inspect actual bundled files for credentials and unexpected modules, retain third-party notices, create an artifact-derived SBOM, sign the final executables/installers, and test normal and forced shutdown on clean machines. See [Releasing](RELEASING.md). Do not broaden firewall rules or disable operating-system security to conceal a failed test.

Primary references: [Tauri security](https://v2.tauri.app/security/), [Tauri capabilities](https://v2.tauri.app/security/capabilities/), [Tauri content security policy](https://v2.tauri.app/security/csp/).
