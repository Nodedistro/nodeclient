use anyhow::{bail, Result};
use nodeclient_types::Profile;
pub async fn retrieve(client: &reqwest::Client, token: &str) -> Result<Profile> {
    let response = client
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(token)
        .send()
        .await?;
    if response.status() == 404 {
        bail!("Minecraft profile not found. Create your Java Edition profile on minecraft.net.");
    }
    let mut profile: Profile = crate::checked(response, "Minecraft profile")
        .await?
        .json()
        .await?;
    if profile.id.len() != 32
        || !profile.id.bytes().all(|c| c.is_ascii_hexdigit())
        || profile.name.is_empty()
    {
        bail!("Minecraft Services returned an invalid profile.");
    }
    for skin in &mut profile.skins {
        if let Some(url) = skin.get("url").and_then(|u| u.as_str()) {
            if url.starts_with("http://textures.minecraft.net/") {
                skin["url"] = serde_json::Value::String(
                    url.replace("http://textures.minecraft.net/", "https://textures.minecraft.net/"),
                );
            }
        }
    }
    for cape in &mut profile.capes {
        if let Some(url) = cape.get("url").and_then(|u| u.as_str()) {
            if url.starts_with("http://textures.minecraft.net/") {
                cape["url"] = serde_json::Value::String(
                    url.replace("http://textures.minecraft.net/", "https://textures.minecraft.net/"),
                );
            }
        }
    }
    Ok(profile)
}
