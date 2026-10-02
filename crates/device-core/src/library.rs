use meridian_shared_types::{Library, TypedError};
use std::path::PathBuf;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};
const MAX_LIBRARY_BYTES: u64 = 32 * 1024 * 1024;
pub struct LibraryStore {
    path: PathBuf,
    write_lock: Mutex<()>,
}
impl LibraryStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            write_lock: Mutex::new(()),
        }
    }
    pub async fn load(&self) -> Result<Library, TypedError> {
        match tokio::fs::File::open(&self.path).await {
            Ok(file) => {
                let mut bytes = vec![];
                file.take(MAX_LIBRARY_BYTES + 1)
                    .read_to_end(&mut bytes)
                    .await
                    .map_err(|_| error())?;
                if bytes.len() as u64 > MAX_LIBRARY_BYTES {
                    return Err(error());
                }
                let library: Library = serde_json::from_slice(&bytes).map_err(|_| error())?;
                validate(&library)?;
                Ok(library)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Library::default()),
            Err(_) => Err(error()),
        }
    }
    pub async fn save(&self, library: &Library) -> Result<(), TypedError> {
        validate(library)?;
        let _writer = self.write_lock.lock().await;
        self.save_locked(library).await
    }
    /// Explicit repair only: preserve every original byte before replacing an
    /// unreadable library. A healthy library can never be reset by this method.
    pub async fn recover_corrupt(&self) -> Result<Library, TypedError> {
        let _writer = self.write_lock.lock().await;
        if self.load().await.is_ok() {
            return Err(TypedError::invalid(
                "The library is readable and does not need recovery.",
            ));
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| error())?
            .as_nanos();
        let backup = self.path.with_extension(format!("recovered-{stamp}.json"));
        let mut original = tokio::fs::File::open(&self.path)
            .await
            .map_err(|_| error())?;
        let mut preserved = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
            .await
            .map_err(|_| error())?;
        tokio::io::copy(&mut original, &mut preserved)
            .await
            .map_err(|_| error())?;
        preserved.sync_all().await.map_err(|_| error())?;
        drop(original);
        drop(preserved);
        let fresh = Library::default();
        self.save_locked(&fresh).await?;
        Ok(fresh)
    }
    async fn save_locked(&self, library: &Library) -> Result<(), TypedError> {
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|_| error())?;
        }
        let bytes = serde_json::to_vec_pretty(library).map_err(|_| error())?;
        if bytes.len() as u64 > MAX_LIBRARY_BYTES {
            return Err(error());
        }
        let temp = self.path.with_extension("pending");
        let mut file = tokio::fs::File::create(&temp).await.map_err(|_| error())?;
        file.write_all(&bytes).await.map_err(|_| error())?;
        file.sync_all().await.map_err(|_| error())?;
        drop(file);
        // Same-directory atomic replacement: no delete window and no duplicate history backup.
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows_sys::Win32::Storage::FileSystem::{
                MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
            };
            let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = self.path.as_os_str().encode_wide().chain(Some(0)).collect();
            if unsafe {
                MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } == 0
            {
                return Err(error());
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            tokio::fs::rename(temp, &self.path)
                .await
                .map_err(|_| error())
        }
    }
}
fn validate(library: &Library) -> Result<(), TypedError> {
    if library.places.len() > 10_000
        || library.routes.len() > 1_000
        || library.history.len() > 10_000
    {
        return Err(error());
    }
    for place in &library.places {
        if place.id.is_empty() || place.id.len() > 128 || place.name.len() > 256 {
            return Err(error());
        }
        place.point.validate()?;
    }
    for route in &library.routes {
        meridian_route_engine::Route::new(route.clone())?;
    }
    for item in &library.history {
        item.point.validate()?;
    }
    Ok(())
}
fn error() -> TypedError {
    TypedError::new(
        "LIBRARY_STORAGE_FAILED",
        "The local places and route library could not be read or saved.",
        "Check disk access and available space; existing library files are retained when possible.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use meridian_shared_types::{Point, SavedPlace};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn store() -> LibraryStore {
        LibraryStore::new(std::env::temp_dir().join(format!(
            "meridian-library-test-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    #[tokio::test]
    async fn replaces_existing_library_and_rejects_invalid_update_without_data_loss() {
        let store = store();
        let mut library = Library::default();
        library.places.push(SavedPlace {
            id: "p".into(),
            name: "Place".into(),
            point: Point {
                latitude: 10.,
                longitude: 20.,
            },
        });
        store.save(&library).await.unwrap();
        library.places[0].name = "Updated".into();
        store.save(&library).await.unwrap();
        assert_eq!(store.load().await.unwrap().places[0].name, "Updated");
        library.places[0].point.latitude = 99.;
        assert!(store.save(&library).await.is_err());
        assert_eq!(store.load().await.unwrap().places[0].point.latitude, 10.);
        tokio::fs::remove_file(&store.path).await.unwrap();
    }
    #[tokio::test]
    async fn malformed_library_is_not_silently_replaced_by_empty_data() {
        let store = store();
        tokio::fs::write(&store.path, b"{incomplete").await.unwrap();
        assert_eq!(
            store.load().await.unwrap_err().code,
            "LIBRARY_STORAGE_FAILED"
        );
        assert_eq!(tokio::fs::read(&store.path).await.unwrap(), b"{incomplete");
        tokio::fs::remove_file(&store.path).await.unwrap();
    }
    #[tokio::test]
    async fn explicit_recovery_preserves_corrupt_bytes_and_refuses_to_reset_healthy_data() {
        let root = std::env::temp_dir().join(format!(
            "meridian-recovery-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        tokio::fs::create_dir_all(&root).await.unwrap();
        let store = LibraryStore::new(root.join("library.json"));
        let damaged = b"{incomplete original library";
        tokio::fs::write(&store.path, damaged).await.unwrap();
        assert!(store.recover_corrupt().await.unwrap().places.is_empty());
        assert!(store.load().await.is_ok());
        let mut entries = tokio::fs::read_dir(&root).await.unwrap();
        let mut backups = vec![];
        while let Some(entry) = entries.next_entry().await.unwrap() {
            if entry.file_name().to_string_lossy().contains("recovered-") {
                backups.push(entry.path());
            }
        }
        assert_eq!(backups.len(), 1);
        assert_eq!(tokio::fs::read(&backups[0]).await.unwrap(), damaged);
        assert!(store.recover_corrupt().await.is_err());
        tokio::fs::remove_file(&store.path).await.unwrap();
        tokio::fs::remove_file(&backups[0]).await.unwrap();
        tokio::fs::remove_dir(root).await.unwrap();
    }
}
