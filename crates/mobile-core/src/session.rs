use crate::model::{Configuration, ConnectionMode, Fault, Point, verify_identity};
use idevice::{
    IdeviceService,
    core_device_proxy::CoreDeviceProxy,
    dvt::{location_simulation::LocationSimulationClient, remote_server::RemoteServerClient},
    lockdown::LockdownClient,
    provider::TcpProvider,
    remote_pairing::{RemotePairingClient, RpPairingSocket, connect_tls_psk_tunnel_native},
    rsd::RsdHandshake,
    tcp::{adapter::Adapter, handle::AdapterHandle},
};
use meridian_route_engine::Route;
use meridian_shared_types::{RouteProgress, RouteSpec};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
    time::timeout,
};

const CONNECT: Duration = Duration::from_secs(25);
const COMMAND: Duration = Duration::from_secs(5);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub phase: String,
    pub held_point: Option<Point>,
    pub active_route: Option<RouteProgress>,
    pub error: Option<Fault>,
    pub physical_device_verified: bool,
    pub restore_required: bool,
    pub clear_confirmed: bool,
}
impl Status {
    fn phase(phase: &str) -> Self {
        Self {
            phase: phase.into(),
            held_point: None,
            active_route: None,
            error: None,
            physical_device_verified: false,
            restore_required: false,
            clear_confirmed: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "method",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    Prepare {},
    RestoreOnly {},
    Set { point: Point },
    Route { spec: RouteSpec },
    Pause {},
    Resume {},
    Speed { speed_mps: f64 },
    Joystick { bearing: f64, meters: f64 },
    Clear {},
    Health {},
    Status {},
    Stop {},
}
struct Envelope {
    request: Request,
    reply: oneshot::Sender<Result<Status, Fault>>,
}
pub struct SessionHandle {
    sender: mpsc::Sender<Envelope>,
    cancel: watch::Sender<bool>,
    status: watch::Receiver<Status>,
}

impl SessionHandle {
    pub fn new(config: Configuration, _runtime: &tokio::runtime::Runtime) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel(8);
        let (cancel, cancel_rx) = watch::channel(false);
        let (status_tx, status) = watch::channel(Status::phase("idle"));
        // Upstream TLS-PSK's borrowed handshake future is not Send. Keep the
        // entire persistent device session on one owned native thread; Swift
        // only exchanges bounded messages and never runs network I/O itself.
        std::thread::Builder::new()
            .name("meridian-device".into())
            .spawn(move || {
                let session_runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("mobile session runtime");
                session_runtime.block_on(worker(config, receiver, cancel_rx, status_tx));
            })
            .expect("mobile session thread");
        Arc::new(Self {
            sender,
            cancel,
            status,
        })
    }
    pub async fn call(&self, request: Request) -> Result<Status, Fault> {
        if matches!(request, Request::Status {}) {
            return Ok(self.status.borrow().clone());
        }
        if matches!(request, Request::Stop {}) {
            self.cancel.send_replace(true);
            let mut state = self.status.clone();
            timeout(Duration::from_secs(22), async {
                loop {
                    let value = state.borrow().clone();
                    if value.phase == "stopped" { return Ok(value); }
                    if value.phase == "failed" {
                        if !value.restore_required {
                            return Ok(Status { phase: "stopped".into(), error: None, ..value });
                        }
                        return Err(value.error.unwrap_or_else(restore_unconfirmed));
                    }
                    state.changed().await.map_err(|_| lost())?;
                }
            }).await.map_err(|_| Fault::new("RESTORE_UNCONFIRMED", "The connection did not confirm that simulation was cleared. Reconnect through the desktop and Restore Real Location."))?
        } else {
            let (reply, receive) = oneshot::channel();
            self.sender
                .try_send(Envelope { request, reply })
                .map_err(|_| {
                    Fault::new(
                        "SESSION_BUSY",
                        "A developer operation is in progress or the connection has closed.",
                    )
                })?;
            timeout(CONNECT + COMMAND + Duration::from_secs(2), receive)
                .await
                .map_err(|_| timed_out())?
                .map_err(|_| lost())?
        }
    }
}
impl Drop for SessionHandle {
    fn drop(&mut self) {
        self.cancel.send_replace(true);
    }
}

fn lost() -> Fault {
    Fault::new(
        "CONNECTION_DROPPED",
        "The local developer connection was lost. Reopen the helper and reconnect; desktop Restore is available if needed.",
    )
}
fn timed_out() -> Fault {
    Fault::new(
        "CONNECTION_TIMEOUT",
        "The developer service did not respond in time. Check the local connection and Developer Mode.",
    )
}
fn restore_unconfirmed() -> Fault {
    Fault::new(
        "RESTORE_UNCONFIRMED",
        "The device has not confirmed Clear. Reconnect the same paired device and retry Restore; no location will be replayed.",
    )
}
fn finish_clear(
    status: &mut Status,
    clear_result: Result<(), Fault>,
    connection_error: Option<Fault>,
) {
    status.active_route = None;
    match clear_result {
        Err(_) => {
            status.phase = "failed".into();
            status.restore_required = true;
            status.clear_confirmed = false;
            status.error = Some(restore_unconfirmed());
        }
        Ok(()) => {
            status.held_point = None;
            status.restore_required = false;
            status.clear_confirmed = true;
            status.phase = if connection_error.is_some() {
                "failed"
            } else {
                "stopped"
            }
            .into();
            status.error = connection_error;
        }
    }
}
async fn bounded<T>(
    future: impl std::future::Future<Output = Result<T, idevice::IdeviceError>>,
) -> Result<T, Fault> {
    timeout(COMMAND, future)
        .await
        .map_err(|_| timed_out())?
        .map_err(|_| lost())
}
async fn check_identity(
    adapter: &mut AdapterHandle,
    rsd_port: u16,
    expected: &str,
) -> Result<RsdHandshake, Fault> {
    let stream = timeout(COMMAND, adapter.connect(rsd_port))
        .await
        .map_err(|_| timed_out())?
        .map_err(|_| lost())?;
    let handshake = bounded(RsdHandshake::new(stream)).await?;
    verify_identity(
        expected,
        handshake
            .properties
            .get("UniqueDeviceID")
            .and_then(|v| v.as_string()),
    )?;
    Ok(handshake)
}

async fn connect(
    config: &Configuration,
) -> Result<
    (
        AdapterHandle,
        u16,
        RemoteServerClient<Box<dyn idevice::ReadWrite>>,
    ),
    Fault,
> {
    let endpoint = config.endpoint()?;
    if config.connection_mode == ConnectionMode::LocalCoreDevice {
        let provider = TcpProvider {
            addr: endpoint.ip(),
            scope_id: None,
            pairing_file: config.lockdown_pairing()?,
            label: "Meridian Lab local developer session".into(),
        };
        let mut lockdown = bounded(LockdownClient::connect(&provider)).await.map_err(|_|Fault::new("LOCAL_LOCKDOWN_UNAVAILABLE", "The local iPhone management service is unreachable. Check the local connection profile; this iOS build may restrict self-device access."))?;
        bounded(lockdown.start_session(&provider.pairing_file)).await.map_err(|_|Fault::new("TRUST_REQUIRED", "The phone rejected its saved Lockdown trust. Repair pairing through the trusted desktop."))?;
        let identity = bounded(lockdown.get_value(Some("UniqueDeviceID"), None)).await?;
        verify_identity(&config.expected_device_id, identity.as_string())?;
        let proxy = bounded(CoreDeviceProxy::connect(&provider)).await.map_err(|_|Fault::new("LOCAL_COREDEVICE_UNAVAILABLE", "This iOS build did not open CoreDeviceProxy through the local connection. No developer readiness has been assumed."))?;
        let port = proxy.tunnel_info().server_rsd_port;
        if port == 0 {
            return Err(Fault::new(
                "RSD_UNAVAILABLE",
                "The local CoreDevice tunnel did not publish RSD.",
            ));
        }
        let adapter = AdapterHandle::new(proxy.create_software_tunnel().map_err(|_| lost())?);
        return open_developer_services(adapter, port, &config.expected_device_id).await;
    }
    let stream = TcpStream::connect(endpoint).await.map_err(|_| Fault::new("LOCAL_HELPER_UNREACHABLE", "The local developer endpoint is unreachable. Activate the approved connection helper, then try again."))?;
    let mut pairing = config.pairing()?;
    let mut remote = RemotePairingClient::new(RpPairingSocket::new(stream), "Meridian Lab");
    remote.attempt_pair_verify().await.map_err(|_| {
        Fault::new(
            "REMOTE_PAIRING_REQUIRED",
            "Repair mobile pairing from the trusted desktop.",
        )
    })?;
    // Do not use connect(): upstream falls back to creating new pairing implicitly.
    remote.validate_pairing(&mut pairing).await.map_err(|_| {
        Fault::new(
            "REMOTE_PAIRING_REJECTED",
            "The phone rejected the pairing record. Repair mobile setup over trusted USB.",
        )
    })?;
    let port = remote.create_tcp_listener().await.map_err(|_| {
        Fault::new(
            "TUNNEL_START_FAILED",
            "The device did not publish a developer tunnel.",
        )
    })?;
    let stream = TcpStream::connect(std::net::SocketAddr::new(endpoint.ip(), port))
        .await
        .map_err(|_| lost())?;
    let tunnel = connect_tls_psk_tunnel_native(stream, remote.encryption_key())
        .await
        .map_err(|_| {
            Fault::new(
                "TUNNEL_START_FAILED",
                "The paired developer tunnel could not be established.",
            )
        })?;
    let info = tunnel.info.clone();
    if info.server_rsd_port == 0 {
        return Err(Fault::new(
            "RSD_UNAVAILABLE",
            "The tunnel did not publish Remote Service Discovery.",
        ));
    }
    let client = info.client_address.parse().map_err(|_| lost())?;
    let server = info.server_address.parse().map_err(|_| lost())?;
    let adapter = AdapterHandle::new(Adapter::new(Box::new(tunnel.into_inner()), client, server));
    open_developer_services(adapter, info.server_rsd_port, &config.expected_device_id).await
}

async fn open_developer_services(
    mut adapter: AdapterHandle,
    rsd_port: u16,
    expected_device_id: &str,
) -> Result<
    (
        AdapterHandle,
        u16,
        RemoteServerClient<Box<dyn idevice::ReadWrite>>,
    ),
    Fault,
> {
    let result = async {
        let mut handshake = check_identity(&mut adapter, rsd_port, expected_device_id).await?;
        if !handshake.services.contains_key("com.apple.instruments.dtservicehub") { return Err(Fault::new("DEVELOPER_SERVICES_UNAVAILABLE", "Pairing succeeded, but Instruments is unavailable. Check Developer Mode and developer support from the desktop.")); }
        bounded(handshake.connect::<RemoteServerClient<Box<dyn idevice::ReadWrite>>>(&mut adapter)).await
    }.await;
    match result {
        Ok(server) => Ok((adapter, rsd_port, server)),
        Err(error) => {
            let _ = adapter.close().await;
            Err(error)
        }
    }
}

async fn worker(
    config: Configuration,
    mut receive: mpsc::Receiver<Envelope>,
    mut cancel: watch::Receiver<bool>,
    state: watch::Sender<Status>,
) {
    let first = tokio::select! {
        _ = cancel.changed() => { state.send_replace(Status::phase("stopped")); return; },
        item = receive.recv() => match item { Some(item) => item, None => return }
    };
    let restore_only = matches!(first.request, Request::RestoreOnly {});
    if !matches!(first.request, Request::Prepare {} | Request::RestoreOnly {}) {
        let _ = first.reply.send(Err(Fault::new(
            "SESSION_NOT_READY",
            "Prepare the developer connection before changing location.",
        )));
        return;
    }
    state.send_replace(Status {
        restore_required: restore_only,
        ..Status::phase("connecting")
    });
    let connected = tokio::select! {
        _ = cancel.changed() => {
            let error = if restore_only { restore_unconfirmed() } else { Fault::new("CANCELED", "Connection canceled.") };
            let _ = first.reply.send(Err(error.clone()));
            state.send_replace(Status { restore_required: restore_only, error: restore_only.then_some(error), ..Status::phase(if restore_only {"failed"} else {"stopped"}) }); return;
        },
        value = timeout(CONNECT, connect(&config)) => value.map_err(|_| timed_out()).and_then(|v| v)
    };
    let (mut adapter, port, mut server) = match connected {
        Ok(value) => value,
        Err(error) => {
            state.send_replace(Status {
                error: Some(error.clone()),
                restore_required: restore_only,
                ..Status::phase("failed")
            });
            let _ = first.reply.send(Err(error));
            return;
        }
    };
    let mut location = match bounded(LocationSimulationClient::new(&mut server)).await {
        Ok(location) => location,
        Err(error) => {
            let _ = adapter.close().await;
            state.send_replace(Status {
                error: Some(error.clone()),
                restore_required: restore_only,
                ..Status::phase("failed")
            });
            let _ = first.reply.send(Err(error));
            return;
        }
    };
    let mut status = Status::phase("ready");
    if restore_only {
        status.restore_required = true;
        state.send_replace(Status {
            phase: "stopping".into(),
            ..status.clone()
        });
        let cleared = bounded(location.clear()).await;
        drop(server);
        let _ = adapter.close().await;
        finish_clear(&mut status, cleared, None);
        state.send_replace(status.clone());
        let reply = if status.clear_confirmed {
            Ok(status)
        } else {
            Err(restore_unconfirmed())
        };
        let _ = first.reply.send(reply);
        return;
    }
    state.send_replace(status.clone());
    let _ = first.reply.send(Ok(status.clone()));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut connection_error = None;
    let mut route: Option<Route> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(250));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_tick = tokio::time::Instant::now();
    loop {
        tokio::select! {
            biased;
            _ = cancel.changed() => break,
            item = receive.recv() => {
                let Some(item) = item else { break; };
                let result = match item.request {
                    Request::Set { point } => {
                        match point.validate() {
                            Err(error) => Err(error),
                            Ok(()) => match check_identity(&mut adapter, port, &config.expected_device_id).await {
                                Err(error) => Err(error),
                                Ok(_) => {route=None; status.active_route=None; status.restore_required=true;status.clear_confirmed=false;state.send_replace(status.clone()); bounded(location.set(point.latitude, point.longitude)).await.map(|_| {status.held_point=Some(point); status.phase="active".into();})}
                            }
                        }
                    },
                    Request::Route {spec} => match Route::new(spec) {
                        Err(error)=>Err(Fault::new("INVALID_REQUEST",&error.message)),
                        Ok(next)=>{let point=next.point_at(0.); route=None; status.active_route=None;status.restore_required=true;status.clear_confirmed=false;state.send_replace(status.clone()); bounded(location.set(point.latitude,point.longitude)).await.map(|_|{status.held_point=Some(Point{latitude:point.latitude,longitude:point.longitude});status.phase="active".into();status.active_route=Some(next.progress());route=Some(next);last_tick=tokio::time::Instant::now();})}
                    },
                    Request::Pause {} => {if let Some(active)=route.as_mut(){active.paused=true;status.active_route=Some(active.progress());} Ok(())},
                    Request::Resume {} => {if let Some(active)=route.as_mut(){active.paused=false;status.active_route=Some(active.progress());last_tick=tokio::time::Instant::now();} Ok(())},
                    Request::Speed{speed_mps} => meridian_route_engine::validate_speed(speed_mps).map_err(|error|Fault::new("INVALID_REQUEST",&error.message)).map(|_|{if let Some(active)=route.as_mut(){active.spec.speed_mps=speed_mps;status.active_route=Some(active.progress());last_tick=tokio::time::Instant::now();}}),
                    Request::Joystick{bearing,meters} => match status.held_point.as_ref() {
                        None=>Err(Fault::new("INVALID_REQUEST","Teleport to a starting point before using the joystick.")),
                        Some(point)=>match meridian_route_engine::step(&meridian_shared_types::Point{latitude:point.latitude,longitude:point.longitude},bearing,meters) {
                            Err(error)=>Err(Fault::new("INVALID_REQUEST",&error.message)),
                            Ok(next)=>{route=None;status.active_route=None;status.restore_required=true;status.clear_confirmed=false;state.send_replace(status.clone());bounded(location.set(next.latitude,next.longitude)).await.map(|_|{status.held_point=Some(Point{latitude:next.latitude,longitude:next.longitude});status.phase="active".into();})}
                        }
                    },
                    Request::Clear {} => {route=None;status.active_route=None;bounded(location.clear()).await.map(|_| {status.held_point=None; status.phase="ready".into();status.restore_required=false;status.clear_confirmed=true;})},
                    Request::RestoreOnly {} => Err(Fault::new("INVALID_REQUEST","Clear-only recovery requires a fresh session; use Clear for a live session.")),
                    Request::Health {} | Request::Prepare {} => check_identity(&mut adapter, port, &config.expected_device_id).await.map(|_| ()),
                    Request::Status {} => Ok(()),
                    Request::Stop {} => Ok(()),
                };
                match result {
                    Ok(()) => { state.send_replace(status.clone()); let _ = item.reply.send(Ok(status.clone())); },
                    Err(error) => { let fatal = !["INVALID_COORDINATE","INVALID_REQUEST"].contains(&error.code.as_str()); let _ = item.reply.send(Err(error.clone())); if fatal { connection_error=Some(error); break; } }
                }
            },
            _ = heartbeat.tick() => {
                if let Err(error) = check_identity(&mut adapter, port, &config.expected_device_id).await { connection_error=Some(error); break; }
            },
            _ = tick.tick() => {
                let elapsed=last_tick.elapsed().as_secs_f64();last_tick=tokio::time::Instant::now();
                if let Some(active)=route.as_mut().filter(|r|!r.paused && !r.finished) {
                        let (distance,point)=active.proposal(elapsed);
                        match bounded(location.set(point.latitude,point.longitude)).await {
                            Ok(())=>{active.acknowledge(distance);status.held_point=Some(Point{latitude:point.latitude,longitude:point.longitude});status.active_route=Some(active.progress());state.send_replace(status.clone());},
                            Err(error)=>{connection_error=Some(error);break;}
                        }
                }
            }
        }
    }
    status.active_route = None;
    status.phase = "stopping".into();
    state.send_replace(status.clone());
    // Clear even when the last health check failed, and preserve failed restoration.
    let cleared = bounded(location.clear()).await;
    drop(server);
    let _ = adapter.close().await;
    finish_clear(&mut status, cleared, connection_error);
    state.send_replace(status);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_clear_preserves_uncertainty_and_last_acknowledged_point() {
        let mut status = Status::phase("active");
        status.held_point = Some(Point {
            latitude: 1.0,
            longitude: 2.0,
        });
        finish_clear(&mut status, Err(lost()), None);
        assert!(status.restore_required);
        assert!(!status.clear_confirmed);
        assert_eq!(status.held_point.as_ref().unwrap().latitude, 1.0);
        assert_eq!(status.error.as_ref().unwrap().code, "RESTORE_UNCONFIRMED");
    }
    #[test]
    fn confirmed_clear_resolves_uncertainty_even_after_connection_error() {
        let mut status = Status::phase("active");
        status.restore_required = true;
        finish_clear(&mut status, Ok(()), Some(lost()));
        assert!(!status.restore_required);
        assert!(status.clear_confirmed);
        assert!(status.held_point.is_none());
    }
    #[test]
    fn restore_only_is_a_distinct_first_operation() {
        let value: Request = serde_json::from_str(r#"{"method":"restoreOnly"}"#).unwrap();
        assert!(matches!(value, Request::RestoreOnly {}));
        assert!(
            serde_json::from_str::<Request>(
                r#"{"method":"restoreOnly","point":{"latitude":1,"longitude":2}}"#
            )
            .is_err()
        );
    }
}
