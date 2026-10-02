use base64::Engine;
use idevice::remote_pairing::RpPairingFile;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};
use zeroize::Zeroizing;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Point {
    pub latitude: f64,
    pub longitude: f64,
}
impl Point {
    pub fn validate(&self) -> Result<(), Fault> {
        if self.latitude.is_finite()
            && self.longitude.is_finite()
            && (-90.0..=90.0).contains(&self.latitude)
            && (-180.0..=180.0).contains(&self.longitude)
        {
            Ok(())
        } else {
            Err(Fault::new(
                "INVALID_COORDINATE",
                "Enter latitude from -90 to 90 and longitude from -180 to 180.",
            ))
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Configuration {
    pub schema_version: u8,
    pub expected_device_id: String,
    pub remote_address: String,
    pub remote_port: u16,
    pub pairing_record: String,
    #[serde(default)]
    pub connection_mode: ConnectionMode,
    #[serde(default)]
    pub local_tunnel: Option<LocalTunnel>,
}
#[derive(Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionMode {
    #[default]
    RemotePairing,
    LocalRemotePairing,
    LocalCoreDevice,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalTunnel {
    pub interface_address: String,
    pub peer_address: String,
}
impl Configuration {
    pub fn endpoint(&self) -> Result<SocketAddr, Fault> {
        if ![1, 2].contains(&self.schema_version)
            || self.remote_port == 0
            || self.expected_device_id.len() < 8
            || self.expected_device_id.len() > 128
        {
            return Err(Fault::new(
                "PAIRING_RECORD_INVALID",
                "Repair mobile setup from your trusted desktop.",
            ));
        }
        let ip: IpAddr = self.remote_address.parse().map_err(|_| {
            Fault::new(
                "ENDPOINT_INVALID",
                "Use the numeric address provided by your local developer connection.",
            )
        })?;
        let local = match ip {
            IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
            IpAddr::V6(ip) => {
                ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
            }
        };
        if !local || ip.is_unspecified() || ip.is_multicast() {
            return Err(Fault::new(
                "ENDPOINT_INVALID",
                "The developer endpoint must be on this device or a private local network.",
            ));
        }
        if self.connection_mode != ConnectionMode::RemotePairing {
            let invalid = || {
                Fault::new(
                    "LOCAL_SETUP_INVALID",
                    "Prepare this iPhone's local connection configuration from the trusted desktop.",
                )
            };
            if self.schema_version != 2 || self.connection_mode != ConnectionMode::LocalCoreDevice {
                return Err(invalid());
            }
            let tunnel = self.local_tunnel.as_ref().ok_or_else(invalid)?;
            let interface = tunnel.interface_address.parse().map_err(|_| invalid())?;
            let peer = tunnel.peer_address.parse().map_err(|_| invalid())?;
            if !crate::local_packets::validate_pair(interface, peer)
                || interface != std::net::Ipv4Addr::new(10, 83, 0, 2)
                || peer != std::net::Ipv4Addr::new(10, 83, 0, 1)
                || ip != IpAddr::V4(peer)
                || self.remote_port != 62078
            {
                return Err(invalid());
            }
        } else if self.schema_version != 1 || self.local_tunnel.is_some() {
            return Err(Fault::new(
                "LOCAL_SETUP_INVALID",
                "Schema 2 requires an explicit on-device connection mode.",
            ));
        }
        Ok(SocketAddr::new(ip, self.remote_port))
    }

    pub fn validate_pairing(&self) -> Result<(), Fault> {
        if self.connection_mode == ConnectionMode::LocalCoreDevice {
            self.lockdown_pairing().map(|_| ())
        } else {
            self.pairing().map(|_| ())
        }
    }
    pub fn lockdown_pairing(&self) -> Result<idevice::pairing_file::PairingFile, Fault> {
        let invalid = || {
            Fault::new(
                "PAIRING_RECORD_INVALID",
                "A legitimate USB Lockdown pairing record is required for local CoreDevice setup.",
            )
        };
        if self.pairing_record.len() > 65536 {
            return Err(invalid());
        }
        let bytes = Zeroizing::new(
            base64::engine::general_purpose::STANDARD
                .decode(&self.pairing_record)
                .map_err(|_| invalid())?,
        );
        let pairing =
            idevice::pairing_file::PairingFile::from_bytes(&bytes).map_err(|_| invalid())?;
        // The exported schema must bind the record to the selected device before
        // any tunnel starts; live Lockdown and RSD identities are checked again.
        verify_identity(&self.expected_device_id, pairing.udid.as_deref())?;
        Ok(pairing)
    }

    pub fn pairing(&self) -> Result<RpPairingFile, Fault> {
        let bad = || {
            Fault::new(
                "PAIRING_RECORD_INVALID",
                "Repair mobile setup from your trusted desktop.",
            )
        };
        if self.pairing_record.len() > 65536 {
            return Err(bad());
        }
        let bytes = Zeroizing::new(
            base64::engine::general_purpose::STANDARD
                .decode(&self.pairing_record)
                .map_err(|_| bad())?,
        );
        // Upstream from_bytes emits the complete secret dictionary at DEBUG. Parse only
        // the documented fields locally, with no Debug implementation or logging.
        let mut dict: plist::Dictionary = plist::from_bytes(&bytes).map_err(|_| bad())?;
        let mut private = Zeroizing::new(
            dict.remove("private_key")
                .and_then(|v| v.into_data())
                .ok_or_else(bad)?,
        );
        let mut raw: [u8; 32] = private.as_slice().try_into().map_err(|_| bad())?;
        let signing = ed25519_dalek::SigningKey::from_bytes(&raw);
        raw.fill(0);
        private.fill(0);
        let public: [u8; 32] = dict
            .remove("public_key")
            .and_then(|v| v.into_data())
            .ok_or_else(bad)?
            .as_slice()
            .try_into()
            .map_err(|_| bad())?;
        if signing.verifying_key().to_bytes() != public {
            return Err(bad());
        }
        let identifier = dict
            .remove("identifier")
            .and_then(|v| v.into_string())
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(bad)?;
        let alt_irk = dict.remove("alt_irk").and_then(|v| v.into_data());
        if alt_irk.as_ref().is_some_and(|b| b.len() != 16) {
            return Err(bad());
        }
        Ok(RpPairingFile {
            e_private_key: signing,
            e_public_key: ed25519_dalek::VerifyingKey::from_bytes(&public).map_err(|_| bad())?,
            identifier,
            alt_irk,
        })
    }
}
impl Drop for Configuration {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.pairing_record.zeroize();
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Fault {
    pub code: String,
    pub message: String,
}
impl Fault {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Fault {}

pub fn verify_identity(expected: &str, actual: Option<&str>) -> Result<(), Fault> {
    match actual {
        Some(actual) if !expected.is_empty() && expected == actual => Ok(()),
        Some(_) => Err(Fault::new(
            "DEVICE_IDENTITY_MISMATCH",
            "The developer connection belongs to another device. Repair pairing before continuing.",
        )),
        None => Err(Fault::new(
            "DEVICE_IDENTITY_UNAVAILABLE",
            "The developer connection did not provide a verifiable device identity.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn coordinates_reject_nonfinite_and_range_errors() {
        for point in [
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
            assert!(point.validate().is_err());
        }
    }
    #[test]
    fn identity_never_guesses() {
        assert!(verify_identity("device-a", Some("device-b")).is_err());
        assert!(verify_identity("device-a", None).is_err());
        assert!(verify_identity("device-a", Some("device-a")).is_ok());
    }
    #[test]
    fn endpoints_reject_public_and_unspecified_addresses() {
        for address in ["8.8.8.8", "0.0.0.0", "::", "example.org"] {
            assert!(
                Configuration {
                    schema_version: 1,
                    expected_device_id: "device-a".into(),
                    remote_address: address.into(),
                    remote_port: 1234,
                    pairing_record: String::new(),
                    connection_mode: ConnectionMode::RemotePairing,
                    local_tunnel: None,
                }
                .endpoint()
                .is_err()
            );
        }
    }

    #[test]
    fn local_core_device_requires_the_verified_fixed_profile() {
        let mut config = Configuration {
            schema_version: 2,
            expected_device_id: "device-a".into(),
            remote_address: "10.83.0.1".into(),
            remote_port: 62078,
            pairing_record: String::new(),
            connection_mode: ConnectionMode::LocalCoreDevice,
            local_tunnel: Some(LocalTunnel {
                interface_address: "10.83.0.2".into(),
                peer_address: "10.83.0.1".into(),
            }),
        };
        assert!(config.endpoint().is_ok());
        config.remote_address = "10.83.0.2".into();
        assert!(config.endpoint().is_err());
        config.remote_address = "10.83.0.1".into();
        config.local_tunnel.as_mut().unwrap().interface_address = "10.83.0.3".into();
        assert!(config.endpoint().is_err());
    }
}
