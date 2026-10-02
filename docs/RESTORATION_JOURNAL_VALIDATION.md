# Restoration journal validation

Date: 2026-09-10

The release device-core validation used the bundled Rust 1.98.1 toolchain through `work/rust-task.ps1 test-release` with the mobile and desktop crates excluded. It completed 45 tests with 45 passed and 0 failed, including the coordinator recovery cases and journal lock, malformed-record, atomic-write, clear, conflict, and write-failure cases.

The workspace clippy check used `work/rust-task.ps1 clippy` with `-D warnings`. It completed successfully for the checked native crates, including `meridian-device-core` and `meridian-mobile-core`.

The root manifest now declares Rust 1.89 because the journal uses the standard library's non-blocking file lock API. Rust 1.89 compatibility was not separately built on this host; the recorded checks use Rust 1.98.1.

The journal contains only the selected reconnect request and schema version. It stores no coordinates, location history, pairing credentials, or export data. Its persistent lock file is created in the same private application-data directory using the host's normal file permissions.
