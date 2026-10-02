#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod acceptance;

use meridian_device_core::{library::LibraryStore, runtime::RuntimeConfig, Coordinator};
use meridian_shared_types::*;
use serde_json::Value;
#[cfg(debug_assertions)]
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use tauri::{Emitter, Manager, State};
use tokio::sync::Mutex;

struct AppState {
    core: Arc<Coordinator>,
    library: Mutex<Library>,
    library_load_error: Mutex<Option<TypedError>>,
    store: LibraryStore,
    closing: Arc<AtomicU8>,
}
#[tauri::command]
async fn close_with_pending_restore(handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), TypedError> {
    state.closing.compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| TypedError::invalid("Closing is already in progress."))?;
    match state.core.close_preserving_restore().await {
        Ok(()) => {
            state.closing.store(2, Ordering::SeqCst);
            handle.exit(0);
            Ok(())
        }
        Err(error) => {
            state.closing.store(0, Ordering::SeqCst);
            Err(error)
        }
    }
}
async fn change_library(
    state: &AppState,
    change: impl FnOnce(&mut Library),
) -> Result<Library, TypedError> {
    let mut live = state.library.lock().await;
    if let Some(error) = state.library_load_error.lock().await.clone() {
        return Err(error);
    }
    let mut pending = live.clone();
    change(&mut pending);
    state.store.save(&pending).await?;
    *live = pending;
    Ok(live.clone())
}
#[tauri::command]
async fn runtime_status(state: State<'_, AppState>) -> Result<RuntimeStatus, TypedError> {
    Ok(state.core.status().await)
}
#[tauri::command]
async fn list_devices(state: State<'_, AppState>) -> Result<Vec<Device>, TypedError> {
    state.core.devices().await
}
#[tauri::command]
async fn connect_device(
    state: State<'_, AppState>,
    request: ConnectRequest,
) -> Result<RuntimeStatus, TypedError> {
    state.core.connect(request).await
}
#[tauri::command]
async fn trust_device(
    state: State<'_, AppState>,
    udid: String,
) -> Result<RuntimeStatus, TypedError> {
    state.core.prepare(udid, "device.trust").await
}
#[tauri::command]
async fn prepare_device(
    state: State<'_, AppState>,
    udid: String,
) -> Result<RuntimeStatus, TypedError> {
    state.core.prepare(udid, "device.prepare").await
}
#[tauri::command]
async fn prepare_wireless(
    state: State<'_, AppState>,
    udid: String,
) -> Result<RuntimeStatus, TypedError> {
    state.core.prepare(udid, "pairing.prepare").await
}
#[tauri::command]
async fn disconnect_device(state: State<'_, AppState>) -> Result<RuntimeStatus, TypedError> {
    state.core.restore(true).await
}
#[tauri::command]
async fn restore_location(state: State<'_, AppState>) -> Result<RuntimeStatus, TypedError> {
    state.core.restore(false).await
}
#[tauri::command]
async fn teleport(state: State<'_, AppState>, point: Point) -> Result<RuntimeStatus, TypedError> {
    let status = state.core.teleport(point.clone()).await?;
    if status.held_point.as_ref() == Some(&point) {
        let history_enabled = state.library.lock().await.history_enabled;
        if history_enabled {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            change_library(&state, |library| {
                if library.history_enabled {
                    library.history.insert(
                        0,
                        HistoryEntry {
                            id: format!("{}-{}", time, status.generation),
                            label: "Selected location".into(),
                            point,
                            timestamp_ms: time,
                            mode: status.mode,
                        },
                    );
                    library.history.truncate(500);
                }
            })
            .await
            .map_err(|_| {
                TypedError::new(
                    "HISTORY_SAVE_FAILED",
                    "The location was applied, but its history entry could not be saved.",
                    "Check available storage. The active location can still be restored.",
                    false,
                )
            })?;
        }
    }
    Ok(status)
}
#[tauri::command]
async fn start_route(
    state: State<'_, AppState>,
    route: RouteSpec,
) -> Result<RuntimeStatus, TypedError> {
    state.core.start_route(route).await
}
#[tauri::command]
async fn pause_route(state: State<'_, AppState>) -> Result<RuntimeStatus, TypedError> {
    state.core.pause_route(true).await
}
#[tauri::command]
async fn resume_route(state: State<'_, AppState>) -> Result<RuntimeStatus, TypedError> {
    state.core.pause_route(false).await
}
#[tauri::command]
async fn set_route_speed(
    state: State<'_, AppState>,
    speed_mps: f64,
) -> Result<RuntimeStatus, TypedError> {
    state.core.speed(speed_mps).await
}
#[tauri::command]
async fn joystick_step(
    state: State<'_, AppState>,
    bearing_degrees: f64,
    distance_meters: f64,
) -> Result<RuntimeStatus, TypedError> {
    state.core.joystick(bearing_degrees, distance_meters).await
}
#[tauri::command]
async fn recover_session(state: State<'_, AppState>) -> Result<RuntimeStatus, TypedError> {
    state.core.recover().await
}
#[tauri::command]
async fn set_demo_mode(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<RuntimeStatus, TypedError> {
    state.core.demo(enabled).await
}
#[tauri::command]
async fn get_diagnostics(state: State<'_, AppState>) -> Result<Value, TypedError> {
    Ok(state.core.diagnostics().await)
}
#[tauri::command]
async fn get_library(state: State<'_, AppState>) -> Result<Library, TypedError> {
    let library = state.library.lock().await;
    if let Some(error) = state.library_load_error.lock().await.clone() {
        return Err(error);
    }
    Ok(library.clone())
}
#[tauri::command]
async fn recover_library(state: State<'_, AppState>) -> Result<Library, TypedError> {
    let mut library = state.library.lock().await;
    let mut load_error = state.library_load_error.lock().await;
    if load_error.is_none() {
        return Ok(library.clone());
    }
    let recovered = state.store.recover_corrupt().await?;
    *library = recovered;
    *load_error = None;
    Ok(library.clone())
}
#[tauri::command]
async fn save_place(state: State<'_, AppState>, place: SavedPlace) -> Result<Library, TypedError> {
    place.point.validate()?;
    if place.id.is_empty()
        || place.id.len() > 128
        || place.name.is_empty()
        || place.name.len() > 120
    {
        return Err(TypedError::invalid("Invalid saved place."));
    }
    change_library(&state, |lib| {
        lib.places.retain(|p| p.id != place.id);
        lib.places.push(place);
    })
    .await
}
#[tauri::command]
async fn delete_place(state: State<'_, AppState>, id: String) -> Result<Library, TypedError> {
    change_library(&state, |lib| lib.places.retain(|p| p.id != id)).await
}
#[tauri::command]
async fn save_route(state: State<'_, AppState>, route: RouteSpec) -> Result<Library, TypedError> {
    meridian_route_engine::Route::new(route.clone())?;
    change_library(&state, |lib| {
        lib.routes.retain(|r| r.id != route.id);
        lib.routes.push(route);
    })
    .await
}
#[tauri::command]
async fn delete_route(state: State<'_, AppState>, id: String) -> Result<Library, TypedError> {
    change_library(&state, |lib| lib.routes.retain(|r| r.id != id)).await
}
#[tauri::command]
async fn clear_history(state: State<'_, AppState>) -> Result<Library, TypedError> {
    change_library(&state, |lib| lib.history.clear()).await
}
#[tauri::command]
async fn set_history_enabled(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<Library, TypedError> {
    change_library(&state, |lib| lib.history_enabled = enabled).await
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--device-test") {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(acceptance::run());
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    if std::env::args().nth(1).as_deref() == Some("--self-test") {
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(offline_self_test());
        match result {
            Ok(()) => {
                println!("Meridian desktop offline self-test: PASS (no phone accessed)");
                std::process::exit(0);
            }
            Err(error) => {
                eprintln!("Meridian desktop offline self-test: {}", error.code);
                std::process::exit(1);
            }
        }
    }
    let closing = Arc::new(AtomicU8::new(0));
    let setup_closing = closing.clone();
    let app = tauri::Builder::default()
        .setup(move |app| {
            let resource = app.path().resource_dir()?;
            #[cfg(windows)]
            let executable = resource.join("runtime/meridian-runtime/meridian-runtime.exe");
            #[cfg(not(windows))]
            let executable = resource.join("runtime/meridian-runtime/meridian-runtime");
            let config = RuntimeConfig {
                executable,
                arguments: vec![],
                working_directory: None,
            };
            #[cfg(debug_assertions)]
            let config = if let Some(python) = std::env::var_os("MERIDIAN_PYTHON") {
                RuntimeConfig {
                    executable: PathBuf::from(python),
                    arguments: vec!["-m".into(), "location_runtime".into()],
                    working_directory: Some(
                        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../python-runtime"),
                    ),
                }
            } else {
                config
            };
            let core = Coordinator::with_restoration(config, app.path().app_data_dir()?.join("restoration.json"));
            let store = LibraryStore::new(app.path().app_data_dir()?.join("library.json"));
            // Saved-data corruption must never make the Restore UI inaccessible.
            // Keep writes blocked until the owner explicitly preserves/restarts it.
            let (library, library_load_error) = match tauri::async_runtime::block_on(store.load()) {
                Ok(library) => (library, None),
                Err(error) => (Library::default(), Some(error)),
            };
            app.manage(AppState {
                core: core.clone(),
                library: Mutex::new(library),
                library_load_error: Mutex::new(library_load_error),
                store,
                closing: setup_closing.clone(),
            });
            let handle = app.handle().clone();
            let emitter = core.clone();
            tauri::async_runtime::spawn(async move {
                let mut timer = tokio::time::interval(std::time::Duration::from_millis(200));
                loop {
                    timer.tick().await;
                    let _ = handle.emit("runtime-status", emitter.status().await);
                }
            });
            let route = core.clone();
            tauri::async_runtime::spawn(async move {
                let mut timer = tokio::time::interval(std::time::Duration::from_millis(200));
                timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    timer.tick().await;
                    route.tick().await;
                }
            });
            tauri::async_runtime::spawn(async move {
                let mut timer = tokio::time::interval(std::time::Duration::from_secs(5));
                loop {
                    timer.tick().await;
                    core.health().await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runtime_status,
            list_devices,
            connect_device,
            trust_device,
            prepare_device,
            prepare_wireless,
            disconnect_device,
            restore_location,
            teleport,
            start_route,
            pause_route,
            resume_route,
            set_route_speed,
            joystick_step,
            recover_session,
            close_with_pending_restore,
            set_demo_mode,
            get_diagnostics,
            get_library,
            recover_library,
            save_place,
            delete_place,
            save_route,
            delete_route,
            clear_history,
            set_history_enabled
        ])
        .build(tauri::generate_context!())
        .expect("Meridian desktop initialization failed");
    app.run(move |handle, event| match event {
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { api, .. },
            ..
        } if closing.load(Ordering::SeqCst) != 2 => {
            api.prevent_close();
            request_clean_exit(handle.clone(), closing.clone());
        }
        tauri::RunEvent::ExitRequested { api, .. } if closing.load(Ordering::SeqCst) != 2 => {
            api.prevent_exit();
            request_clean_exit(handle.clone(), closing.clone());
        }
        _ => (),
    });
}

fn request_clean_exit(handle: tauri::AppHandle, closing: Arc<AtomicU8>) {
    if closing
        .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    // A close request may wait for an in-flight developer operation. Show that
    // immediately, even if the renderer has failed to load or stopped listening.
    if let Some(window) = handle.get_webview_window("main") {
        let _ = window.set_title("Meridian Lab — restoring and closing…");
    }
    tauri::async_runtime::spawn(async move {
        let state = handle.state::<AppState>();
        if state.core.shutdown().await.is_ok() {
            closing.store(2, Ordering::SeqCst);
            handle.exit(0);
        } else {
            // Keep the existing window and its restore error visible. A second
            // close click during cleanup must not skip the pending Clear.
            closing.store(0, Ordering::SeqCst);
            let _ = handle.emit("close-restore-pending", serde_json::json!({}));
            if let Some(window) = handle.get_webview_window("main") {
                let _ = window.set_title("Meridian Lab — Restore needs attention");
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    });
}

/// Exercises the real owned-process/JSONL boundary and explicit demo coordinator.
/// No device enumeration, preparation, or location command is sent to hardware.
async fn offline_self_test() -> Result<(), TypedError> {
    use meridian_device_core::runtime::{DeveloperRuntime, JsonlRuntime};
    // Release verification must fail for a build that still relies on Vite.
    let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
    if tauri::is_dev()
        || context.assets().get(&"index.html".into()).is_none()
        || !context
            .assets()
            .iter()
            .any(|(path, _)| path.contains("maplibre-gl-worker-") && path.ends_with(".js"))
    {
        return Err(TypedError::invalid(
            "The desktop interface and map worker must be embedded in this build.",
        ));
    }
    let root = std::env::current_exe()
        .map_err(|_| TypedError::invalid("Executable path unavailable"))?
        .parent()
        .ok_or_else(|| TypedError::invalid("Executable parent unavailable"))?
        .to_path_buf();
    let config = RuntimeConfig {
        executable: root.join(if cfg!(windows) {
            "runtime/meridian-runtime/meridian-runtime.exe"
        } else {
            "runtime/meridian-runtime/meridian-runtime"
        }),
        arguments: vec![],
        working_directory: None,
    };
    let mut runtime = JsonlRuntime::new(config.clone());
    let info = runtime.call("runtime.info", serde_json::json!({})).await;
    if info
        .as_ref()
        .is_ok_and(|v| v["pymobiledevice3Version"] == "11.9.1")
    {
        runtime
            .call("runtime.shutdown", serde_json::json!({}))
            .await?;
    } else {
        runtime.terminate().await;
        return Err(info
            .err()
            .unwrap_or_else(|| TypedError::invalid("Runtime dependency mismatch")));
    }
    runtime.terminate().await;
    let core = Coordinator::new(config);
    core.demo(true).await?;
    core.teleport(Point {
        latitude: 40.758,
        longitude: -73.9855,
    })
    .await?;
    let cleared = core.restore(false).await?;
    if cleared.held_point.is_some() {
        return Err(TypedError::invalid("Demo restore failed"));
    }
    core.shutdown().await?;
    Ok(())
}
