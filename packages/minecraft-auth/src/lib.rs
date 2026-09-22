pub mod auth_manager;
pub mod entitlements;
pub mod microsoft;
pub mod minecraft;
pub mod profile;
pub mod token_store;
pub mod xbox;
pub mod xsts;

pub async fn checked(
    response: reqwest::Response,
    stage: &str,
) -> anyhow::Result<reqwest::Response> {
    let status = response.status();
    if !status.is_success() {
        let hint=match status.as_u16() {
            401=>"Sign-in expired. Please sign in again.",
            403=>"Access denied. The application registration may still require Minecraft/Mojang approval.",
            429=>"Too many requests. Please wait before trying again.",
            _=>"Check your connection and Microsoft application registration."
        };
        anyhow::bail!("{stage} returned HTTP {}. {hint}", status.as_u16());
    }
    Ok(response)
}
