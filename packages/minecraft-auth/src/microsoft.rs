use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::Instant;
use subtle::ConstantTimeEq;

pub const DEFAULT_REDIRECT: &str = "https://login.microsoftonline.com/common/oauth2/nativeclient";
const TOKEN: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const SCOPE: &str = "XboxLive.signin offline_access";
#[derive(Clone)]
pub struct Config {
    pub client_id: String,
    pub redirect_uri: String,
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if self.client_id.trim().is_empty() {
            bail!("MICROSOFT_CLIENT_ID is missing. Add it to the repository .env and restart NodeClient.");
        }
        let u = reqwest::Url::parse(&self.redirect_uri)?;
        if self.redirect_uri != DEFAULT_REDIRECT
            && !(u.scheme() == "http"
                && ["127.0.0.1", "localhost"].contains(&u.host_str().unwrap_or(""))
                && u.port().is_some()
                && u.query().is_none()
                && u.fragment().is_none()
                && u.username().is_empty()
                && u.password().is_none())
        {
            bail!("Use the Microsoft nativeclient redirect or an explicit localhost/127.0.0.1 HTTP port registered as a desktop redirect.");
        }
        Ok(())
    }
}
pub struct Pending {
    pub verifier: String,
    pub state: String,
    pub config: Config,
    pub created: Instant,
}
pub fn random_secret() -> String {
    let mut b = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}
pub fn start(config: Config) -> Result<(Pending, String)> {
    config.validate()?;
    let pending = Pending {
        verifier: random_secret(),
        state: random_secret(),
        config,
        created: Instant::now(),
    };
    let mut url =
        reqwest::Url::parse("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize")?;
    url.query_pairs_mut().extend_pairs([
        ("client_id", pending.config.client_id.as_str()),
        ("response_type", "code"),
        ("redirect_uri", pending.config.redirect_uri.as_str()),
        ("scope", SCOPE),
        ("response_mode", "query"),
        ("state", &pending.state),
        ("code_challenge", &challenge(&pending.verifier)),
        ("code_challenge_method", "S256"),
        ("prompt", "select_account"),
    ]);
    Ok((pending, url.into()))
}
pub fn validate_response(pending: &Pending, callback: &str) -> Result<String> {
    if pending.created.elapsed().as_secs() > 600 {
        bail!("Microsoft sign-in expired. Please sign in again.");
    }
    let url =
        reqwest::Url::parse(callback).context("Invalid Microsoft redirect URL.")?;
    let expected = reqwest::Url::parse(&pending.config.redirect_uri)?;
    if url.origin() != expected.origin()
        || url.path() != expected.path()
        || url.fragment().is_some()
    {
        bail!("Unexpected OAuth redirect.");
    }
    let pairs: Vec<_> = url.query_pairs().collect();
    let one = |key: &str| -> Result<Option<String>> {
        let values: Vec<_> = pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.to_string())
            .collect();
        if values.len() > 1 {
            bail!("Duplicate OAuth response parameter.");
        }
        Ok(values.into_iter().next())
    };
    let state = one("state")?.context("OAuth state missing.")?;
    if !bool::from(pending.state.as_bytes().ct_eq(state.as_bytes())) {
        bail!("OAuth state mismatch. Start sign-in again.");
    }
    if let Some(error) = one("error")? {
        if error == "access_denied" {
            bail!("Microsoft authentication was cancelled.");
        }
        bail!("Microsoft authentication failed. Check the application registration.");
    }
    one("code")?
        .filter(|s| !s.is_empty())
        .context("Authorization code missing.")
}
#[derive(Serialize, Deserialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct Tokens {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub expires_in: u64,
}
pub async fn exchange(
    client: &reqwest::Client,
    pending: Pending,
    callback: &str,
) -> Result<Tokens> {
    let code = validate_response(&pending, callback)?;
    let r = client
        .post(TOKEN)
        .form(&[
            ("client_id", pending.config.client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &pending.config.redirect_uri),
            ("code_verifier", &pending.verifier),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    Ok(crate::checked(r, "Microsoft OAuth").await?.json().await?)
}
pub async fn refresh(
    client: &reqwest::Client,
    config: &Config,
    refresh_token: &str,
) -> Result<Tokens> {
    let r = client
        .post(TOKEN)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", SCOPE),
        ])
        .send()
        .await?;
    Ok(crate::checked(r, "Microsoft token refresh")
        .await?
        .json()
        .await?)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pkce_rfc7636() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(random_secret().len(), 43);
    }
    #[test]
    fn state_and_redirect() {
        let (p, _) = start(Config {
            client_id: "test".into(),
            redirect_uri: DEFAULT_REDIRECT.into(),
        })
        .unwrap();
        assert!(
            validate_response(&p, &format!("{DEFAULT_REDIRECT}?code=a&state={}", p.state)).is_ok()
        );
        for s in [
            format!("{DEFAULT_REDIRECT}?code=a&state=wrong"),
            format!("https://evil.test/?code=a&state={}", p.state),
            format!(
                "{DEFAULT_REDIRECT}?code=a&state={}&state={}",
                p.state, p.state
            ),
        ] {
            assert!(validate_response(&p, &s).is_err());
        }
    }
}
