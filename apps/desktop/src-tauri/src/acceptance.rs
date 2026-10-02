//! Explicit physical acceptance mode for the shipped executable's coordinator.
//! This does not claim that clicking the graphical controls was tested.
use meridian_device_core::{runtime::RuntimeConfig, Coordinator};
use meridian_shared_types::{ConnectRequest, Mode, Point, RouteSpec, Transport, TypedError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configuration {
    udid: String,
    transport: Transport,
    report_path: PathBuf,
}
fn require(condition: bool, message: &str) -> Result<(), TypedError> {
    if condition {
        Ok(())
    } else {
        Err(TypedError::invalid(message))
    }
}
fn step(steps: &mut Vec<Value>, name: &str) {
    let entry = json!({"name":name,"result":"PASS"});
    println!("{entry}");
    steps.push(entry);
}
async fn observe(seconds: u64, label: &str) {
    println!("{}", json!({"phase":label,"observeSeconds":seconds}));
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}
async fn exercise(
    core: &Arc<Coordinator>,
    config: &Configuration,
    steps: &mut Vec<Value>,
) -> Result<(), TypedError> {
    let selected = core.devices().await?;
    if config.transport == Transport::Usb {
        require(
            selected.iter().any(|device| device.id == config.udid),
            "The explicitly selected iPhone is absent.",
        )?;
    }
    step(steps, "device-enumeration");
    let opened = core
        .connect(ConnectRequest {
            udid: config.udid.clone(),
            transport: config.transport,
            rsd_address: None,
            rsd_port: None,
        })
        .await?;
    require(
        opened.mode == Mode::Hardware && opened.developer_services_ready,
        "A real developer session is required.",
    )?;
    step(steps, "selected-device-connect");
    core.health().await;
    require(
        core.status().await.last_typed_error.is_none(),
        "Initial session health failed.",
    )?;
    step(steps, "developer-session-health");
    let origin = Point {
        latitude: 40.758,
        longitude: -73.9855,
    };
    let applied = core.teleport(origin.clone()).await?;
    require(
        applied.held_point.as_ref() == Some(&origin),
        "Teleport acknowledgment mismatch.",
    )?;
    step(steps, "teleport");
    observe(8, "Times Square start point").await;
    let invalid = core
        .teleport(Point {
            latitude: 91.,
            longitude: 0.,
        })
        .await;
    require(
        invalid.is_err() && core.status().await.held_point.as_ref() == Some(&origin),
        "Invalid input disturbed the active session.",
    )?;
    step(
        steps,
        "invalid-coordinate-rejected-without-changing-location",
    );
    core.start_route(RouteSpec {
        id: "physical-acceptance-route".into(),
        name: "Times Square test route".into(),
        points: vec![
            origin,
            Point {
                latitude: 40.761,
                longitude: -73.9855,
            },
        ],
        speed_mps: 4.,
        r#loop: false,
    })
    .await?;
    observe(8, "Route moving north at 4 m/s").await;
    let paused = core.pause_route(true).await?;
    let progress = paused
        .active_route
        .as_ref()
        .ok_or_else(|| TypedError::invalid("Route progress missing."))?;
    require(
        progress.distance_meters > 0. && progress.paused,
        "Route did not advance and pause.",
    )?;
    let pause_distance = progress.distance_meters;
    let pause_point = paused.held_point;
    step(steps, "route-progress-and-pause");
    observe(6, "Route paused; location should stay still").await;
    let still = core.status().await;
    require(
        still.held_point == pause_point
            && still
                .active_route
                .as_ref()
                .is_some_and(|p| p.distance_meters == pause_distance),
        "Paused route moved.",
    )?;
    step(steps, "paused-route-keeps-acknowledged-position");
    core.speed(15.).await?;
    core.pause_route(false).await?;
    observe(8, "Route resumed north at 15 m/s").await;
    let resumed = core.status().await;
    require(
        resumed
            .active_route
            .as_ref()
            .is_some_and(|p| p.speed_mps == 15. && p.distance_meters > pause_distance),
        "Resume or speed change failed.",
    )?;
    step(steps, "route-resume-and-speed-change");
    let before_joystick = resumed.held_point;
    let moved = core.joystick(90., 35.).await?;
    require(
        moved.active_route.is_none()
            && moved.held_point.is_some()
            && moved.held_point != before_joystick,
        "Joystick handoff failed.",
    )?;
    step(steps, "joystick-east-retires-route");
    observe(8, "Joystick moved east; now holding").await;
    let restored = core.restore(false).await?;
    require(
        restored.held_point.is_none()
            && restored.active_route.is_none()
            && restored.location_session_state == "ready",
        "Restore did not clear the session.",
    )?;
    step(steps, "restore-clears-location-and-route");
    observe(6, "Restore sent; observe real location").await;
    Ok(())
}
pub async fn run() -> Result<(), TypedError> {
    require(
        std::env::var("ENABLE_IOS_DEVICE_TESTS").as_deref() == Ok("1"),
        "Physical acceptance requires ENABLE_IOS_DEVICE_TESTS=1.",
    )?;
    let file = std::env::args_os().nth(2).ok_or_else(|| {
        TypedError::invalid("A private selection/configuration file is required.")
    })?;
    let bytes = std::fs::read(&file)
        .map_err(|_| TypedError::invalid("Cannot read physical test configuration."))?;
    require(
        bytes.len() <= 16_384,
        "Physical test configuration is too large.",
    )?;
    let config: Configuration = serde_json::from_slice(&bytes)
        .map_err(|_| TypedError::invalid("Invalid physical test configuration."))?;
    require(
        config.report_path.is_absolute()
            && matches!(config.transport, Transport::Usb | Transport::Wifi),
        "An absolute report path and USB/Wi-Fi transport are required.",
    )?;
    let exe =
        std::env::current_exe().map_err(|_| TypedError::invalid("Executable path unavailable."))?;
    let root = exe
        .parent()
        .ok_or_else(|| TypedError::invalid("Executable parent unavailable."))?;
    let core = Coordinator::new(RuntimeConfig {
        executable: root.join("runtime/meridian-runtime/meridian-runtime.exe"),
        arguments: vec![],
        working_directory: None,
    });
    let ticking = core.clone();
    let stopping = Arc::new(AtomicBool::new(false));
    let tick_stopping = stopping.clone();
    let ticker = tokio::spawn(async move {
        let mut timer = tokio::time::interval(Duration::from_millis(200));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while !tick_stopping.load(Ordering::SeqCst) {
            timer.tick().await;
            if tick_stopping.load(Ordering::SeqCst) {
                break;
            }
            ticking.tick().await;
        }
    });
    let mut steps = vec![];
    let result = exercise(&core, &config, &mut steps).await;
    // Complete any in-flight JSONL exchange before asking the same pipe to Clear.
    stopping.store(true, Ordering::SeqCst);
    let _ = ticker.await;
    let before_close = core.status().await;
    let diagnostics = core.diagnostics().await;
    let cleanup = core.shutdown().await;
    if cleanup.is_ok() {
        step(&mut steps, "restore-and-owned-helper-shutdown");
    }
    let passed = result.is_ok() && cleanup.is_ok();
    let report = json!({"artifact":"shipped desktop executable and bundled helper","scope":"native coordinator physical acceptance; graphical clicks not automated","transport":config.transport,"protocolResult":if passed {"PASS"} else {"FAIL"},"steps":steps,"error":result.as_ref().err(),"lastSessionError":before_close.last_typed_error,"diagnostics":diagnostics,"cleanupError":cleanup.as_ref().err(),"deviceIdRedacted":before_close.device_id_redacted,"productVersion":before_close.product_version,"physicalGpsObservation":"UNVERIFIED","noGpsFlashObservation":"UNVERIFIED"});
    if let Some(parent) = config.report_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|_| TypedError::invalid("Cannot create report directory."))?;
    }
    std::fs::write(
        &config.report_path,
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|_| TypedError::invalid("Cannot save physical test report."))?;
    println!("{report}");
    result.and(cleanup)
}
