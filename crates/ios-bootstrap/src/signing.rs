//! Explicit-provider development signing. This API is not exposed by the desktop UI yet.
//! No default anisette server, silent team selection, certificate revocation, or disk credentials.
use super::{ArtifactManifest, validate_bundle_id, verify_artifact};
use idevice::{
    IdeviceService, installation_proxy::InstallationProxyClient, provider::IdeviceProvider,
};
use isideload::{
    anisette::AnisetteProvider,
    auth::apple_account::{AppleAccount, TwoFactorCallbackParams, TwoFactorCallbackResponse},
    dev::{
        developer_session::DeveloperSession,
        devices::DevicesApi,
        teams::{DeveloperTeam, TeamsApi},
    },
    sideload::{
        builder::{MaxCertsBehavior, SideloaderBuilder, TeamSelection},
        sideloader::Sideloader,
    },
    util::{device::IdeviceInfo, storage::InMemoryStorage},
};
use std::{
    future::Future,
    path::{Path, PathBuf},
};
use tokio::time::{Duration, timeout};
use zeroize::Zeroizing;

pub struct DevelopmentSigningSession {
    sideloader: Sideloader,
    teams: Vec<DeveloperTeam>,
}
pub struct SignedBuild {
    pub app_bundle: PathBuf,
    pub actual_bundle_identifier: String,
}
impl DevelopmentSigningSession {
    /// The caller supplies a reviewed local or explicitly configured provider. The
    /// callback may await an independently dispatched UI 2FA response; do not serialize
    /// that response behind this future. Drop this future to cancel authentication.
    pub async fn authenticate<P, C, F>(
        email: &str,
        password: Zeroizing<String>,
        provider: P,
        two_factor: C,
    ) -> Result<Self, String>
    where
        P: AnisetteProvider + Send + Sync + 'static,
        C: Fn(TwoFactorCallbackParams) -> F + Send + Sync,
        F: Future<Output = Result<TwoFactorCallbackResponse, rootcause::Report>> + Send,
    {
        // Raw upstream reports can contain account details. Only static classified
        // messages leave this boundary; the library's debug mode stays disabled.
        timeout(Duration::from_secs(600), async {
            let mut account = AppleAccount::builder(email)
                .anisette_provider(provider)
                .login(&password, two_factor)
                .await
                .map_err(
                    |_| "APPLE_AUTH_FAILED: Apple development authentication did not complete",
                )?;
            drop(password);
            let mut developer = DeveloperSession::from_account(&mut account)
                .await
                .map_err(|_| "APPLE_AUTH_FAILED: A development session could not be opened")?;
            let teams = developer
                .list_teams()
                .await
                .map_err(|_| "DEVELOPER_TEAM_UNAVAILABLE: Developer teams could not be read")?;
            if teams.is_empty() {
                return Err(
                    "DEVELOPER_TEAM_UNAVAILABLE: This account has no available developer team"
                        .into(),
                );
            }
            let sideloader = SideloaderBuilder::new(developer, email.into())
                .max_certs_behavior(MaxCertsBehavior::Error)
                .team_selection(TeamSelection::PromptAlways(|_| None))
                .storage(Box::new(InMemoryStorage::new()))
                .machine_name("Meridian Lab development".into())
                .build();
            Ok(Self { sideloader, teams })
        })
        .await
        .map_err(|_| "APPLE_AUTH_FAILED: Authentication timed out".to_string())?
    }
    pub fn available_teams(&self) -> &[DeveloperTeam] {
        &self.teams
    }

    /// Registers only the explicitly selected device, then signs the verified project
    /// artifact with the explicitly selected team. Reaching the certificate limit fails.
    pub async fn sign(
        &mut self,
        provider: &impl IdeviceProvider,
        expected_device_id: &str,
        ipa: &Path,
        manifest: &ArtifactManifest,
        selected_team_id: &str,
    ) -> Result<SignedBuild, String> {
        verify_artifact(ipa, manifest)?;
        validate_bundle_id(&manifest.bundle_identifier)?;
        let team = self
            .teams
            .iter()
            .find(|t| t.team_id == selected_team_id)
            .cloned()
            .ok_or("DEVELOPER_TEAM_UNAVAILABLE: Select an available team")?;
        timeout(Duration::from_secs(300),async{
            let device=IdeviceInfo::from_device(provider).await.map_err(|_|"TRUST_REQUIRED: Connect and trust the selected iPhone")?;
            if expected_device_id.is_empty()||device.udid!=expected_device_id{return Err("DEVICE_IDENTITY_MISMATCH: The connected phone is not the selected device".into());}
            self.sideloader.get_dev_session().ensure_device_registered(&team,&device.name,&device.udid,None).await.map_err(|_|"PROVISIONING_FAILED: Device registration failed")?;
            let (app_bundle,special)=self.sideloader.sign_app(ipa.to_owned(),Some(team.clone()),false,Some(|_:f32|async{})).await.map_err(|_|"SIGNING_FAILED: Development signing failed; check certificates and provisioning")?;
            if special.is_some(){return Err("SIGNING_FAILED: The artifact is not an ordinary Meridian development build".into());}
            let info:plist::Dictionary=plist::from_file(app_bundle.join("Info.plist")).map_err(|_|"SIGNING_FAILED: Signed build metadata is unreadable")?;
            let actual=info.get("CFBundleIdentifier").and_then(|v|v.as_string()).ok_or("SIGNING_FAILED: Signed bundle identity is missing")?;
            validate_bundle_id(actual)?;
            if actual!=format!("{}.{}",manifest.bundle_identifier,team.team_id){return Err("SIGNING_FAILED: Signed build identity differs from the selected project and team".into());}
            Ok(SignedBuild{actual_bundle_identifier:actual.into(),app_bundle})
        }).await.map_err(|_|"SIGNING_FAILED: Signing timed out; inspect development resources before retrying".to_string())?
    }
}

/// Installs a signed build and asks InstallationProxy for the actual bundle. This
/// confirms installation only; launching and operational mobile pairing remain gates.
pub async fn install_signed(
    provider: &impl IdeviceProvider,
    expected_device_id: &str,
    build: &SignedBuild,
) -> Result<String, String> {
    validate_bundle_id(&build.actual_bundle_identifier)?;
    timeout(Duration::from_secs(180), async {
        super::device::verify_selected(provider, expected_device_id).await?;
        idevice::utils::installation::install_package(provider, &build.app_bundle, None)
            .await
            .map_err(|_| "INSTALL_FAILED: The iPhone rejected installation")?;
        let mut proxy = InstallationProxyClient::connect(provider)
            .await
            .map_err(|_| "INSTALL_FAILED: Cannot verify the installed app")?;
        let apps = proxy
            .get_apps(
                Some("User"),
                Some(vec![build.actual_bundle_identifier.clone()]),
            )
            .await
            .map_err(|_| "INSTALL_FAILED: Cannot query the installed app")?;
        let actual = apps
            .get(&build.actual_bundle_identifier)
            .and_then(|v| v.as_dictionary())
            .and_then(|v| v.get("CFBundleIdentifier"))
            .and_then(|v| v.as_string());
        if actual != Some(build.actual_bundle_identifier.as_str()) {
            return Err("INSTALL_FAILED: Installed bundle identity differs".into());
        }
        Ok(build.actual_bundle_identifier.clone())
    })
    .await
    .map_err(|_| {
        "INSTALL_FAILED: Installation timed out; inspect the iPhone before retrying".to_string()
    })?
}
