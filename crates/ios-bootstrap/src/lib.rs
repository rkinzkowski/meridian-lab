//! Integrity and constrained app-container bootstrap. Hardware support is UNVERIFIED.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactManifest {
    pub mobile_version: String,
    pub bundle_identifier: String,
    pub pairing_schema_version: u32,
    pub minimum_desktop_version: String,
    pub size: u64,
    pub sha256: String,
}
pub fn verify_artifact(path: &Path, manifest: &ArtifactManifest) -> Result<(), String> {
    if manifest.size == 0
        || manifest.size > 512 * 1024 * 1024
        || ![1, 2].contains(&manifest.pairing_schema_version)
        || manifest.sha256.len() != 64
    {
        return Err("Invalid mobile artifact manifest".into());
    }
    let mut file = std::fs::File::open(path).map_err(|_| "Mobile artifact is missing")?;
    if file
        .metadata()
        .map_err(|_| "Cannot inspect mobile artifact")?
        .len()
        != manifest.size
    {
        return Err("Mobile artifact size does not match manifest".into());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "Cannot read mobile artifact")?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("{:x}", digest.finalize()) != manifest.sha256.to_lowercase() {
        return Err("Mobile artifact integrity check failed".into());
    }
    Ok(())
}
pub fn validate_bundle_id(value: &str) -> Result<(), String> {
    if value.len() < 3
        || value.len() > 255
        || !value.contains('.')
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
    {
        Err("Invalid installed bundle identifier".into())
    } else {
        Ok(())
    }
}
/// The destination is fixed by the app's schema; caller-supplied paths are never accepted.
pub const MOBILE_CONFIGURATION_PATH: &str = "Documents/meridian-device.json";
pub const LOCAL_CORE_DEVICE_INTERFACE: &str = "10.83.0.2";
pub const LOCAL_CORE_DEVICE_PEER: &str = "10.83.0.1";
pub const LOCAL_CORE_DEVICE_PORT: u16 = 62078;

#[cfg(feature = "signing")]
pub mod signing;

#[cfg(feature = "device")]
pub mod device {
    use super::*;
    use base64::Engine;
    use idevice::{
        IdeviceService,
        afc::{AfcClient, opcode::AfcFopenMode},
        house_arrest::HouseArrestClient,
        installation_proxy::InstallationProxyClient,
        provider::IdeviceProvider,
    };
    use tokio::time::{Duration, timeout};

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct MobileConfiguration {
        schema_version: u32,
        expected_device_id: String,
        remote_address: String,
        remote_port: u16,
        pairing_record: String,
        #[serde(default)]
        connection_mode: Option<String>,
        #[serde(default)]
        local_tunnel: Option<LocalTunnel>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct LocalTunnel {
        interface_address: String,
        peer_address: String,
    }

    fn validate_selected_id(expected_device_id: &str) -> Result<(), String> {
        if expected_device_id.len() < 16
            || expected_device_id.len() > 64
            || !expected_device_id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() || b == b'-')
        {
            Err("Invalid selected device identity".into())
        } else {
            Ok(())
        }
    }

    fn validate_mobile_configuration(
        configuration: &[u8],
        expected_device_id: &str,
    ) -> Result<(), String> {
        if configuration.is_empty() || configuration.len() > 131072 {
            return Err("Invalid mobile configuration size".into());
        }
        let json: MobileConfiguration =
            serde_json::from_slice(configuration).map_err(|_| "Invalid mobile configuration")?;
        if json.expected_device_id != expected_device_id || json.remote_port == 0 {
            return Err("Mobile configuration identity differs from selected device".into());
        }
        let encoded = base64::engine::general_purpose::STANDARD
            .decode(&json.pairing_record)
            .map_err(|_| "Invalid mobile pairing record")?;
        if encoded.is_empty() || encoded.len() > 49152 {
            return Err("Invalid mobile pairing record size".into());
        }
        match json.schema_version {
            1 if json.connection_mode.as_deref().unwrap_or("remotePairing") == "remotePairing"
                && json.local_tunnel.is_none() =>
            {
                let dict: plist::Dictionary =
                    plist::from_bytes(&encoded).map_err(|_| "Invalid RemotePairing record")?;
                let private = dict.get("private_key").and_then(|v| v.as_data());
                let public = dict.get("public_key").and_then(|v| v.as_data());
                let identifier = dict.get("identifier").and_then(|v| v.as_string());
                if private.map_or(true, |value| value.len() != 32)
                    || public.map_or(true, |value| value.len() != 32)
                    || identifier.map_or(true, |value| value.is_empty() || value.len() > 128)
                {
                    return Err("Invalid RemotePairing record".into());
                }
                Ok(())
            }
            2 if json.connection_mode.as_deref() == Some("localCoreDevice") => {
                let tunnel = json
                    .local_tunnel
                    .ok_or("Schema 2 requires a local tunnel profile")?;
                if json.remote_address != tunnel.peer_address
                    || json.remote_port != LOCAL_CORE_DEVICE_PORT
                    || tunnel.interface_address != LOCAL_CORE_DEVICE_INTERFACE
                    || tunnel.peer_address != LOCAL_CORE_DEVICE_PEER
                {
                    return Err("Schema 2 local CoreDevice endpoint is invalid".into());
                }
                let pairing = idevice::pairing_file::PairingFile::from_bytes(&encoded)
                    .map_err(|_| "Invalid USB Lockdown pairing record")?;
                if pairing.udid.as_deref() != Some(expected_device_id) {
                    return Err("Mobile pairing record belongs to another device".into());
                }
                Ok(())
            }
            _ => Err("Unsupported mobile pairing schema".into()),
        }
    }

