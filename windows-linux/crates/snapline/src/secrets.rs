//! Secrets in the OS credential store (Credential Manager on Windows, the Secret Service keyring on Linux, the
//! Keychain on macOS), never in
//! `settings.toml`. Same service and account names as the macOS app.

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
        #[cfg(target_os = "linux")]
        let store = zbus_secret_service_keyring_store::Store::new()
            .map_err(|e| crate::platform::linux_keyring_unavailable(&e.to_string()))?;
        #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
        let store = keyring_core::mock::Store::new().map_err(|e| e.to_string())?;
        keyring_core::set_default_store(store);
        // Ask once, so a missing or locked-out keyring shows now and not at the first upload.
        #[cfg(target_os = "linux")]
        match Self::entry(Secret::S3AccessKey)?.get_password() {
            Ok(_) | Err(keyring_core::Error::NoEntry) => {}
            Err(e) => return Err(crate::platform::linux_keyring_unavailable(&e.to_string())),
        }
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
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<Secret, String>>);

#[cfg(test)]
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

    /// A real round trip through the Secret Service. CI unlocks a throwaway GNOME Keyring for it.
    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "needs a session bus with an unlocked keyring: CI runs it under dbus-run-session"]
    fn linux_secrets_round_trip_through_the_keyring() {
        let store = Keyring::open().unwrap();
        store.put(Secret::S3AccessKey, "AKIA-test").unwrap();
        assert_eq!(store.get(Secret::S3AccessKey).as_deref(), Some("AKIA-test"));
        store.remove(Secret::S3AccessKey).unwrap();
        assert_eq!(store.get(Secret::S3AccessKey), None);
    }

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
