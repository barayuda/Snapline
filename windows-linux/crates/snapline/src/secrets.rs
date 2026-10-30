//! Secrets in the OS credential store (Credential Manager on Windows, the Keychain on macOS), never in
//! `settings.toml`. Same service and account names as the macOS app.

use std::collections::HashMap;
use std::sync::Mutex;

const SERVICE: &str = "dev.barayuda.snapline";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(clippy::enum_variant_names)] // the names mirror the macOS Keychain accounts
pub enum Secret {
    S3AccessKey,
    S3SecretKey,
    GoogleClientSecret,
    GoogleRefreshToken,
}

impl Secret {
    #[must_use]
    pub const fn account(self) -> &'static str {
        match self {
            Self::S3AccessKey => "s3AccessKey",
            Self::S3SecretKey => "s3SecretKey",
            Self::GoogleClientSecret => "googleClientSecret",
            Self::GoogleRefreshToken => "googleRefreshToken",
        }
    }
}

pub trait SecretStore: Send + Sync {
    fn get(&self, secret: Secret) -> Option<String>;
    fn put(&self, secret: Secret, value: &str) -> Result<(), String>;
    /// Removing a secret that is not there succeeds.
    fn remove(&self, secret: Secret) -> Result<(), String>;
}

/// Saves `value` without surrounding spaces; an empty value removes the secret.
pub fn save(store: &dyn SecretStore, secret: Secret, value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        store.remove(secret)
    } else {
        store.put(secret, value)
    }
}

/// The OS credential store, chosen once with `open`.
#[derive(Clone, Copy)]
pub struct Keyring;

impl Keyring {
    /// # Errors
    /// When the OS store cannot be opened.
    pub fn open() -> Result<Self, String> {
        #[cfg(windows)]
        let store = windows_native_keyring_store::Store::new().map_err(|e| e.to_string())?;
        #[cfg(target_os = "macos")]
        let store =
            apple_native_keyring_store::keychain::Store::new().map_err(|e| e.to_string())?;
        // ponytail: Linux gets the Secret Service store in Phase 6; until then secrets last for one run.
        #[cfg(not(any(windows, target_os = "macos")))]
        let store = keyring_core::mock::Store::new().map_err(|e| e.to_string())?;
        keyring_core::set_default_store(store);
        Ok(Self)
    }

    fn entry(secret: Secret) -> Result<keyring_core::Entry, String> {
        keyring_core::Entry::new(SERVICE, secret.account()).map_err(|e| e.to_string())
    }
}

impl SecretStore for Keyring {
    fn get(&self, secret: Secret) -> Option<String> {
        Self::entry(secret).ok()?.get_password().ok()
    }

    fn put(&self, secret: Secret, value: &str) -> Result<(), String> {
        Self::entry(secret)?
            .set_password(value)
            .map_err(|e| e.to_string())
    }

    fn remove(&self, secret: Secret) -> Result<(), String> {
        match Self::entry(secret)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

/// An in-memory store for tests (keyring-core's mock store forgets values between entries).
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<Secret, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, secret: Secret) -> Option<String> {
        self.0.lock().ok()?.get(&secret).cloned()
    }

    fn put(&self, secret: Secret, value: &str) -> Result<(), String> {
        self.0
            .lock()
            .map_err(|e| e.to_string())?
            .insert(secret, value.to_owned());
        Ok(())
    }

    fn remove(&self, secret: Secret) -> Result<(), String> {
        self.0.lock().map_err(|e| e.to_string())?.remove(&secret);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_match_the_macos_keychain_names() {
        let names: Vec<_> = [
            Secret::S3AccessKey,
            Secret::S3SecretKey,
            Secret::GoogleClientSecret,
            Secret::GoogleRefreshToken,
        ]
        .map(Secret::account)
        .into();
        assert_eq!(
            names,
            [
                "s3AccessKey",
                "s3SecretKey",
                "googleClientSecret",
                "googleRefreshToken"
            ]
        );
    }

    #[test]
    fn saving_trims_and_an_empty_value_removes_the_secret() {
        let store = MemoryStore::default();
        save(&store, Secret::S3SecretKey, "  abc \n").unwrap();
        assert_eq!(store.get(Secret::S3SecretKey).as_deref(), Some("abc"));
        save(&store, Secret::S3SecretKey, "   ").unwrap();
        assert_eq!(store.get(Secret::S3SecretKey), None);
        save(&store, Secret::S3SecretKey, "").unwrap(); // removing what is not there is fine
    }
}
