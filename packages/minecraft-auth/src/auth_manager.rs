use crate::{
    microsoft::{self, Config, Pending, Tokens},
    token_store::{OsTokenStore, TokenStore},
};
use anyhow::{bail, Result};
use nodeclient_types::Profile;
pub struct Session {
    pub profile: Profile,
    pub access_token: String,
}
async fn complete(client: &reqwest::Client, tokens: &Tokens) -> Result<Session> {
    let xbox = crate::xbox::authenticate(client, &tokens.access_token).await?;
    let xsts = crate::xsts::authenticate(client, &xbox).await?;
    let minecraft = crate::minecraft::authenticate(client, &xsts).await?;
    crate::entitlements::verify(client, &minecraft.access_token).await?;
    let profile = crate::profile::retrieve(client, &minecraft.access_token).await?;
    Ok(Session {
        profile,
        access_token: minecraft.access_token,
    })
}
pub async fn finish(pending: Pending, callback: &str) -> Result<Session> {
    let client = nodeclient_downloader::client()?;
    let tokens = microsoft::exchange(&client, pending, callback).await?;
    let session = complete(&client, &tokens).await?;
    if tokens.refresh_token.is_empty() {
        bail!(
            "Microsoft did not issue a refresh token. Sign in again and consent to offline access."
        );
    }
    OsTokenStore.save(&session.profile.id, &tokens.refresh_token)?;
    Ok(session)
}
pub async fn restore(config: &Config, id: &str) -> Result<Session> {
    let client = nodeclient_downloader::client()?;
    let refresh = zeroize::Zeroizing::new(OsTokenStore.load(id)?);
    let tokens = microsoft::refresh(&client, config, &refresh).await?;
    // Persist rotation immediately so a later Minecraft outage does not lose the new refresh token.
    if !tokens.refresh_token.is_empty() {
        OsTokenStore.save(id, &tokens.refresh_token)?;
    }
    let session = complete(&client, &tokens).await?;
    if session.profile.id != id {
        bail!("Authenticated Minecraft profile does not match the selected account.");
    }
    Ok(session)
}
