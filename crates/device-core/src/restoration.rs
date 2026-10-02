//! Private, fail-closed persistence for an outstanding real-location restore.
//!
//! The journal intentionally stores no coordinates. Its presence means that a
//! previous session may have applied a simulated location and that Clear must
//! be confirmed before the device is treated as restored.

use meridian_shared_types::{ConnectRequest, TypedError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::io::Write;
use std::sync::Arc;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct RecoveryJournal {
    pub(crate) schema_version: u32,
    pub(crate) request: ConnectRequest,
}

#[derive(Debug, Clone)]
pub(crate) struct JournalStore {
    path: PathBuf,
    _lock: Option<Arc<std::fs::File>>,
    lock_error: Option<TypedError>,
}

impl JournalStore {
    pub(crate) fn new(path: PathBuf) -> Self {
        let lock_path = path.with_extension("lock");
        let result = (|| {
            if let Some(parent) = lock_path.parent() {
                std::fs::create_dir_all(parent).map_err(|_| io_error())?;
            }
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(lock_path)
                .map_err(|_| io_error())?;
            file.try_lock().map_err(|error| match error {
                std::fs::TryLockError::WouldBlock => lock_error(),
                std::fs::TryLockError::Error(_) => io_error(),
            })?;
            Ok(Arc::new(file))
        })();
        match result {
            Ok(lock) => Self { path, _lock: Some(lock), lock_error: None },
            Err(error) => Self { path, _lock: None, lock_error: Some(error) },
        }
    }

    pub(crate) fn load(&self) -> Result<Option<RecoveryJournal>, TypedError> {
        self.ensure_lock()?;
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(io_error()),
        };
        let journal: RecoveryJournal =
            serde_json::from_slice(&bytes).map_err(|_| invalid_error())?;
        validate(&journal)?;
        Ok(Some(journal))
    }

    pub(crate) fn arm(&self, request: &ConnectRequest) -> Result<(), TypedError> {
        self.ensure_lock()?;
        request.validate().map_err(|_| invalid_error())?;
        if let Some(existing) = self.load()? {
            if !same_request(&existing.request, request) {
                return Err(conflict_error());
            }
        }
        self.write(&RecoveryJournal {
            schema_version: SCHEMA_VERSION,
            request: request.clone(),
        })
    }

    pub(crate) fn clear(&self) -> Result<(), TypedError> {
        self.ensure_lock()?;
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(io_error()),
        }
    }

    fn write(&self, journal: &RecoveryJournal) -> Result<(), TypedError> {
        validate(journal)?;
        let parent = self.path.parent().ok_or_else(io_error)?;
        std::fs::create_dir_all(parent).map_err(|_| io_error())?;

        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| io_error())?
            .as_nanos();
        let temp = self
            .path
            .with_extension(format!("pending-{}-{}", std::process::id(), nonce));
        let bytes = serde_json::to_vec(journal).map_err(|_| invalid_error())?;
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .map_err(|_| io_error())?;
            file.write_all(&bytes).map_err(|_| io_error())?;
            file.sync_all().map_err(|_| io_error())?;
            drop(file);
            replace_file(&temp, &self.path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }

    fn ensure_lock(&self) -> Result<(), TypedError> {
        self.lock_error.clone().map_or(Ok(()), Err)
    }
}

fn validate(journal: &RecoveryJournal) -> Result<(), TypedError> {
    if journal.schema_version != SCHEMA_VERSION || journal.request.validate().is_err() {
        return Err(invalid_error());
    }
    Ok(())
}

fn same_request(left: &ConnectRequest, right: &ConnectRequest) -> bool {
    serde_json::to_vec(left).ok() == serde_json::to_vec(right).ok()
}

fn replace_file(temp: &Path, destination: &Path) -> Result<(), TypedError> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io_error());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::rename(temp, destination).map_err(|_| io_error())
    }
}

fn invalid_error() -> TypedError {
    TypedError::new(
        "RESTORE_JOURNAL_INVALID",
        "The restore journal is malformed or unsupported.",
        "Preserve the journal and inspect it before retrying device recovery.",
        false,
    )
}