    pub async fn verify_selected(
        provider: &impl IdeviceProvider,
        expected_device_id: &str,
    ) -> Result<(), String> {
        validate_selected_id(expected_device_id)?;
        timeout(Duration::from_secs(10), async {
            let mut lockdown = idevice::lockdown::LockdownClient::connect(provider)
                .await
                .map_err(|_| "Cannot open selected device identity service")?;
            let actual = lockdown
                .get_value(Some("UniqueDeviceID"), None)
                .await
                .map_err(|_| "Cannot verify selected device identity")?;
            if actual.as_string() != Some(expected_device_id) {
                return Err("Device identity mismatch".into());
            }
            Ok(())
        })
        .await
        .map_err(|_| "Device identity check timed out".to_string())?
    }

    /// Export a complete USB Lockdown record into the bounded schema consumed by
    /// the native iOS app. The provider must be the explicitly selected USB
    /// device; no pairing record is fabricated or copied from a remote pairing.
    pub async fn export_local_core_device_configuration(
        provider: &impl IdeviceProvider,
        expected_device_id: &str,
    ) -> Result<Vec<u8>, String> {
        validate_selected_id(expected_device_id)?;
        timeout(Duration::from_secs(15), async {
            verify_selected(provider, expected_device_id).await?;
            let pairing = provider
                .get_pairing_file()
                .await
                .map_err(|_| "Cannot read the selected USB Lockdown pairing record")?;
            if pairing.udid.as_deref() != Some(expected_device_id) {
                return Err("USB Lockdown pairing record belongs to another device".into());
            }
            let pairing_bytes = pairing
                .serialize()
                .map_err(|_| "Cannot serialize the selected USB Lockdown pairing record")?;
            let configuration = serde_json::json!({
                "schemaVersion": 2,
                "connectionMode": "localCoreDevice",
                "expectedDeviceId": expected_device_id,
                "remoteAddress": LOCAL_CORE_DEVICE_PEER,
                "remotePort": LOCAL_CORE_DEVICE_PORT,
                "pairingRecord": base64::engine::general_purpose::STANDARD.encode(pairing_bytes),
                "localTunnel": {
                    "interfaceAddress": LOCAL_CORE_DEVICE_INTERFACE,
                    "peerAddress": LOCAL_CORE_DEVICE_PEER
                }
            });
            let bytes = serde_json::to_vec(&configuration)
                .map_err(|_| "Cannot encode mobile pairing configuration")?;
            validate_mobile_configuration(&bytes, expected_device_id)?;
            Ok(bytes)
        })
        .await
        .map_err(|_| "Lockdown export timed out; inspect the selected iPhone".to_string())?
    }
    /// Installs only an already development-signed, integrity-verified IPA. Apple verifies
    /// its signature. This function performs no Apple account authentication or revocation.
    pub async fn install_presigned(
        provider: &impl IdeviceProvider,
        expected_device_id: &str,
        ipa: &Path,
        manifest: &ArtifactManifest,
    ) -> Result<String, String> {
        verify_artifact(ipa, manifest)?;
        validate_bundle_id(&manifest.bundle_identifier)?;
        let bytes = std::fs::read(ipa).map_err(|_| "Unable to read IPA")?;
        if bytes.len() as u64 != manifest.size
            || format!("{:x}", Sha256::digest(&bytes)) != manifest.sha256.to_lowercase()
        {
            return Err("Mobile artifact changed after validation".into());
        }
        timeout(Duration::from_secs(180), async {
            verify_selected(provider, expected_device_id).await?;
            let mut afc = AfcClient::connect(provider)
                .await
                .map_err(|_| "Apple file service unavailable")?;
            let name = format!("PublicStaging/meridian-{}.ipa", &manifest.sha256[..16]);
            let _ = afc.mk_dir("PublicStaging").await;
            let mut file = afc
                .open(&name, AfcFopenMode::WrOnly)
                .await
                .map_err(|_| "Unable to open install staging")?;
            file.write_entire(&bytes)
                .await
                .map_err(|_| "IPA upload failed")?;
            file.close().await.map_err(|_| "IPA upload close failed")?;
            let mut installer = InstallationProxyClient::connect(provider)
                .await
                .map_err(|_| "Installation service unavailable")?;
            let installed = installer.install(&name, None).await;
            let _ = afc.remove(&name).await;
            installed.map_err(
                |_| "iPhone rejected installation; verify development signing and provisioning",
            )?;
            let apps = installer
                .get_apps(Some("User"), None)
                .await
                .map_err(|_| "Unable to verify installed app")?;
            let actual = apps
                .get(&manifest.bundle_identifier)
                .and_then(|a| a.as_dictionary())
                .and_then(|a| a.get("CFBundleIdentifier"))
                .and_then(|v| v.as_string())
                .ok_or("Installed target could not be identified")?;
            if actual != manifest.bundle_identifier {
                return Err("Installed target identity mismatch".into());
            }
            Ok(actual.to_string())
        })
        .await
        .map_err(|_| "Installation timed out; inspect the iPhone before retrying".to_string())?
    }
    /// Call only for a bundle discovered by InstallationProxy and only after operational
    /// RemotePairing validation. A readback confirms bytes, not developer-session readiness.
    pub async fn write_mobile_configuration(
        provider: &impl IdeviceProvider,
        expected_device_id: &str,
        actual_bundle: &str,
        configuration: &[u8],
    ) -> Result<(), String> {
        validate_bundle_id(actual_bundle)?;
        validate_mobile_configuration(configuration, expected_device_id)?;
        timeout(Duration::from_secs(20), async {
            verify_selected(provider, expected_device_id).await?;
            let mut installer = InstallationProxyClient::connect(provider)
                .await
                .map_err(|_| "Cannot verify installed app")?;
            if !installer
                .get_apps(Some("User"), Some(vec![actual_bundle.into()]))
                .await
                .map_err(|_| "Cannot verify installed app")?
                .contains_key(actual_bundle)
            {
                return Err("Target app is not installed".into());
            }
            let house = HouseArrestClient::connect(provider)
                .await
                .map_err(|_| "App container unavailable")?;
            let mut afc = house
                .vend_documents(actual_bundle)
                .await
                .map_err(|_| "App Documents access unavailable")?;
            let staging = "Documents/meridian-device.pending";
            let mut file = afc
                .open(staging, AfcFopenMode::WrOnly)
                .await
                .map_err(|_| "Cannot stage mobile pairing")?;
            file.write_entire(configuration)
                .await
                .map_err(|_| "Cannot write mobile pairing")?;
            file.close()
                .await
                .map_err(|_| "Cannot close pairing file")?;
            let mut check = afc
                .open(staging, AfcFopenMode::RdOnly)
                .await
                .map_err(|_| "Cannot read pairing back")?;
            let read = check
                .read_n(configuration.len() + 1)
                .await
                .map_err(|_| "Pairing readback failed")?;
            check
                .close()
                .await
                .map_err(|_| "Pairing readback close failed")?;
            if read != configuration {
                return Err("Pairing readback differs".into());
            }
            afc.rename(staging, MOBILE_CONFIGURATION_PATH)
                .await
                .map_err(|_| "Cannot commit verified pairing bytes")?;
            Ok(())
        })
        .await
        .map_err(|_| "Pairing placement timed out".to_string())?
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundle_paths_cannot_escape() {
        for s in ["../app", "app/name", "a", "com.app\n"] {
            assert!(validate_bundle_id(s).is_err());
        }
        assert!(validate_bundle_id("dev.meridianlab.ios").is_ok());
    }
}
