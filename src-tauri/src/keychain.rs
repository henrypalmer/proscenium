//! OS keychain abstraction (Windows Credential Manager / macOS Keychain).
//!
//! Provider passwords are stored under one keychain entry per provider. The
//! SQLite `providers.password` column only ever holds the reference key
//! returned by [`store_secret`], never the secret itself.

use keyring::Entry;

const SERVICE: &str = "Proscenium";

fn account(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

fn entry(provider_id: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, &account(provider_id))
        .map_err(|e| format!("Could not access the OS keychain: {e}"))
}

/// The opaque value stored in the database's `password` column.
pub fn reference_key(provider_id: &str) -> String {
    format!("keyring:{SERVICE}/{}", account(provider_id))
}

/// Store a secret in the OS keychain; returns the reference key to persist.
pub fn store_secret(provider_id: &str, secret: &str) -> Result<String, String> {
    entry(provider_id)?
        .set_password(secret)
        .map_err(|e| format!("Failed to store credentials in the OS keychain: {e}"))?;
    Ok(reference_key(provider_id))
}

pub fn get_secret(provider_id: &str) -> Result<String, String> {
    entry(provider_id)?
        .get_password()
        .map_err(|e| format!("Failed to read credentials from the OS keychain: {e}"))
}

/// Remove a secret; missing entries are not an error.
pub fn delete_secret(provider_id: &str) -> Result<(), String> {
    match entry(provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!(
            "Failed to remove credentials from the OS keychain: {e}"
        )),
    }
}
