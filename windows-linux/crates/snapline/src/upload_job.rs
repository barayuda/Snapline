//! One upload: from the saved settings and secrets to a result. Task 8 adds `run`.

use crate::secrets::{Secret, SecretStore};
use crate::settings::{Settings, UploadTarget};
use snapline_upload::google::GoogleClient;
use snapline_upload::s3::S3Config;

pub enum Destination {
    S3(S3Config),
    Drive {
        client: GoogleClient,
        refresh_token: Option<String>,
        folder_id: Option<String>,
    },
}

/// Where the next upload goes, from settings read just now and the stored secrets.
pub fn destination(settings: &Settings, secrets: &dyn SecretStore) -> Destination {
    match settings.upload_target {
        UploadTarget::S3 => Destination::S3(S3Config {
            endpoint: settings.s3_endpoint.clone(),
            region: settings.s3_region.clone(),
            bucket: settings.s3_bucket.clone(),
            prefix: settings.s3_prefix.clone(),
            public_base_url: settings.s3_public_url.clone(),
            access_key: secrets.get(Secret::S3AccessKey).unwrap_or_default(),
            secret_key: secrets.get(Secret::S3SecretKey).unwrap_or_default(),
        }),
        UploadTarget::Drive => Destination::Drive {
            client: GoogleClient {
                id: settings.google_client_id.clone(),
                secret: secrets.get(Secret::GoogleClientSecret).unwrap_or_default(),
            },
            refresh_token: secrets.get(Secret::GoogleRefreshToken),
            folder_id: Some(settings.drive_folder_id.clone()).filter(|id| !id.is_empty()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::{MemoryStore, Secret, SecretStore};
    use crate::settings::UploadTarget;

    // Review Focus 5: every upload reads the file again, so a change in the settings window applies at once.
    #[test]
    fn upload_target_follows_the_file() {
        let store = MemoryStore::default();
        store.put(Secret::S3AccessKey, "AK").unwrap();
        store.put(Secret::S3SecretKey, "SK").unwrap();
        store.put(Secret::GoogleClientSecret, "CS").unwrap();
        let dir = std::env::temp_dir().join(format!("snapline-dest-{}", std::process::id()));
        let path = dir.join("settings.toml");
        Settings::update(&path, |s| {
            s.s3_bucket = "shots".into();
            s.s3_endpoint = "https://acct.r2.cloudflarestorage.com".into();
        })
        .unwrap();
        let Destination::S3(config) = destination(&Settings::load(&path).0, &store) else {
            panic!("expected S3")
        };
        assert_eq!(
            (
                config.bucket.as_str(),
                config.access_key.as_str(),
                config.secret_key.as_str()
            ),
            ("shots", "AK", "SK")
        );

        Settings::update(&path, |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
        })
        .unwrap();
        let Destination::Drive {
            client,
            refresh_token,
            folder_id,
        } = destination(&Settings::load(&path).0, &store)
        else {
            panic!("expected Drive")
        };
        assert_eq!((client.id.as_str(), client.secret.as_str()), ("ID", "CS"));
        assert_eq!((refresh_token, folder_id), (None, None));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
