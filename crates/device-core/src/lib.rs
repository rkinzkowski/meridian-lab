pub mod library;
mod restoration;
pub mod runtime;

use meridian_route_engine::{step, validate_speed, Route};
use meridian_shared_types::*;
use runtime::{DemoRuntime, DeveloperRuntime, JsonlRuntime, RuntimeConfig};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, RwLock};

pub struct Coordinator {
    config: RuntimeConfig,
    runtime: Mutex<Box<dyn DeveloperRuntime>>,
    status: RwLock<RuntimeStatus>,
    operation: Mutex<()>,
    generation: AtomicU64,
    selected: Mutex<Option<ConnectRequest>>,
    route: Mutex<Option<Route>>,
    last_tick: Mutex<Instant>,
    health_state: Mutex<HealthState>,
    restoration: Option<restoration::JournalStore>,
    restoration_load_error: Option<TypedError>,
    restore_pending: AtomicBool,
}

impl Coordinator {
    pub fn new(config: RuntimeConfig) -> Arc<Self> {
        Self::build(config, None)
    }
    pub fn with_restoration(config: RuntimeConfig, path: std::path::PathBuf) -> Arc<Self> {
        Self::build(config, Some(restoration::JournalStore::new(path)))
    }
    fn build(config: RuntimeConfig, restoration: Option<restoration::JournalStore>) -> Arc<Self> {
        let loaded = restoration.as_ref().map(|store| store.load()).transpose();
        let (pending, load_error) = match loaded {
            Ok(record) => (record.flatten(), None),
            Err(error) => (None, Some(error)),
        };
        let restore_pending = pending.is_some() || load_error.is_some();
        let mut status = RuntimeStatus::default();
        if restore_pending {
            status.location_session_state = "restoreFailed".into();
            status.last_typed_error =
                Some(load_error.clone().unwrap_or_else(restore_pending_error));
            status.derive_setup();
        }
        Arc::new(Self {
            runtime: Mutex::new(Box::new(JsonlRuntime::new(config.clone()))),
            config,
            status: RwLock::new(status),
            operation: Mutex::new(()),
            generation: AtomicU64::new(0),
            selected: Mutex::new(pending.map(|record| record.request)),
            route: Mutex::new(None),
            last_tick: Mutex::new(Instant::now()),
            health_state: Mutex::new(HealthState::new()),
            restoration,
            restoration_load_error: load_error,
            restore_pending: AtomicBool::new(restore_pending),
        })
    }
    // Persist intent before Set: a lost reply cannot prove the device did not apply it.
    async fn arm_restoration(&self) -> Result<(), TypedError> {
        if self.status().await.mode == Mode::Demo {
            return Ok(());
        }
        if let Some(error) = &self.restoration_load_error {
            return Err(error.clone());
        }
        if self.restore_pending.load(Ordering::SeqCst) {
            return Ok(());
        }
        let request = self
            .selected
            .lock()
            .await
            .clone()
            .ok_or_else(TypedError::disconnected)?;
        if let Some(store) = &self.restoration {
            store.arm(&request)?;
        }
        self.restore_pending.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn clear_restoration(&self) -> Result<(), TypedError> {
        if let Some(error) = &self.restoration_load_error {
            return Err(error.clone());
        }
        if let Some(store) = &self.restoration {
            store.clear()?;
        }
        self.restore_pending.store(false, Ordering::SeqCst);
        Ok(())
    }
    pub async fn status(&self) -> RuntimeStatus {
        self.status.read().await.clone()
    }
    fn intent(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }
    fn current(&self, value: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == value
    }
    async fn update<F: FnOnce(&mut RuntimeStatus)>(&self, callback: F) -> RuntimeStatus {
        let mut state = self.status.write().await;
        callback(&mut state);
        state.derive_setup();
        state.clone()
    }
    async fn error(&self, error: TypedError) -> TypedError {
        self.update(|s| {
            s.busy = false;
            s.last_typed_error = Some(error.clone());
        })
        .await;
        error
    }
    async fn call(&self, method: &str, params: Value) -> Result<Value, TypedError> {
        self.runtime.lock().await.call(method, params).await
    }
    pub async fn devices(&self) -> Result<Vec<Device>, TypedError> {
        let _operation = self.operation.lock().await;
        match self.call("device.list", json!({})).await {
            Ok(value) => {
                let devices: Vec<Device> = serde_json::from_value(value["devices"].clone())
                    .map_err(|_| TypedError::invalid("Invalid device enumeration response."))?;
                let selected = self.selected.lock().await.clone();
                let selected_present = selected
                    .as_ref()
                    .is_some_and(|r| devices.iter().any(|d| d.id == r.udid));
                let stack_reachable = value["usbmuxReachable"]
                    .as_bool()
                    .ok_or_else(protocol_error)?;
                let state = self.status().await;
                // A healthy Wi-Fi/RSD session is authoritative even while usbmux has no cable.
                let active_network = state.developer_services_ready
                    && matches!(
                        state.active_transport,
                        Some(Transport::Wifi | Transport::Rsd)
                    );
                self.update(|s| {
                    s.usbmux_reachable = stack_reachable;
                    s.device_found = if selected.is_some() {
                        selected_present || active_network
                    } else {
                        !devices.is_empty()
                    };
                    s.usb_cable_present = devices.iter().any(|d| {
                        d.connection_type.eq_ignore_ascii_case("usb")
                            && selected.as_ref().is_none_or(|r| r.udid == d.id)
                    });
                })
                .await;
                if state.developer_services_ready
                    && state.active_transport == Some(Transport::Usb)
                    && !selected_present
                {
                    self.lost(TypedError::new(
                        "DEVICE_DISCONNECTED",
                        "The selected USB iPhone is no longer connected.",
                        "Reconnect the selected iPhone and recover the session.",
                        true,
                    ))
                    .await;
                }
                Ok(devices)
            }
            Err(error) => {
                let state = self.status().await;
                self.update(|s| {
                    s.usbmux_reachable = false;
                    s.usb_cable_present = false;
                    if s.active_transport != Some(Transport::Wifi)
                        && s.active_transport != Some(Transport::Rsd)
                    {
                        s.device_found = false;
                    }
                })
                .await;
                if state.developer_services_ready && state.active_transport == Some(Transport::Usb)
                {
                    self.lost(error.clone()).await;
                }
                Err(self.error(error).await)
            }
        }
    }
    pub async fn connect(&self, request: ConnectRequest) -> Result<RuntimeStatus, TypedError> {
        request.validate()?;
        let generation = self.intent();
        let _operation = self.operation.lock().await;
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        if self.status().await.location_session_state != "closed" {
            self.restore_locked(true, generation).await?;
            if !self.current(generation) {
                return Ok(self.status().await);
            }
        }
        let previous = self.selected.lock().await.clone();
        *self.selected.lock().await = Some(request.clone());
        *self.route.lock().await = None;
        self.update(|s| {
            s.busy = true;
            s.generation = generation;
            s.tunnel_state = "starting".into();
            s.location_session_state = "opening".into();
            s.active_route = None;
            s.developer_services_ready = false;
            s.last_typed_error = None;
            if previous.as_ref().is_none_or(|r| r.udid != request.udid) {
                s.lockdown_trusted = false;
                s.remote_pairing_state = "unknown".into();
                s.developer_mode_state = "unknown".into();
                s.developer_image_state = "unknown".into();
                s.device_name = None;
                s.product_version = None;
                s.device_id_redacted = None;
            }
        })
        .await;
        let params = connect_params(&request);
        match self.call("session.open", params).await {
            Ok(value) => {
                if !self.current(generation) {
                    self.restore_locked(true, generation).await?;
                    return Ok(self.status().await);
                }
                if !valid_open(&value) {
                    let _ = self.call("session.close", json!({})).await;
                    self.update(|s| {
                        s.location_session_state = "closed".into();
                        s.tunnel_state = "failed".into();
                        s.rsd_state = "failed".into();
                    })
                    .await;
                    return Err(self.error(protocol_error()).await);
                }
                *self.selected.lock().await = Some(request.clone());
                *self.health_state.lock().await = HealthState::new();
                Ok(self
                    .update(|s| {
                        s.device_found = true;
                        if request.transport == Transport::Usb {
                            s.lockdown_trusted = true;
                        }
                        s.developer_services_ready = true;
                        s.active_transport = Some(request.transport);
                        s.device_name = value["deviceName"].as_str().map(str::to_string);
                        s.product_version = value["productVersion"].as_str().map(str::to_string);
                        s.device_id_redacted =
                            value["deviceIdRedacted"].as_str().map(str::to_string);
                        s.tunnel_state = "ready".into();
                        s.rsd_state = "ready".into();
                        s.location_session_state = "ready".into();
                        s.recovering = false;
                        s.busy = false;
                        s.last_typed_error = None;
                    })
                    .await)
            }
            Err(error) => {
                if self.current(generation) {
                    self.update(|s| {
                        s.developer_services_ready = false;
                        s.tunnel_state = "failed".into();
                        s.rsd_state = "failed".into();
                        s.location_session_state = "closed".into();
                        if error.code == "DEVELOPER_MODE_DISABLED" {
                            s.developer_mode_state = "disabled".into();
                        }
                        if error.code == "TRUST_REQUIRED" || error.code == "STALE_PAIRING_RECORD" {
                            s.lockdown_trusted = false;
                        }
                    })
                    .await;
                    Err(self.error(error).await)
                } else {
                    Err(error)
                }
            }
        }
    }
    pub async fn prepare(&self, udid: String, method: &str) -> Result<RuntimeStatus, TypedError> {
        if !["device.trust", "device.prepare", "pairing.prepare"].contains(&method) {
            return Err(TypedError::invalid("Unknown preparation operation."));
        }
        ConnectRequest {
            udid: udid.clone(),
            transport: Transport::Usb,
            rsd_address: None,
            rsd_port: None,
        }
        .validate()?;
        let _operation = self.operation.lock().await;
        let selected = self.selected.lock().await.clone();
        if selected.as_ref().is_some_and(|r| r.udid != udid) {
            return Err(TypedError::new(
                "DEVICE_IDENTITY_MISMATCH",
                "Preparation target differs from the selected iPhone.",
                "Connect the intended iPhone before preparing it.",
                false,
            ));
        }
        if method == "pairing.prepare"
            && (selected.is_none()
                || !self.status().await.developer_services_ready
                || self.status().await.active_transport != Some(Transport::Usb))
        {
            return Err(TypedError::new(
                "CABLE_REQUIRED",
                "Wireless preparation needs a healthy selected USB developer session.",
                "Connect the intended iPhone over USB first.",
                true,
            ));
        }
        if method == "device.prepare" && self.status().await.held_point.is_some() {
            return Err(TypedError::new(
                "DEVICE_BUSY",
                "Developer image preparation cannot run during simulation.",
                "Restore real location before preparing developer support.",
                false,
            ));
        }
        self.update(|s| {
            s.busy = true;
            s.last_typed_error = None;
        })
        .await;
        match self.call(method, json!({"udid":udid})).await {
            Ok(value) => {
                let valid = match method {
                    "device.trust" => value["lockdownTrusted"] == true,
                    "device.prepare" => value["developerImageState"] == "mounted",
                    "pairing.prepare" => value["remotePairingState"] == "verified",
                    _ => false,
                };
                if !valid {
                    return Err(self.error(protocol_error()).await);
                }
                if selected.is_none() {
                    *self.selected.lock().await = Some(ConnectRequest {
                        udid,
                        transport: Transport::Usb,
                        rsd_address: None,
                        rsd_port: None,
                    });
                }
                Ok(self
                    .update(|s| {
                        s.busy = false;
                        match method {
                            "device.trust" => s.lockdown_trusted = true,
                            "device.prepare" => s.developer_image_state = "mounted".into(),
                            "pairing.prepare" => s.remote_pairing_state = "verified".into(),
                            _ => (),
                        }
                    })
                    .await)
            }
            Err(error) => Err(self.error(error).await),
        }
    }
    pub async fn teleport(&self, point: Point) -> Result<RuntimeStatus, TypedError> {
        point.validate()?;
        let generation = self.intent();
        let _operation = self.operation.lock().await;
        self.teleport_locked(point, generation).await
    }
    async fn teleport_locked(
        &self,
        point: Point,
        generation: u64,
    ) -> Result<RuntimeStatus, TypedError> {
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        self.require_ready().await?;
        self.arm_restoration().await?;
        *self.route.lock().await = None;
        self.update(|s| {
            s.active_route = None;
            s.generation = generation;
            s.busy = true;
        })
        .await;
        match self
            .call(
                "location.set",
                json!({"latitude":point.latitude,"longitude":point.longitude}),
            )
            .await
        {
            Ok(value) if self.current(generation) => {
                if !valid_set(&value, &point) {
                    let error = protocol_error();
                    self.lost(error.clone()).await;
                    return Err(error);
                }
                Ok(self
                    .update(|s| {
                        s.held_point = Some(point);
                        s.location_session_state = "holding".into();
                        s.last_typed_error = None;
                        s.busy = false;
                    })
                    .await)
            }
            Ok(_) => Ok(self.status().await),
            Err(error) => {
                if self.current(generation) {
                    self.lost(error.clone()).await;
                }
                Err(error)
            }
        }
    }
    pub async fn restore(&self, disconnect: bool) -> Result<RuntimeStatus, TypedError> {
        let generation = self.intent();
        let _operation = self.operation.lock().await;
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        self.restore_locked(disconnect, generation).await
    }
    async fn restore_locked(
        &self,
        disconnect: bool,
        generation: u64,
    ) -> Result<RuntimeStatus, TypedError> {
        *self.route.lock().await = None;
        self.update(|s| {
            s.generation = generation;
            s.active_route = None;
            s.recovering = false;
            s.busy = true;
        })
        .await;
        let state = self.status().await;
        if self.restore_pending.load(Ordering::SeqCst)
            && !state.developer_services_ready
            && state.mode == Mode::Hardware
        {
            // A fresh helper can acknowledge an empty session.close without contacting
            // the phone. Only explicit reconnect + session-bound Clear retires this debt.
            return Err(self
                .restore_recovery_error(
                    self.restoration_load_error
                        .clone()
                        .unwrap_or_else(restore_pending_error),
                )
                .await);
        }
        let was_closed = state.location_session_state == "closed" && state.held_point.is_none();
        let close_session =
            disconnect || (state.mode == Mode::Hardware && !state.developer_services_ready);
        let result = if was_closed {
            Ok(json!({"closed":true,"cleared":true}))
        } else if close_session {
            self.call("session.close", json!({})).await
        } else {
            self.call("location.clear", json!({})).await
        };
        let result = result.and_then(|value| {
            if value["cleared"] == true && (!close_session || value["closed"] == true) {
                Ok(value)
            } else {
                Err(protocol_error())
            }
        });
        match result {
            Ok(_) => {
                if let Err(error) = self.clear_restoration() {
                    return Err(self.restore_recovery_error(error).await);
                }
                if disconnect {
                    *self.selected.lock().await = None;
                }
                Ok(self
                    .update(|s| {
                        s.held_point = None;
                        s.last_typed_error = None;
                        s.busy = false;
                        s.location_session_state = if close_session || was_closed {
                            "closed"
                        } else {
                            "ready"
                        }
                        .into();
                        if close_session {
                            s.developer_services_ready = false;
                            s.tunnel_state = "closed".into();
                            s.rsd_state = "closed".into();
                            s.active_transport = None;
                        }
                    })
                    .await)
            }
            Err(error) => {
                self.update(|s| {
                    s.location_session_state = "restoreFailed".into();
                    s.developer_services_ready = false;
                    s.busy = false;
                    s.last_typed_error = Some(error.clone());
                })
                .await;
                Err(error)
            }
        }
    }
    pub async fn start_route(&self, spec: RouteSpec) -> Result<RuntimeStatus, TypedError> {
        let route = Route::new(spec)?;
        let generation = self.intent();
        let _operation = self.operation.lock().await;
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        self.require_ready().await?;
        self.arm_restoration().await?;
        *self.route.lock().await = None;
        self.update(|s| {
            s.active_route = None;
            s.generation = generation;
            s.busy = true;
        })
        .await;
        let point = route.point_at(0.);
        match self
            .call("location.set", serde_json::to_value(&point).unwrap())
            .await
        {
            Ok(value) if self.current(generation) => {
                if !valid_set(&value, &point) {
                    let error = protocol_error();
                    self.lost(error.clone()).await;
                    return Err(error);
                }
                let progress = route.progress();
                *self.route.lock().await = Some(route);
                *self.last_tick.lock().await = Instant::now();
                Ok(self
                    .update(|s| {
                        s.generation = generation;
                        s.held_point = Some(point);
                        s.active_route = Some(progress);
                        s.location_session_state = "routing".into();
                        s.last_typed_error = None;
                        s.busy = false;
                    })
                    .await)
            }
            Ok(_) => Ok(self.status().await),
            Err(error) => {
                if self.current(generation) {
                    self.lost(error.clone()).await;
                }
                Err(error)
            }
        }
    }
    pub async fn pause_route(&self, paused: bool) -> Result<RuntimeStatus, TypedError> {
        let _operation = self.operation.lock().await;
        let mut routes = self.route.lock().await;
        let route = routes.as_mut().ok_or_else(TypedError::disconnected)?;
        if route.finished {
            return Err(TypedError::invalid("This route has finished."));
        }
        if self.status().await.recovering {
            return Err(TypedError::disconnected());
        }
        route.paused = paused;
        *self.last_tick.lock().await = Instant::now();
        let progress = route.progress();
        Ok(self
            .update(|s| {
                s.active_route = Some(progress);
                s.location_session_state = if paused { "paused" } else { "routing" }.into();
            })
            .await)
    }
    pub async fn speed(&self, speed: f64) -> Result<RuntimeStatus, TypedError> {
        validate_speed(speed)?;
        let _operation = self.operation.lock().await;
        let mut routes = self.route.lock().await;
        let route = routes.as_mut().ok_or_else(TypedError::disconnected)?;
        route.spec.speed_mps = speed;
        *self.last_tick.lock().await = Instant::now();
        let progress = route.progress();
        Ok(self.update(|s| s.active_route = Some(progress)).await)
    }
    pub async fn joystick(&self, bearing: f64, meters: f64) -> Result<RuntimeStatus, TypedError> {
        // Validate before superseding a route, then calculate from the latest ACK under the writer lock.
        step(
            &Point {
                latitude: 0.,
                longitude: 0.,
            },
            bearing,
            meters,
        )?;
        let generation = self.intent();
        let _operation = self.operation.lock().await;
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        let state = self.status().await;
        let point = state.held_point.ok_or_else(TypedError::disconnected)?;
        self.teleport_locked(step(&point, bearing, meters)?, generation)
            .await
    }
    pub async fn tick(&self) {
        let Ok(_operation) = self.operation.try_lock() else {
            return;
        };
        let generation = self.generation.load(Ordering::SeqCst);
        let proposal = {
            let routes = self.route.lock().await;
            routes
                .as_ref()
                .filter(|r| !r.paused && !r.finished)
                .cloned()
        };
        let Some(route) = proposal else {
            return;
        };
        if self.require_ready().await.is_err() {
            return;
        }
        let now = Instant::now();
        let elapsed = now
            .duration_since(*self.last_tick.lock().await)
            .as_secs_f64();
        let (distance, point) = route.proposal(elapsed);
        match self
            .call("location.set", serde_json::to_value(&point).unwrap())
            .await
        {
            Ok(value) if self.current(generation) => {
                if !valid_set(&value, &point) {
                    self.lost(protocol_error()).await;
                    return;
                }
                *self.last_tick.lock().await = now;
                let mut routes = self.route.lock().await;
                if let Some(route) = routes.as_mut() {
                    route.acknowledge(distance);
                    let progress = route.progress();
                    self.update(|s| {
                        s.held_point = Some(point);
                        s.active_route = Some(progress);
                        if route.finished {
                            s.location_session_state = "holding".into();
                        }
                    })
                    .await;
                }
            }
            Ok(_) => (),
            Err(error) => {
                if self.current(generation) {
                    self.lost(error).await;
                }
            }
        }
    }
    async fn lost(&self, error: TypedError) {
        self.update(|s| {
            s.developer_services_ready = false;
            s.location_session_state = "recovering".into();
            s.tunnel_state = "failed".into();
            s.rsd_state = "failed".into();
            s.recovering = true;
            s.last_typed_error = Some(error);
            s.busy = false;
        })
        .await;
        if let Some(route) = self.route.lock().await.as_mut() {
            route.paused = true;
            let progress = route.progress();
            self.update(|s| s.active_route = Some(progress)).await;
        }
    }
    pub async fn health(&self) {
        let Ok(_operation) = self.operation.try_lock() else {
            return;
        };
        let state = self.status().await;
        if state.developer_services_ready && state.mode == Mode::Hardware {
            let generation = self.generation.load(Ordering::SeqCst);
            let result = self.call("session.health", json!({})).await.and_then(|v| {
                if v["healthy"] == true {
                    Ok(())
                } else {
                    Err(protocol_error())
                }
            });
            if !self.current(generation) {
                return;
            }
            let mut health = self.health_state.lock().await;
            match result {
                Ok(()) => health.success(),
                Err(error) => {
                    if health.failure(Instant::now()) {
                        drop(health);
                        self.lost(error).await;
                    }
                }
            }
        }
    }
    pub async fn recover(&self) -> Result<RuntimeStatus, TypedError> {
        let generation = self.generation.load(Ordering::SeqCst);
        let _operation = self.operation.lock().await;
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        let state = self.status().await;
        let restore_only = state.location_session_state == "restoreFailed";
        if !state.recovering && !restore_only {
            return Err(TypedError::invalid(
                "Recovery is available after a connection loss or failed restore.",
            ));
        }
        let request = self
            .selected
            .lock()
            .await
            .clone()
            .ok_or_else(TypedError::disconnected)?;
        if restore_only {
            return self.recover_restore_locked(&request).await;
        }
        let checkpoint = self.status().await.held_point;
        // Only explicitly invoked recovery; automatic retry budgets are a release gate.
        let _ = self.call("session.close", json!({})).await;
        self.runtime.lock().await.terminate().await;
        let opened = match self.call("session.open", connect_params(&request)).await {
            Ok(v) => v,
            Err(e) => {
                self.lost(e.clone()).await;
                return Err(e);
            }
        };
        if !valid_open(&opened) {
            self.lost(protocol_error()).await;
            return Err(protocol_error());
        }
        if !self.current(generation) {
            self.restore_locked(true, generation).await?;
            return Ok(self.status().await);
        }
        if let Some(point) = &checkpoint {
            self.arm_restoration().await?;
            match self
                .call("location.set", serde_json::to_value(point).unwrap())
                .await
            {
                Ok(value) if valid_set(&value, point) => (),
                Ok(_) => {
                    self.lost(protocol_error()).await;
                    return Err(protocol_error());
                }
                Err(e) => {
                    self.lost(e.clone()).await;
                    return Err(e);
                }
            }
        }
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        *self.last_tick.lock().await = Instant::now();
        *self.health_state.lock().await = HealthState::new();
        Ok(self
            .update(|s| {
                s.recovering = false;
                s.developer_services_ready = true;
                s.tunnel_state = "ready".into();
                s.rsd_state = "ready".into();
                s.location_session_state = if s.active_route.is_some() {
                    "paused"
                } else if checkpoint.is_some() {
                    "holding"
                } else {
                    "ready"
                }
                .into();
                s.last_typed_error = None;
            })
            .await)
    }
    /// A failed Restore retires the desired simulation. Reconnecting here exists
    /// only to issue a real Clear against that same selected device; the retained
    /// point is diagnostic evidence and must never become a replay request.
    async fn recover_restore_locked(
        &self,
        request: &ConnectRequest,
    ) -> Result<RuntimeStatus, TypedError> {
        *self.route.lock().await = None;
        self.update(|s| {
            s.active_route = None;
            s.recovering = false;
            s.busy = true;
            s.developer_services_ready = false;
            // Keep restoreFailed and heldPoint until the actual Clear succeeds.
        })
        .await;
        self.runtime.lock().await.terminate().await;
        let opened = match self.call("session.open", connect_params(request)).await {
            Ok(value) if valid_open(&value) => value,
            Ok(_) => return Err(self.restore_recovery_error(protocol_error()).await),
            Err(error) => return Err(self.restore_recovery_error(error).await),
        };
        match self.call("location.clear", json!({})).await {
            Ok(value)
                if value["cleared"] == true
                    && value["sessionId"].as_str() == opened["sessionId"].as_str() => {}
            Ok(_) => return Err(self.restore_recovery_error(protocol_error()).await),
            Err(error) => return Err(self.restore_recovery_error(error).await),
        }
        *self.health_state.lock().await = HealthState::new();
        if let Err(error) = self.clear_restoration() {
            return Err(self.restore_recovery_error(error).await);
        }
        Ok(self
            .update(|s| {
                s.held_point = None;
                s.active_route = None;
                s.location_session_state = "ready".into();
                s.developer_services_ready = true;
                s.device_found = true;
                s.active_transport = Some(request.transport);
                s.device_name = opened["deviceName"].as_str().map(str::to_string);
                s.product_version = opened["productVersion"].as_str().map(str::to_string);
                s.device_id_redacted = opened["deviceIdRedacted"].as_str().map(str::to_string);
                s.tunnel_state = "ready".into();
                s.rsd_state = "ready".into();
                s.recovering = false;
                s.busy = false;
                s.last_typed_error = None;
            })
            .await)
    }
    async fn restore_recovery_error(&self, error: TypedError) -> TypedError {
        self.update(|s| {
            s.location_session_state = "restoreFailed".into();
            s.developer_services_ready = false;
            s.recovering = false;
            s.busy = false;
            s.tunnel_state = "failed".into();
            s.rsd_state = "failed".into();
            s.last_typed_error = Some(error.clone());
        })
        .await;
        error
    }
    pub async fn demo(&self, enabled: bool) -> Result<RuntimeStatus, TypedError> {
        let generation = self.intent();
        let _operation = self.operation.lock().await;
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        self.restore_locked(true, generation).await?;
        if !self.current(generation) {
            return Ok(self.status().await);
        }
        self.runtime.lock().await.terminate().await;
        *self.runtime.lock().await = if enabled {
            Box::new(DemoRuntime)
        } else {
            Box::new(JsonlRuntime::new(self.config.clone()))
        };
        Ok(self
            .update(|s| {
                *s = RuntimeStatus::default();
                s.generation = generation;
                if enabled {
                    s.mode = Mode::Demo;
                    s.device_name = Some("Demo iPhone".into());
                    s.location_session_state = "ready".into();
                }
            })
            .await)
    }
    pub async fn diagnostics(&self) -> Value {
        let _operation = self.operation.lock().await;
        let state = self.status().await;
        let result = self
            .call("diagnostics.get", json!({}))
            .await
            .unwrap_or(json!({"runtime":"unavailable"}));
        json!({"runtime":result,"platform":state.platform,"mode":state.mode,"deviceIdRedacted":state.device_id_redacted,"session":state.location_session_state,"physicalDeviceVerified":false,"lastTypedError":state.last_typed_error})
    }
    pub async fn shutdown(&self) -> Result<(), TypedError> {
        let generation = self.intent();
        let _operation = self.operation.lock().await;
        let result = self.restore_locked(true, generation).await;
        self.runtime.lock().await.terminate().await;
        result.map(|_| ())
    }
    /// Explicit owner choice after a failed close. No phone operation or replay.
    pub async fn close_preserving_restore(&self) -> Result<(), TypedError> {
        self.intent();
        let _operation = self.operation.lock().await;
        let state = self.status().await;
        if self.restoration.is_none()
            || state.mode != Mode::Hardware
            || !(state.recovering || state.location_session_state == "restoreFailed")
        {
            return Err(TypedError::invalid(
                "There is no pending restoration to preserve.",
            ));
        }
        // A corrupt existing journal is preserved as-is, never replaced or deleted.
        if self.restoration_load_error.is_none() {
            let request = self.selected.lock().await.clone().ok_or_else(TypedError::disconnected)?;
            self.restoration.as_ref().unwrap().arm(&request)?;
            self.restore_pending.store(true, Ordering::SeqCst);
        }
        *self.route.lock().await = None;
        self.runtime.lock().await.terminate().await;
        Ok(())
    }
    async fn require_ready(&self) -> Result<(), TypedError> {
        let s = self.status().await;
        if s.recovering
            || s.location_session_state == "restoreFailed"
            || s.location_session_state == "closed"
            || (s.mode == Mode::Hardware && !s.developer_services_ready)
        {
            Err(TypedError::disconnected())
        } else {
            Ok(())
        }
    }
}
fn restore_pending_error() -> TypedError {
    TypedError::new("RESTORE_PENDING", "Real location has not been confirmed restored.",
        "Reconnect the previously selected iPhone and choose Reconnect and restore. No location will be replayed.", true)
}
fn protocol_error() -> TypedError {
    TypedError::new(
        "PROTOCOL_INVALID",
        "The runtime did not confirm the requested developer operation.",
        "Reconnect the selected iPhone and inspect runtime diagnostics.",
        true,
    )
}
fn valid_open(value: &Value) -> bool {
    value["developerServicesReady"] == true
        && value["sessionId"].as_str().is_some_and(|v| !v.is_empty())
}
fn valid_set(value: &Value, point: &Point) -> bool {
    // Rust/Python JSON decimal conversion can differ by one floating-point bit.
    // Permit at most ~11 micrometers, while retaining strict session identity.
    let same = |actual: Option<f64>, expected: f64| {
        actual.is_some_and(|actual| actual.is_finite() && (actual - expected).abs() <= 1e-10)
    };
    same(value["latitude"].as_f64(), point.latitude)
        && same(value["longitude"].as_f64(), point.longitude)
        && value["sessionId"].as_str().is_some_and(|v| !v.is_empty())
}
struct HealthState {
    started: Instant,
    confirmed: bool,
    failures: u8,
}
impl HealthState {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            confirmed: false,
            failures: 0,
        }
    }
    fn success(&mut self) {
        self.confirmed = true;
        self.failures = 0;
    }
    fn failure(&mut self, now: Instant) -> bool {
        if !self.confirmed {
            return now.duration_since(self.started) >= Duration::from_secs(30);
        }
        self.failures = self.failures.saturating_add(1);
        self.failures >= 4
    }
}
fn connect_params(request: &ConnectRequest) -> Value {
    let mut params = json!({"udid":request.udid,"transport":request.transport});
    if request.transport == Transport::Rsd {
        params["rsdAddress"] = json!(request.rsd_address);
        params["rsdPort"] = json!(request.rsd_port);
    }
    params
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::Semaphore;
    #[test]
    fn route_acknowledgement_survives_json_float_roundtrip() {
        for index in 1..2000 {
            let point = Point {
                latitude: 40.758 + (index as f64) * 0.0000007314321,
                longitude: -73.9855,
            };
            let reply = json!({"latitude":point.latitude,"longitude":point.longitude,"sessionId":"test-session"});
            let wire = serde_json::to_string(&reply).unwrap();
            let decoded: Value = serde_json::from_str(&wire).unwrap();
            assert!(
                valid_set(&decoded, &point),
                "A serialized route coordinate was rejected: {wire}"
            );
        }
        let point = Point {
            latitude: 40.758,
            longitude: -73.9855,
        };
        assert!(!valid_set(
            &json!({"latitude":40.758001,"longitude":-73.9855,"sessionId":"test"}),
            &point
        ));
        assert!(!valid_set(
            &json!({"latitude":40.758,"longitude":-73.9855,"sessionId":""}),
            &point
        ));
    }
    fn core() -> Arc<Coordinator> {
        Coordinator::new(RuntimeConfig {
            executable: std::env::temp_dir().join("absent-meridian-test-runtime"),
            arguments: vec![],
            working_directory: None,
        })
    }
    #[tokio::test]
    async fn hardware_does_not_fall_back_to_demo() {
        let c = core();
        assert!(c.devices().await.is_err());
        assert_eq!(c.status().await.mode, Mode::Hardware);
        assert!(!c.status().await.physical_device_verified);
    }
    #[tokio::test]
    async fn invalid_point_does_not_retire_state() {
        let c = core();
        assert!(c
            .teleport(Point {
                latitude: 100.,
                longitude: 0.
            })
            .await
            .is_err());
        assert_eq!(c.status().await.generation, 0);
    }
    #[tokio::test]
    async fn explicit_demo_works_without_installed_hardware_runtime() {
        let c = core();
        c.demo(true).await.unwrap();
        assert_eq!(c.status().await.mode, Mode::Demo);
        c.teleport(Point {
            latitude: 40.,
            longitude: -73.,
        })
        .await
        .unwrap();
        c.demo(false).await.unwrap();
        assert_eq!(c.status().await.mode, Mode::Hardware);
        assert!(c.status().await.held_point.is_none());
        assert!(c.devices().await.is_err());
    }
    #[tokio::test]
    async fn restoring_closed_state_does_not_report_ready() {
        let c = core();
        assert_eq!(
            c.restore(false).await.unwrap().location_session_state,
            "closed"
        );
    }

    struct Controls {
        calls: Mutex<Vec<(String, Value)>>,
        fail_set: AtomicBool,
        fail_clear: AtomicBool,
        session_live: AtomicBool,
        open_ready: AtomicBool,
        present: AtomicBool,
        hold_first_set: AtomicBool,
        hold_next_clear: AtomicBool,
        entered: Semaphore,
        release: Semaphore,
    }
    impl Default for Controls {
        fn default() -> Self {
            Self {
                calls: Mutex::new(vec![]),
                fail_set: AtomicBool::new(false),
                fail_clear: AtomicBool::new(false),
                session_live: AtomicBool::new(false),
                open_ready: AtomicBool::new(true),
                present: AtomicBool::new(true),
                hold_first_set: AtomicBool::new(false),
                hold_next_clear: AtomicBool::new(false),
                entered: Semaphore::new(0),
                release: Semaphore::new(0),
            }
        }
    }
    struct FaultRuntime(Arc<Controls>);
    #[async_trait]
    impl DeveloperRuntime for FaultRuntime {
        async fn call(&mut self, method: &str, params: Value) -> Result<Value, TypedError> {
            self.0
                .calls
                .lock()
                .await
                .push((method.into(), params.clone()));
            if matches!(
                method,
                "session.close" | "session.health" | "location.set" | "location.clear"
            ) && !self.0.session_live.load(Ordering::SeqCst)
            {
                return Err(TypedError::new(
                    "RUNTIME_EXITED",
                    "Injected terminated runtime.",
                    "Reconnect the selected device.",
                    true,
                ));
            }
            if method == "location.set" {
                if self.0.hold_first_set.swap(false, Ordering::SeqCst) {
                    self.0.entered.add_permits(1);
                    self.0.release.acquire().await.unwrap().forget();
                }
                if self.0.fail_set.load(Ordering::SeqCst) {
                    return Err(TypedError::new(
                        "CONNECTION_DROPPED",
                        "Injected test connection loss.",
                        "Recover.",
                        true,
                    ));
                }
            }
            if method == "location.clear" && self.0.hold_next_clear.swap(false, Ordering::SeqCst) {
                self.0.entered.add_permits(1);
                self.0.release.acquire().await.unwrap().forget();
            }
            if matches!(method, "location.clear" | "session.close")
                && self.0.fail_clear.load(Ordering::SeqCst)
            {
                return Err(TypedError::new(
                    "RESTORE_FAILED",
                    "Injected test clear failure.",
                    "Retry clear.",
                    true,
                ));
            }
            let mut value = DemoRuntime.call(method, params).await?;
            if method == "session.open" {
                self.0.session_live.store(true, Ordering::SeqCst);
            } else if method == "session.close" {
                self.0.session_live.store(false, Ordering::SeqCst);
            }
            if method == "session.open" && !self.0.open_ready.load(Ordering::SeqCst) {
                value["developerServicesReady"] = json!(false);
            }
            if method == "device.list" && !self.0.present.load(Ordering::SeqCst) {
                value["devices"] = json!([]);
            }
            Ok(value)
        }
        async fn terminate(&mut self) {
            self.0.session_live.store(false, Ordering::SeqCst);
        }
    }
    fn request(transport: Transport) -> ConnectRequest {
        ConnectRequest {
            udid: "DEMO".into(),
            transport,
            rsd_address: None,
            rsd_port: None,
        }
    }
    async fn fault_core() -> (Arc<Coordinator>, Arc<Controls>) {
        let c = core();
        let control = Arc::new(Controls::default());
        *c.runtime.lock().await = Box::new(FaultRuntime(control.clone()));
        c.connect(request(Transport::Usb)).await.unwrap();
        (c, control)
    }
    fn point(latitude: f64) -> Point {
        Point {
            latitude,
            longitude: 0.,
        }
    }
    fn route_spec() -> RouteSpec {
        RouteSpec {
            id: "test-route".into(),
            name: "Test route".into(),
            points: vec![point(0.), point(0.01)],
            speed_mps: 10.,
            r#loop: false,
        }
    }

    #[tokio::test]
    async fn failed_clear_preserves_location_and_blocks_demo_transition() {
        let (c, control) = fault_core().await;
        c.teleport(point(12.)).await.unwrap();
        control.fail_clear.store(true, Ordering::SeqCst);
        assert_eq!(c.demo(true).await.unwrap_err().code, "RESTORE_FAILED");
        let state = c.status().await;
        assert_eq!(state.mode, Mode::Hardware);
        assert_eq!(state.held_point, Some(point(12.)));
        assert_eq!(state.location_session_state, "restoreFailed");
        assert!(!state.developer_services_ready);
        assert!(c.recover().await.is_err());
        assert!(c.teleport(point(13.)).await.is_err());
    }
    #[tokio::test]
    async fn failed_shutdown_reconnects_selected_device_and_clears_without_replaying() {
        let (c, control) = fault_core().await;
        c.teleport(point(12.)).await.unwrap();
        control.fail_clear.store(true, Ordering::SeqCst);
        assert_eq!(c.shutdown().await.unwrap_err().code, "RESTORE_FAILED");
        assert!(!control.session_live.load(Ordering::SeqCst));
        assert_eq!(c.restore(false).await.unwrap_err().code, "RESTORE_PENDING");
        control.fail_clear.store(false, Ordering::SeqCst);
        control.hold_next_clear.store(true, Ordering::SeqCst);
        let before = control.calls.lock().await.len();
        let recovery = tokio::spawn({
            let c = c.clone();
            async move { c.recover().await }
        });
        control.entered.acquire().await.unwrap().forget();
        let pending = c.status().await;
        assert_eq!(pending.location_session_state, "restoreFailed");
        assert_eq!(pending.held_point, Some(point(12.)));
        assert!(!pending.developer_services_ready);
        assert!(pending.busy);
        control.release.add_permits(1);
        let restored = recovery.await.unwrap().unwrap();
        assert_eq!(restored.location_session_state, "ready");
        assert!(restored.held_point.is_none());
        assert!(restored.active_route.is_none());
        assert!(restored.developer_services_ready);
        let calls = control.calls.lock().await;
        let retry = &calls[before..];
        assert_eq!(retry.len(), 2);
        assert_eq!(retry[0].0, "session.open");
        assert_eq!(retry[0].1["udid"], "DEMO");
        assert_eq!(retry[0].1["transport"], "usb");
        assert_eq!(retry[1].0, "location.clear");
    }
    #[tokio::test]
    async fn failed_restore_reconnect_or_clear_keeps_failure_reachable_for_another_retry() {
        let (c, control) = fault_core().await;
        c.teleport(point(7.)).await.unwrap();
        control.fail_clear.store(true, Ordering::SeqCst);
        assert!(c.shutdown().await.is_err());
        let before = control.calls.lock().await.len();
        control.open_ready.store(false, Ordering::SeqCst);
        assert_eq!(c.recover().await.unwrap_err().code, "PROTOCOL_INVALID");
        assert_eq!(c.status().await.location_session_state, "restoreFailed");
        control.open_ready.store(true, Ordering::SeqCst);
        assert_eq!(c.recover().await.unwrap_err().code, "RESTORE_FAILED");
        let failed = c.status().await;
        assert_eq!(failed.location_session_state, "restoreFailed");
        assert_eq!(failed.held_point, Some(point(7.)));
        assert!(!failed.recovering);
        assert!(!failed.developer_services_ready);
        control.fail_clear.store(false, Ordering::SeqCst);
        assert!(c.recover().await.unwrap().held_point.is_none());
        assert!(!control.calls.lock().await[before..]
            .iter()
            .any(|(method, _)| method == "location.set"));
    }
    #[tokio::test]
    async fn newer_teleport_retires_delayed_acknowledgement_without_clearing_channel() {
        let (c, control) = fault_core().await;
        control.hold_first_set.store(true, Ordering::SeqCst);
        let a = tokio::spawn({
            let c = c.clone();
            async move { c.teleport(point(1.)).await }
        });
        control.entered.acquire().await.unwrap().forget();
        let previous = c.generation.load(Ordering::SeqCst);
        let b = tokio::spawn({
            let c = c.clone();
            async move { c.teleport(point(2.)).await }
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while c.generation.load(Ordering::SeqCst) == previous {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        control.release.add_permits(1);
        a.await.unwrap().unwrap();
        b.await.unwrap().unwrap();
        assert_eq!(c.status().await.held_point, Some(point(2.)));
        let calls = control.calls.lock().await;
        assert_eq!(calls.iter().filter(|(m, _)| m == "session.open").count(), 1);
        assert_eq!(calls.iter().filter(|(m, _)| m == "location.set").count(), 2);
        assert!(!calls
            .iter()
            .any(|(m, _)| m == "location.clear" || m == "session.close"));
    }
    #[tokio::test]
    async fn recovery_replays_acknowledged_checkpoint_and_keeps_route_paused() {
        let (c, control) = fault_core().await;
        c.start_route(route_spec()).await.unwrap();
        *c.last_tick.lock().await = Instant::now() - Duration::from_millis(500);
        c.tick().await;
        let checkpoint = c.status().await;
        assert!(checkpoint.active_route.as_ref().unwrap().distance_meters > 4.9);
        control.fail_set.store(true, Ordering::SeqCst);
        *c.last_tick.lock().await = Instant::now() - Duration::from_millis(500);
        c.tick().await;
        assert!(c.status().await.recovering);
        assert_eq!(c.status().await.held_point, checkpoint.held_point);
        control.fail_set.store(false, Ordering::SeqCst);
        let recovered = c.recover().await.unwrap();
        assert_eq!(recovered.held_point, checkpoint.held_point);
        assert_eq!(
            recovered.active_route.as_ref().unwrap().distance_meters,
            checkpoint.active_route.as_ref().unwrap().distance_meters
        );
        assert!(recovered.active_route.as_ref().unwrap().paused);
        assert_eq!(recovered.location_session_state, "paused");
        c.tick().await;
        assert_eq!(c.status().await.held_point, checkpoint.held_point);
    }
    #[tokio::test]
    async fn replay_failure_cannot_mark_recovered_or_advance_checkpoint() {
        let (c, control) = fault_core().await;
        c.start_route(route_spec()).await.unwrap();
        *c.last_tick.lock().await = Instant::now() - Duration::from_millis(500);
        c.tick().await;
        let checkpoint = c.status().await;
        control.fail_set.store(true, Ordering::SeqCst);
        c.tick().await;
        assert!(c.recover().await.is_err());
        let state = c.status().await;
        assert!(state.recovering);
        assert!(!state.developer_services_ready);
        assert_eq!(state.held_point, checkpoint.held_point);
        assert_eq!(
            state.active_route.unwrap().distance_meters,
            checkpoint.active_route.unwrap().distance_meters
        );
    }
    #[tokio::test]
    async fn restore_after_connection_loss_does_not_close_an_empty_helper() {
        let (c, control) = fault_core().await;
        c.teleport(point(4.)).await.unwrap();
        control.fail_set.store(true, Ordering::SeqCst);
        assert!(c.teleport(point(5.)).await.is_err());
        let error = c.restore(false).await.unwrap_err();
        assert_eq!(error.code, "RESTORE_PENDING");
        let state = c.status().await;
        assert!(!state.developer_services_ready);
        assert!(state.held_point.is_some());
        assert!(!state.recovering);
        assert!(!control
            .calls
            .lock()
            .await
            .iter()
            .any(|(m, _)| m == "session.close"));
    }
    #[tokio::test]
    async fn removing_selected_usb_invalidates_readiness_and_pauses_route() {
        let (c, control) = fault_core().await;
        c.start_route(route_spec()).await.unwrap();
        control.present.store(false, Ordering::SeqCst);
        c.devices().await.unwrap();
        let state = c.status().await;
        assert!(!state.device_found);
        assert!(!state.developer_services_ready);
        assert!(state.recovering);
        assert!(state.active_route.unwrap().paused);
        assert_eq!(state.setup_stage, "connect");
    }
    #[tokio::test]
    async fn wifi_session_does_not_require_cable_or_infer_lockdown_trust() {
        let c = core();
        let control = Arc::new(Controls::default());
        *c.runtime.lock().await = Box::new(FaultRuntime(control.clone()));
        c.connect(request(Transport::Wifi)).await.unwrap();
        control.present.store(false, Ordering::SeqCst);
        c.devices().await.unwrap();
        let state = c.status().await;
        assert!(state.device_found);
        assert!(state.developer_services_ready);
        assert!(!state.usb_cable_present);
        assert!(!state.lockdown_trusted);
        assert_eq!(state.setup_stage, "ready");
    }
    #[tokio::test]
    async fn rejects_readiness_claim_without_successful_developer_service_flag() {
        let c = core();
        let control = Arc::new(Controls::default());
        control.open_ready.store(false, Ordering::SeqCst);
        *c.runtime.lock().await = Box::new(FaultRuntime(control));
        assert_eq!(
            c.connect(request(Transport::Usb)).await.unwrap_err().code,
            "PROTOCOL_INVALID"
        );
        assert!(!c.status().await.developer_services_ready);
    }
    #[tokio::test]
    async fn preparation_cannot_mutate_another_phones_trust_state() {
        let (c, control) = fault_core().await;
        let before = control.calls.lock().await.len();
        assert_eq!(
            c.prepare("OTHER".into(), "device.trust")
                .await
                .unwrap_err()
                .code,
            "DEVICE_IDENTITY_MISMATCH"
        );
        assert_eq!(control.calls.lock().await.len(), before);
    }
    #[test]
    fn health_ratchet_tolerates_warmup_and_requires_four_confirmed_failures() {
        let mut h = HealthState::new();
        assert!(!h.failure(h.started + Duration::from_secs(29)));
        assert!(h.failure(h.started + Duration::from_secs(30)));
        h.success();
        for _ in 0..3 {
            assert!(!h.failure(Instant::now()));
        }
        assert!(h.failure(Instant::now()));
        h.success();
        assert!(!h.failure(Instant::now()));
    }

    fn restoration_path(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "meridian-coordinator-restore-{label}-{}-{}.json",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ))
    }

    fn test_config() -> RuntimeConfig {
        RuntimeConfig {
            executable: std::env::temp_dir().join("absent-meridian-test-runtime"),
            arguments: vec![],
            working_directory: None,
        }
    }

    fn remove_restoration(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(path.parent().unwrap_or(path));
    }

    #[tokio::test]
    async fn restoration_is_armed_before_a_set_that_loses_its_reply() {
        let path = restoration_path("before-set");
        let c = Coordinator::with_restoration(test_config(), path.clone());
        let control = Arc::new(Controls::default());
        *c.runtime.lock().await = Box::new(FaultRuntime(control.clone()));
        c.connect(request(Transport::Usb)).await.unwrap();
        control.fail_set.store(true, Ordering::SeqCst);
        assert!(c.teleport(point(12.)).await.is_err());
        drop(c);
        assert!(restoration::JournalStore::new(path.clone()).load().unwrap().is_some());
        remove_restoration(&path);
    }

    #[tokio::test]
    async fn startup_pending_restore_is_failed_without_replaying_a_point() {
        let path = restoration_path("startup");
        let store = restoration::JournalStore::new(path.clone());
        store.arm(&request(Transport::Usb)).unwrap();
        drop(store);
        let c = Coordinator::with_restoration(test_config(), path.clone());
        let status = c.status().await;
        assert_eq!(status.location_session_state, "restoreFailed");
        assert!(status.held_point.is_none());
        assert!(status.active_route.is_none());
        remove_restoration(&path);
    }

    #[tokio::test]
    async fn repeated_shutdown_with_pending_restore_does_not_fake_session_close() {
        let path = restoration_path("shutdown-pending");
        restoration::JournalStore::new(path.clone())
            .arm(&request(Transport::Usb))
            .unwrap();
        let c = Coordinator::with_restoration(test_config(), path.clone());
        let control = Arc::new(Controls::default());
        *c.runtime.lock().await = Box::new(FaultRuntime(control.clone()));
        assert!(c.shutdown().await.is_err());
        assert!(c.shutdown().await.is_err());
        let calls = control.calls.lock().await;
        assert!(!calls.iter().any(|(method, _)| method == "session.close"));
        drop(calls);
        drop(c);
        assert!(restoration::JournalStore::new(path.clone()).load().unwrap().is_some());
        remove_restoration(&path);
    }

    #[tokio::test]
    async fn explicit_recovery_opens_and_clears_without_set_then_deletes_journal() {
        let path = restoration_path("recover");
        restoration::JournalStore::new(path.clone())
            .arm(&request(Transport::Usb))
            .unwrap();
        let c = Coordinator::with_restoration(test_config(), path.clone());
        let control = Arc::new(Controls::default());
        *c.runtime.lock().await = Box::new(FaultRuntime(control.clone()));
        c.recover().await.unwrap();
        let calls = control.calls.lock().await;
        assert_eq!(calls.iter().filter(|(method, _)| method == "session.open").count(), 1);
        assert_eq!(calls.iter().filter(|(method, _)| method == "location.clear").count(), 1);
        assert!(!calls.iter().any(|(method, _)| method == "location.set"));
        drop(calls);
        drop(c);
        assert!(restoration::JournalStore::new(path.clone()).load().unwrap().is_none());
        remove_restoration(&path);
    }

    #[tokio::test]
    async fn journal_write_failure_blocks_set() {
        let root = std::env::temp_dir().join(format!(
            "meridian-coordinator-restore-blocker-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let blocker = root.join("blocker");
        std::fs::write(&blocker, b"file").unwrap();
        let path = blocker.join("restoration.json");
        let c = Coordinator::with_restoration(test_config(), path);
        let control = Arc::new(Controls::default());
        *c.runtime.lock().await = Box::new(FaultRuntime(control.clone()));
        assert!(c.teleport(point(12.)).await.is_err());
        assert!(!control
            .calls
            .lock()
            .await
            .iter()
            .any(|(method, _)| method == "location.set"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn explicit_close_preserves_pending_journal_for_restart() {
        let path = restoration_path("close-preserve");
        let c = Coordinator::with_restoration(test_config(), path.clone());
        let control = Arc::new(Controls::default());
        *c.runtime.lock().await = Box::new(FaultRuntime(control.clone()));
        c.connect(request(Transport::Usb)).await.unwrap();
        c.teleport(point(12.)).await.unwrap();
        control.fail_set.store(true, Ordering::SeqCst);
        assert!(c.teleport(point(13.)).await.is_err());
        c.close_preserving_restore().await.unwrap();
        drop(c);
        let restarted = Coordinator::with_restoration(test_config(), path.clone());
        assert_eq!(restarted.status().await.location_session_state, "restoreFailed");
        drop(restarted);
        assert!(restoration::JournalStore::new(path.clone()).load().unwrap().is_some());
        remove_restoration(&path);
    }
}
