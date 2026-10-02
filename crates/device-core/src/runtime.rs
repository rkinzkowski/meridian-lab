use async_trait::async_trait;
use meridian_ipc_types::{decode_reply, Request, MAX_LINE_BYTES};
use meridian_shared_types::TypedError;
use serde_json::{json, Value};
use std::{path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

#[async_trait]
pub trait DeveloperRuntime: Send + Sync {
    async fn call(&mut self, method: &str, params: Value) -> Result<Value, TypedError>;
    async fn terminate(&mut self);
}

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    pub working_directory: Option<PathBuf>,
}
struct OwnedProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    #[cfg(windows)]
    _job: job::Job,
}
pub struct JsonlRuntime {
    config: RuntimeConfig,
    process: Option<OwnedProcess>,
    serial: u64,
    session_id: Option<String>,
}
impl JsonlRuntime {
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config,
            process: None,
            serial: 0,
            session_id: None,
        }
    }
    async fn start(&mut self) -> Result<(), TypedError> {
        if let Some(p) = self.process.as_mut() {
            if p.child.try_wait().map_err(|_| crashed())?.is_none() {
                return Ok(());
            }
        }
        self.process = None;
        self.session_id = None;
        if !self.config.executable.is_absolute() || !self.config.executable.is_file() {
            return Err(TypedError::new("RUNTIME_MISSING","The bundled developer runtime is unavailable.","Repair or reinstall Meridian Lab. Developers can configure MERIDIAN_PYTHON in debug builds.",false));
        }
        let mut command = Command::new(&self.config.executable);
        command
            .args(&self.config.arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .env("PYTHONUNBUFFERED", "1");
        if let Some(dir) = &self.config.working_directory {
            command.current_dir(dir);
        }
        #[cfg(windows)]
        command.creation_flags(0x0800_0000);
        let mut child = command.spawn().map_err(|_| {
            TypedError::new(
                "RUNTIME_START_FAILED",
                "The developer runtime could not start.",
                "Repair the installation and check application-control policy.",
                true,
            )
        })?;
        #[cfg(windows)]
        let job = job::Job::assign(&child)?;
        let stdin = child.stdin.take().ok_or_else(crashed)?;
        let stdout = BufReader::new(child.stdout.take().ok_or_else(crashed)?);
        self.process = Some(OwnedProcess {
            child,
            stdin,
            stdout,
            #[cfg(windows)]
            _job: job,
        });
        Ok(())
    }
    async fn exchange(&mut self, method: &str, params: Value) -> Result<Value, TypedError> {
        let requires_session = matches!(
            method,
            "session.close"
                | "session.health"
                | "location.set"
                | "location.clear"
                | "pairing.prepare"
        );
        if requires_session && self.session_id.is_none() {
            return Err(crashed());
        }
        self.start().await?;
        // start() may replace a dead child. An empty replacement cannot attest
        // that a previous physical simulation was restored.
        if requires_session && self.session_id.is_none() {
            return Err(crashed());
        }
        self.serial = self.serial.wrapping_add(1);
        let id = format!("desktop-{}", self.serial);
        let mut bytes = serde_json::to_vec(&Request {
            request_id: id.clone(),
            method,
            params,
        })
        .map_err(|_| TypedError::invalid("Request cannot be encoded."))?;
        if bytes.len() >= MAX_LINE_BYTES {
            return Err(TypedError::invalid(
                "Request exceeds the runtime message limit.",
            ));
        }
        bytes.push(b'\n');
        let p = self.process.as_mut().ok_or_else(crashed)?;
        p.stdin.write_all(&bytes).await.map_err(|_| crashed())?;
        p.stdin.flush().await.map_err(|_| crashed())?;
        let mut line = Vec::new();
        // Take caps memory allocation before looking for a newline from an untrusted child.
        let n = (&mut p.stdout)
            .take((MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)
            .await
            .map_err(|_| crashed())?;
        if n == 0 {
            return Err(crashed());
        }
        if !line.ends_with(b"\n") {
            return Err(TypedError::new(
                "PROTOCOL_INVALID",
                "The runtime sent an incomplete or oversized message.",
                "Reconnect the session.",
                true,
            ));
        }
        let value = decode_reply(&line, &id)?;
        if method == "session.open" {
            let session = value["sessionId"]
                .as_str()
                .filter(|v| !v.is_empty())
                .ok_or_else(crashed)?;
            self.session_id = Some(session.to_string());
        } else if matches!(method, "session.health" | "location.set" | "location.clear")
            && value["sessionId"].as_str() != self.session_id.as_deref()
        {
            return Err(TypedError::new(
                "PROTOCOL_INVALID",
                "The developer session identity changed unexpectedly.",
                "Reconnect the selected phone before resuming.",
                true,
            ));
        } else if matches!(method, "session.close" | "runtime.shutdown") {
            self.session_id = None;
        }
        Ok(value)
    }
}
fn crashed() -> TypedError {
    TypedError::new(
        "RUNTIME_EXITED",
        "The owned developer runtime stopped responding.",
        "Reconnect the selected device; the last acknowledged location is retained for recovery.",
        true,
    )
}
#[async_trait]
impl DeveloperRuntime for JsonlRuntime {
    async fn call(&mut self, method: &str, params: Value) -> Result<Value, TypedError> {
        // Parent deadlines allow the child to send its typed timeout and finish cleanup.
        let deadline = match method {
            "session.open" | "pairing.prepare" => 90,
            "device.prepare" => 300,
            "device.trust" => 50,
            "device.list" => 70,
            "wireless.discover" => 45,
            "session.close" | "runtime.shutdown" => 35,
            _ => 20,
        };
        let result =
            tokio::time::timeout(Duration::from_secs(deadline), self.exchange(method, params))
                .await;
        match result {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => {
                if matches!(error.code.as_str(), "RUNTIME_EXITED" | "PROTOCOL_INVALID") {
                    self.terminate().await;
                }
                Err(error)
            }
            Err(_) => {
                self.terminate().await;
                Err(TypedError::new(
                    "RUNTIME_TIMEOUT",
                    "The developer operation exceeded its deadline.",
                    "Check the selected device and reconnect.",
                    true,
                ))
            }
        }
    }
    async fn terminate(&mut self) {
        self.session_id = None;
        if let Some(mut p) = self.process.take() {
            let _ = p.child.kill().await;
            let _ = tokio::time::timeout(Duration::from_secs(3), p.child.wait()).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn empty_or_restarted_runtime_cannot_attest_restore() {
        let mut runtime = JsonlRuntime::new(RuntimeConfig {
            executable: std::env::temp_dir().join("absent-meridian-runtime"),
            arguments: vec![],
            working_directory: None,
        });
        for method in ["session.close", "location.clear"] {
            assert_eq!(
                runtime.call(method, json!({})).await.unwrap_err().code,
                "RUNTIME_EXITED"
            );
        }
        runtime.session_id = Some("previous-child-session".into());
        runtime.terminate().await;
        assert_eq!(
            runtime
                .call("session.close", json!({}))
                .await
                .unwrap_err()
                .code,
            "RUNTIME_EXITED"
        );
    }
}

/// Explicitly selected demo adapter. Never a fallback for failed physical hardware.
pub struct DemoRuntime;
#[async_trait]
impl DeveloperRuntime for DemoRuntime {
    async fn call(&mut self, method: &str, params: Value) -> Result<Value, TypedError> {
        Ok(match method {
            "device.list" => {
                json!({"devices":[{"id":"DEMO","name":"Demo iPhone","productVersion":"Simulation","connectionType":"demo"}],"usbmuxReachable":true})
            }
            "session.open" => {
                json!({"deviceIdRedacted":"DEMO","deviceName":"Demo iPhone","productVersion":"Simulation","transport":"usb","developerServicesReady":true,"sessionId":"DEMO"})
            }
            "session.health" => json!({"healthy":true,"sessionId":"DEMO"}),
            "location.set" => {
                json!({"latitude":params["latitude"],"longitude":params["longitude"],"sessionId":"DEMO"})
            }
            "location.clear" => json!({"cleared":true,"sessionId":"DEMO"}),
            "session.close" => json!({"closed":true,"cleared":true}),
            "device.trust" => json!({"lockdownTrusted":true}),
            "device.prepare" => json!({"developerImageState":"mounted"}),
            "pairing.prepare" => json!({"remotePairingState":"verified","deviceIdRedacted":"DEMO"}),
            "diagnostics.get" | "runtime.info" => {
                json!({"mode":"DEMO","physicalDeviceVerified":false})
            }
            "runtime.shutdown" => json!({"closed":true}),
            _ => return Err(TypedError::invalid("Unknown demo method.")),
        })
    }
    async fn terminate(&mut self) {}
}

#[cfg(windows)]
mod job {
    use super::*;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
    };
    pub struct Job(HANDLE);
    unsafe impl Send for Job {}
    unsafe impl Sync for Job {}
    impl Job {
        pub fn assign(child: &Child) -> Result<Self, TypedError> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(TypedError::new(
                        "PROCESS_OWNERSHIP_FAILED",
                        "Could not establish owned helper cleanup.",
                        "Check Windows application policy.",
                        false,
                    ));
                }
                let owned = Self(handle);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as _,
                    std::mem::size_of_val(&info) as u32,
                ) == 0
                    || AssignProcessToJobObject(
                        handle,
                        child.raw_handle().ok_or_else(crashed)? as HANDLE,
                    ) == 0
                {
                    return Err(TypedError::new(
                        "PROCESS_OWNERSHIP_FAILED",
                        "Windows refused owned helper process containment.",
                        "Check application policy before connecting a device.",
                        false,
                    ));
                }
                Ok(owned)
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
