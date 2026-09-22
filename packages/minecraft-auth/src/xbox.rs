use anyhow::{Context, Result};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XboxToken {
    pub token: String,
    pub display_claims: serde_json::Value,
}
impl XboxToken {
    pub fn user_hash(&self) -> Result<String> {
        self.display_claims["xui"][0]["uhs"]
            .as_str()
            .map(str::to_owned)
            .context("Xbox Live did not return a user hash.")
    }
}
pub async fn authenticate(client: &reqwest::Client, microsoft_token: &str) -> Result<XboxToken> {
    let response=client.post("https://user.auth.xboxlive.com/user/authenticate").json(&serde_json::json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={microsoft_token}")},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"})).send().await?;
    Ok(crate::checked(response, "Xbox Live authentication")
        .await?
        .json()
        .await?)
}
