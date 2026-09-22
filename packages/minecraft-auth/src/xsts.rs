use crate::xbox::XboxToken;
use anyhow::Result;
pub async fn authenticate(client: &reqwest::Client, xbox: &XboxToken) -> Result<XboxToken> {
    let response=client.post("https://xsts.auth.xboxlive.com/xsts/authorize").json(&serde_json::json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[xbox.token]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"})).send().await?;
    if response.status() == 401 {
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        let hint=match body["XErr"].as_u64() {Some(2148916233)=>"Create an Xbox profile for this Microsoft account first.",Some(2148916238)=>"This child account requires a Microsoft family organizer to approve Xbox access.",Some(2148916235)=>"Xbox Live is unavailable in this account's region.",_=>"Check Xbox privacy settings, account age restrictions, and Microsoft application registration."};
        anyhow::bail!("XSTS authentication failed. {hint}");
    }
    Ok(crate::checked(response, "XSTS authentication")
        .await?
        .json()
        .await?)
}
