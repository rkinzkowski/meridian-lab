//! Explicit, presigned-IPA bootstrap over the already trusted usbmuxd pairing.
//! Raw identifiers are accepted as input but are never printed.
use meridian_ios_bootstrap::{device, ArtifactManifest};
use idevice::{provider::UsbmuxdProvider, usbmuxd::{Connection, UsbmuxdAddr, UsbmuxdConnection}};
use std::{path::PathBuf, process::ExitCode};

#[derive(Debug, PartialEq, Eq)]
enum Command { List, Install { ipa: PathBuf, manifest: PathBuf, udid: String }, Setup { manifest: PathBuf, udid: String } }

fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut it = args.into_iter();
    let command = it.next().ok_or_else(usage)?;
    let mut ipa = None; let mut manifest = None; let mut udid = None;
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("Missing value for {arg}"));
        match arg.as_str() {
            "--ipa" => ipa = Some(PathBuf::from(value()?)),
            "--manifest" => manifest = Some(PathBuf::from(value()?)),
            "--udid" => udid = Some(value()?),
            _ => return Err(format!("Unknown argument: {arg}\n{}", usage())),
        }
    }
    match command.as_str() {
        "list" if ipa.is_none() && manifest.is_none() && udid.is_none() => Ok(Command::List),
        "install" => Ok(Command::Install { ipa: ipa.ok_or("--ipa is required")?, manifest: manifest.ok_or("--manifest is required")?, udid: checked_udid(udid)? }),
        "setup" => Ok(Command::Setup { manifest: manifest.ok_or("--manifest is required")?, udid: checked_udid(udid)? }),
        _ => Err(usage()),
    }
}

fn checked_udid(value: Option<String>) -> Result<String, String> {
    let value = value.ok_or("--udid is required; run `list` first to see sanitized device aliases")?;
    if value.len() < 16 || value.len() > 64 || !value.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') { return Err("Invalid device identifier".into()); }
    Ok(value)
}
fn usage() -> String { "Usage: meridian-ios-bootstrap list | install --ipa FILE --manifest FILE --udid ID | setup --manifest FILE --udid ID".into() }

async fn selected(udid: &str) -> Result<UsbmuxdProvider, String> {
    let addr = UsbmuxdAddr::from_env_var().map_err(|_| "Invalid usbmuxd address".to_string())?;
    let mut mux = UsbmuxdConnection::new(addr.to_socket().await.map_err(|_| "Cannot connect to usbmuxd" )?, 0);
    let devices = mux.get_devices().await.map_err(|_| "Cannot list trusted iPhones".to_string())?;
    let device = devices.into_iter().find(|d| d.udid == udid).ok_or("Selected iPhone is not connected or trusted")?;
    let provider = device.to_provider(addr, "Meridian Lab bootstrap");
    Ok(provider)
}

fn transport_name(connection: &Connection) -> &'static str {
    match connection { Connection::Usb => "USB", Connection::Network(_) => "network", Connection::Unknown(_) => "unknown" }
}

async fn run(command: Command) -> Result<(), String> {
    match command {
        Command::List => {
            let addr = UsbmuxdAddr::from_env_var().map_err(|_| "Invalid usbmuxd address".to_string())?;
            let mut mux = UsbmuxdConnection::new(addr.to_socket().await.map_err(|_| "Cannot connect to usbmuxd" )?, 0);
            let devices = mux.get_devices().await.map_err(|_| "Cannot list trusted iPhones".to_string())?;
            for (index, d) in devices.iter().enumerate() { println!("device-{} ({})", index + 1, transport_name(&d.connection_type)); }
            if devices.is_empty() { println!("No trusted iPhone is currently connected"); }
        }
        Command::Install { ipa, manifest, udid } => {
            let manifest: ArtifactManifest = serde_json::from_slice(&std::fs::read(manifest).map_err(|_| "Cannot read manifest")?).map_err(|_| "Invalid manifest JSON")?;
            let provider = selected(&udid).await?;
            let bundle = device::install_presigned(&provider, &udid, &ipa, &manifest).await?;
            let configuration = device::export_local_core_device_configuration(&provider, &udid).await?;
            device::write_mobile_configuration(&provider, &udid, &bundle, &configuration).await?;
            println!("Installed and configured the selected Meridian app.");
        }
        Command::Setup { manifest, udid } => {
            let manifest: ArtifactManifest = serde_json::from_slice(&std::fs::read(manifest).map_err(|_| "Cannot read manifest")?).map_err(|_| "Invalid manifest JSON")?;
            let provider = selected(&udid).await?;
            let configuration = device::export_local_core_device_configuration(&provider, &udid).await?;
            device::write_mobile_configuration(&provider, &udid, &manifest.bundle_identifier, &configuration).await?;
            println!("Configured the already installed Meridian app.");
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode { match parse(std::env::args().skip(1)) { Ok(c) => match run(c).await { Ok(()) => ExitCode::SUCCESS, Err(e) => { eprintln!("Bootstrap failed: {e}"); ExitCode::FAILURE } }, Err(e) => { eprintln!("{e}"); ExitCode::FAILURE } } }

#[cfg(test)]
mod tests { use super::*; #[test] fn parser_requires_explicit_id() { assert!(parse(["setup".into(), "--manifest".into(), "m".into()]).is_err()); } #[test] fn list_has_no_sensitive_input() { assert_eq!(parse(["list".into()]).unwrap(), Command::List); } }
