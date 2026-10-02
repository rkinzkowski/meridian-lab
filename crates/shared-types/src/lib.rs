use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
}
impl Point {
    pub fn validate(&self) -> Result<(), TypedError> {
        if !self.latitude.is_finite()
            || !self.longitude.is_finite()
            || self.latitude.abs() > 90.0
            || self.longitude.abs() > 180.0
        {
            return Err(TypedError::new(
                "INVALID_COORDINATE",
                "Coordinate is outside the valid latitude or longitude range.",
                "Use latitude −90 to 90 and longitude −180 to 180.",
                false,
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TypedError {
    pub code: String,
    pub message: String,
    pub recovery: String,
    pub retryable: bool,
}
impl TypedError {
    pub fn new(code: &str, message: &str, recovery: &str, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            recovery: recovery.into(),
            retryable,
        }
    }
    pub fn invalid(message: &str) -> Self {
        Self::new(
            "INVALID_REQUEST",
            message,
            "Check the request and retry.",
            false,
        )
    }
    pub fn disconnected() -> Self {
        Self::new(
            "SESSION_NOT_READY",
            "No developer location session is ready.",
            "Select and connect your development-enabled iPhone.",
            true,
        )
    }
}
impl std::fmt::Display for TypedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for TypedError {}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Hardware,
    Demo,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    Usb,
    Wifi,
    Rsd,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    pub product_version: String,
    pub connection_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectRequest {
    pub udid: String,
    pub transport: Transport,
    pub rsd_address: Option<String>,
    pub rsd_port: Option<u16>,
}
impl ConnectRequest {
    pub fn validate(&self) -> Result<(), TypedError> {
        if self.udid.is_empty()
            || self.udid.len() > 128
            || !self
                .udid
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(TypedError::invalid("Select a valid device identity."));
        }
        if self.transport == Transport::Rsd
            && (self
                .rsd_address
                .as_ref()
                .is_none_or(|a| a.is_empty() || a.len() > 255)
                || self.rsd_port.is_none_or(|p| p == 0))
        {
            return Err(TypedError::invalid(
                "An explicit RSD connection requires an address and port.",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RouteSpec {
    pub id: String,
    pub name: String,
    pub points: Vec<Point>,
    pub speed_mps: f64,
    pub r#loop: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteProgress {
    pub id: String,
    pub name: String,
    pub distance_meters: f64,
    pub total_meters: f64,
    pub speed_mps: f64,
    pub paused: bool,
    pub finished: bool,
    pub r#loop: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub platform: String,
    pub mode: Mode,
    pub setup_stage: String,
    pub device_found: bool,
    pub device_name: Option<String>,
    pub product_version: Option<String>,
    pub device_id_redacted: Option<String>,
    pub usb_cable_present: bool,
    pub lockdown_trusted: bool,
    pub developer_mode_state: String,
    pub developer_image_state: String,
    pub developer_services_ready: bool,
    pub remote_pairing_state: String,
    pub active_transport: Option<Transport>,
    pub tunnel_state: String,
    pub rsd_state: String,
    pub location_session_state: String,
    pub held_point: Option<Point>,
    pub active_route: Option<RouteProgress>,
    pub recovering: bool,
    pub generation: u64,
    pub last_typed_error: Option<TypedError>,
    pub physical_device_verified: bool,
    pub usbmux_reachable: bool,
    pub busy: bool,
}
impl Default for RuntimeStatus {
    fn default() -> Self {
        Self {
            platform: std::env::consts::OS.into(),
            mode: Mode::Hardware,
            setup_stage: "requirements".into(),
            device_found: false,
            device_name: None,
            product_version: None,
            device_id_redacted: None,
            usb_cable_present: false,
            lockdown_trusted: false,
            developer_mode_state: "unknown".into(),
            developer_image_state: "unknown".into(),
            developer_services_ready: false,
            remote_pairing_state: "unknown".into(),
            active_transport: None,
            tunnel_state: "closed".into(),
            rsd_state: "closed".into(),
            location_session_state: "closed".into(),
            held_point: None,
            active_route: None,
            recovering: false,
            generation: 0,
            last_typed_error: None,
            physical_device_verified: false,
            usbmux_reachable: false,
            busy: false,
        }
    }
}
impl RuntimeStatus {
    pub fn derive_setup(&mut self) {
        self.setup_stage = if self.mode == Mode::Demo || self.developer_services_ready {
            "ready"
        } else if !self.usbmux_reachable && self.platform == "windows" {
            "requirements"
        } else if !self.device_found || !self.lockdown_trusted {
            "connect"
        } else {
            "developer"
        }
        .into();
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedPlace {
    pub id: String,
    pub name: String,
    pub point: Point,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub label: String,
    pub point: Point,
    pub timestamp_ms: u64,
    pub mode: Mode,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Library {
    pub places: Vec<SavedPlace>,
    pub routes: Vec<RouteSpec>,
    pub history: Vec<HistoryEntry>,
    pub history_enabled: bool,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            places: vec![],
            routes: vec![],
            history: vec![],
            history_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_nonfinite_and_outside_coordinates() {
        for p in [
            Point {
                latitude: f64::NAN,
                longitude: 0.,
            },
            Point {
                latitude: 91.,
                longitude: 0.,
            },
            Point {
                latitude: 0.,
                longitude: 181.,
            },
        ] {
            assert!(p.validate().is_err());
        }
    }
    #[test]
    fn readiness_rolls_back() {
        let mut s = RuntimeStatus {
            usbmux_reachable: true,
            device_found: true,
            lockdown_trusted: true,
            ..Default::default()
        };
        s.derive_setup();
        assert_eq!(s.setup_stage, "developer");
        s.developer_services_ready = true;
        s.derive_setup();
        assert_eq!(s.setup_stage, "ready");
        s.developer_services_ready = false;
        s.device_found = false;
        s.derive_setup();
        assert_eq!(s.setup_stage, "connect");
    }
}
