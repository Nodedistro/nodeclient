use anyhow::{bail, Result};
pub async fn verify(client: &reqwest::Client, token: &str) -> Result<()> {
    let response = client
        .get("https://api.minecraftservices.com/entitlements/mcstore")
        .bearer_auth(token)
        .send()
        .await?;
    let value: serde_json::Value = crate::checked(response, "Minecraft entitlement check")
        .await?
        .json()
        .await?;
    if !value["items"].as_array().is_some_and(|items| {
        items.iter().any(|v| {
            matches!(
                v["name"].as_str(),
                Some("game_minecraft") | Some("product_minecraft")
            )
        })
    }) {
        bail!("Minecraft Java Edition was not found on this Microsoft account.");
    }
    Ok(())
}