fn io_error() -> TypedError {
    TypedError::new(
        "RESTORE_JOURNAL_IO",
        "The restore journal could not be read or saved.",
        "Check local storage permissions and retry without discarding the pending restore.",
        true,
    )
}

fn lock_error() -> TypedError {
    TypedError::new(
        "RESTORE_JOURNAL_LOCKED",
        "Another Meridian process owns the restore journal.",
        "Close the other Meridian process before retrying recovery.",
        true,
    )
}

fn conflict_error() -> TypedError {
    TypedError::new(
        "RESTORE_JOURNAL_CONFLICT",
        "A restore is pending for a different selected device.",
        "Restore the pending device before selecting another device.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    fn root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "meridian-restore-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn request(transport: meridian_shared_types::Transport) -> ConnectRequest {
        ConnectRequest {
            udid: "test-device".into(),
            transport,
            rsd_address: None,
            rsd_port: None,
        }
    }

    fn cleanup(path: &Path) {
        let _ = std::fs::remove_dir_all(path);
    }

    #[test]
    fn arm_overwrites_and_loads_without_coordinates() {
        let root = root("overwrite");
        let store = JournalStore::new(root.join("restoration-journal.json"));
        store
            .arm(&request(meridian_shared_types::Transport::Usb))
            .unwrap();
        store
            .arm(&request(meridian_shared_types::Transport::Usb))
            .unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.schema_version, SCHEMA_VERSION);
        assert_eq!(
            loaded.request.transport,
            meridian_shared_types::Transport::Usb
        );
        drop(store);
        cleanup(&root);
    }

    #[test]
    fn arm_does_not_replace_a_different_pending_device() {
        let root = root("conflict");
        let store = JournalStore::new(root.join("restoration-journal.json"));
        store
            .arm(&request(meridian_shared_types::Transport::Usb))
            .unwrap();
        let mut changed = request(meridian_shared_types::Transport::Usb);
        changed.udid = "other-device".into();
        assert_eq!(
            store.arm(&changed).unwrap_err().code,
            "RESTORE_JOURNAL_CONFLICT"
        );
        assert_eq!(
            store.load().unwrap().unwrap().request.udid,
            "test-device"
        );
        drop(store);
        cleanup(&root);
    }

    #[test]
    fn truncated_record_fails_closed_and_is_preserved() {
        let root = root("truncated");
        let store = JournalStore::new(root.join("restoration-journal.json"));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("restoration-journal.json");
        std::fs::write(&path, b"{\"schemaVersion\":1").unwrap();
        assert_eq!(
            store.load().unwrap_err().code,
            "RESTORE_JOURNAL_INVALID"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"{\"schemaVersion\":1"
        );
        drop(store);
        cleanup(&root);
    }

    #[test]
    fn clear_is_idempotent() {
        let root = root("clear");
        let store = JournalStore::new(root.join("restoration-journal.json"));
        store
            .arm(&request(meridian_shared_types::Transport::Usb))
            .unwrap();
        store.clear().unwrap();
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
        drop(store);
        cleanup(&root);
    }

    #[test]
    fn write_failure_does_not_replace_existing_journal() {
        let root = root("write-failure");
        std::fs::create_dir_all(&root).unwrap();
        let blocker = root.join("blocker");
        std::fs::write(&blocker, b"file").unwrap();
        let store = JournalStore::new(blocker.join("restoration-journal.json"));
        assert_eq!(
            store
                .arm(&request(meridian_shared_types::Transport::Usb))
                .unwrap_err()
                .code,
            "RESTORE_JOURNAL_IO"
        );
        drop(store);
        cleanup(&root);
    }

    #[test]
    fn second_store_is_locked_until_first_store_drops() {
        let root = root("lock");
        let path = root.join("restoration-journal.json");
        let first = JournalStore::new(path.clone());
        first.arm(&request(meridian_shared_types::Transport::Usb)).unwrap();
        let second = JournalStore::new(path.clone());
        assert_eq!(second.load().unwrap_err().code, "RESTORE_JOURNAL_LOCKED");
        drop(second);
        drop(first);
        let third = JournalStore::new(path.clone());
        assert!(third.load().unwrap().is_some());
        drop(third);
        cleanup(&root);
    }
}
