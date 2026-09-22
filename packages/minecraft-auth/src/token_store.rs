use anyhow::{Context, Result};
pub trait TokenStore {
    fn save(&self, id: &str, refresh: &str) -> Result<()>;
    fn load(&self, id: &str) -> Result<String>;
    fn remove(&self, id: &str) -> Result<()>;
}
pub struct OsTokenStore;
impl TokenStore for OsTokenStore {
    fn save(&self, id: &str, refresh: &str) -> Result<()> {
        keyring::Entry::new("NodeClient.Microsoft", id)?
            .set_password(refresh)
            .context("Cannot save credentials in OS secure storage.")
    }
    fn load(&self, id: &str) -> Result<String> {
        keyring::Entry::new("NodeClient.Microsoft", id)?
            .get_password()
            .context("Microsoft sign-in expired or credentials are missing. Please sign in again.")
    }
    fn remove(&self, id: &str) -> Result<()> {
        match keyring::Entry::new("NodeClient.Microsoft", id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
