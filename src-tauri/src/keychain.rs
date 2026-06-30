//! OS keychain abstraction (Windows Credential Manager / macOS Keychain).
//!
//! Provider passwords are stored under one keychain entry per provider. The
//! SQLite `providers.password` column only ever holds the reference key
//! returned by [`store_secret`], never the secret itself.

use keyring::Entry;
use std::sync::{Mutex, MutexGuard};

const SERVICE: &str = "Proscenium";

/// keyring-rs documents the Windows credential store as not thread-safe for
/// concurrent in-process access (even to different entries); intermittent
/// "no matching entry" errors result. Serialize every keychain operation.
static KEYCHAIN_LOCK: Mutex<()> = Mutex::new(());

fn lock() -> MutexGuard<'static, ()> {
    KEYCHAIN_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn account(provider_id: &str) -> String {
    format!("provider:{provider_id}")
}

/// Milestone 41: Stremio addon manifest URLs (token-bearing → secret) live under
/// a separate `addon:` namespace, never in SQLite.
fn account_addon(addon_id: &str) -> String {
    format!("addon:{addon_id}")
}

fn entry_for(account: &str) -> Result<Entry, String> {
    Entry::new(SERVICE, account).map_err(|e| format!("Could not access the OS keychain: {e}"))
}

fn entry(provider_id: &str) -> Result<Entry, String> {
    entry_for(&account(provider_id))
}

/// The opaque value stored in the database's `password` column.
pub fn reference_key(provider_id: &str) -> String {
    format!("keyring:{SERVICE}/{}", account(provider_id))
}

/// Store a secret in the OS keychain; returns the reference key to persist.
pub fn store_secret(provider_id: &str, secret: &str) -> Result<String, String> {
    let _guard = lock();
    entry(provider_id)?
        .set_password(secret)
        .map_err(|e| format!("Failed to store credentials in the OS keychain: {e}"))?;
    Ok(reference_key(provider_id))
}

pub fn get_secret(provider_id: &str) -> Result<String, String> {
    let _guard = lock();
    entry(provider_id)?
        .get_password()
        .map_err(|e| format!("Failed to read credentials from the OS keychain: {e}"))
}

/// Remove a secret; missing entries are not an error.
pub fn delete_secret(provider_id: &str) -> Result<(), String> {
    let _guard = lock();
    match entry(provider_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!(
            "Failed to remove credentials from the OS keychain: {e}"
        )),
    }
}

// --- Stremio addon manifest URLs (Milestone 41). The URL embeds an access token
// (e.g. AIOStreams/Torbox), so it is a secret: stored in the keychain only, with
// just a reference key in SQLite, and never logged. ---

pub fn store_addon_secret(addon_id: &str, manifest_url: &str) -> Result<String, String> {
    let _guard = lock();
    entry_for(&account_addon(addon_id))?
        .set_password(manifest_url)
        .map_err(|e| format!("Failed to store the addon URL in the OS keychain: {e}"))?;
    Ok(format!("keyring:{SERVICE}/{}", account_addon(addon_id)))
}

pub fn get_addon_secret(addon_id: &str) -> Result<String, String> {
    let _guard = lock();
    entry_for(&account_addon(addon_id))?
        .get_password()
        .map_err(|e| format!("Failed to read the addon URL from the OS keychain: {e}"))
}

pub fn delete_addon_secret(addon_id: &str) -> Result<(), String> {
    let _guard = lock();
    match entry_for(&account_addon(addon_id))?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Failed to remove the addon URL from the OS keychain: {e}")),
    }
}
