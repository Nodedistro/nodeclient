use anyhow::Result;
use serde::Deserialize;
#[derive(Deserialize)]
pub struct MinecraftToken {
    pub access_token: String,
    pub expires_in: u64,
}
pub async fn authenticate(
    client: &reqwest::Client,
    xsts: &crate::xbox::XboxToken,
) -> Result<MinecraftToken> {
    let response=client.post("https://api.minecraftservices.com/authentication/login_with_xbox").json(&serde_json::json!({"identityToken":format!("XBL3.0 x={};{}",xsts.user_hash()?,xsts.token)})).send().await?;
    Ok(
        crate::checked(response, "Minecraft Services authentication")
            .await?
            .json()
            .await?,
    )
}
