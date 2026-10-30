//! One upload: from the saved settings and secrets to a result.

use crate::secrets::{Secret, SecretStore};
use crate::settings::{Settings, UploadTarget};
use chrono::Local;
use snapline_core::search::SearchEngine;
use snapline_core::sigv4::SigV4Signer;
use snapline_upload::UploadResult;
use snapline_upload::drive;
use snapline_upload::error::UploadError;
use snapline_upload::google::{self, AccessToken, GoogleClient, RefreshError};
use snapline_upload::http::Transport;
use snapline_upload::s3::{self, S3Config};
use std::path::Path;

#[derive(Clone)]
pub struct Job {
    pub png: Vec<u8>,
    pub file_name: String,
    /// Open this image search once the upload is done.
    pub search: Option<SearchEngine>,
}

/// A Google access token, remembered with the sign-in it came from so a later sign-in never reuses it.
#[derive(Clone, Debug)]
pub struct CachedToken {
    /// `fingerprint` of the refresh token this access token was made from.
    pub for_refresh: String,
    pub access: AccessToken,
}

/// A one-way fingerprint of a refresh token (the token itself never leaves Credential Manager).
pub fn fingerprint(refresh_token: &str) -> String {
    SigV4Signer::sha256_hex(refresh_token.as_bytes())
}

#[allow(clippy::large_enum_variant)] // one per upload, moved once to the UI thread
pub enum Outcome {
    /// `token` is the Google access token to reuse for the next upload.
    Done {
        result: UploadResult,
        token: Option<CachedToken>,
    },
    Failed(String),
}

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

/// One upload from start to finish. Reads the settings file again (so changes apply at once), uploads,
/// and keeps what Drive hands back: the folder, or a dead sign-in to forget.
pub fn run(
    job: &Job,
    settings_path: &Path,
    secrets: &dyn SecretStore,
    t: &dyn Transport,
    token: Option<CachedToken>,
    progress: &dyn Fn(f64),
) -> Outcome {
    let (settings, problem) = Settings::load(settings_path);
    if let Some(problem) = problem {
        return Outcome::Failed(format!("Snapline could not read its settings:\n{problem}"));
    }
    match destination(&settings, secrets) {
        Destination::S3(config) => match s3::upload(t, &config, &job.png, Local::now(), progress) {
            Ok(result) => Outcome::Done {
                result,
                token: None,
            },
            Err(e) => Outcome::Failed(e.to_string()),
        },
        Destination::Drive {
            client,
            refresh_token,
            folder_id,
        } => drive_upload(
            job,
            settings_path,
            secrets,
            t,
            token,
            progress,
            &client,
            refresh_token,
            folder_id.as_deref(),
        ),
    }
}

#[allow(clippy::too_many_arguments)] // the Drive half of `run`, split out to keep each part readable
fn drive_upload(
    job: &Job,
    settings_path: &Path,
    secrets: &dyn SecretStore,
    t: &dyn Transport,
    token: Option<CachedToken>,
    progress: &dyn Fn(f64),
    client: &GoogleClient,
    refresh_token: Option<String>,
    folder_id: Option<&str>,
) -> Outcome {
    if !client.is_complete() {
        return Outcome::Failed(
            "Fill in the Google client ID and secret in Snapline's upload settings.".into(),
        );
    }
    let Some(refresh) = refresh_token else {
        return Outcome::Failed(google::NOT_SIGNED_IN.into());
    };
    let print = fingerprint(&refresh);
    let renew = || match google::refresh(t, client, &refresh) {
        Ok(access) => Ok(access),
        Err(RefreshError::Expired) => {
            let _ = secrets.remove(Secret::GoogleRefreshToken);
            let _ = Settings::update(settings_path, |s| {
                s.google_email.clear();
                s.drive_folder_id.clear();
            });
            Err(google::EXPIRED.to_owned())
        }
        Err(RefreshError::Failed(e)) => Err(e.to_string()),
    };
    // Only a token made from the refresh token saved now: signing out and in again must not reuse the old one.
    let cached = token
        .filter(|c| c.for_refresh == print && c.access.is_fresh())
        .map(|c| c.access);
    let from_cache = cached.is_some();
    let mut access = match cached.map_or_else(renew, Ok) {
        Ok(access) => access,
        Err(message) => return Outcome::Failed(message),
    };
    let upload = |access: &AccessToken| {
        drive::upload(
            t,
            &access.token,
            folder_id,
            &job.png,
            &job.file_name,
            progress,
        )
    };
    let mut result = upload(&access);
    // A cached token Google no longer accepts (revoked, or signed out elsewhere): get a new one once.
    if from_cache && matches!(result, Err(UploadError::Http { status: 401, .. })) {
        access = match renew() {
            Ok(access) => access,
            Err(message) => return Outcome::Failed(message),
        };
        result = upload(&access);
    }
    match result {
        Ok(done) => {
            if folder_id != Some(done.folder_id.as_str()) {
                let _ = Settings::update(settings_path, |s| {
                    s.drive_folder_id.clone_from(&done.folder_id);
                });
            }
            Outcome::Done {
                result: done.result,
                token: Some(CachedToken {
                    for_refresh: print,
                    access,
                }),
            }
        }
        Err(e) => Outcome::Failed(e.to_string()),
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

    use snapline_upload::http::fake::FakeTransport;

    fn job() -> Job {
        Job {
            png: b"PNG".to_vec(),
            file_name: "Shot.png".into(),
            search: None,
        }
    }

    fn settings_file(name: &str, change: impl FnOnce(&mut Settings)) -> std::path::PathBuf {
        let path = std::env::temp_dir()
            .join(format!("snapline-job-{}-{name}", std::process::id()))
            .join("settings.toml");
        let _ = std::fs::remove_file(&path);
        Settings::update(&path, change).unwrap();
        path
    }

    fn drive_store() -> MemoryStore {
        let store = MemoryStore::default();
        store.put(Secret::GoogleClientSecret, "CS").unwrap();
        store.put(Secret::GoogleRefreshToken, "R").unwrap();
        store
    }

    #[test]
    fn s3_job_returns_the_link() {
        let store = MemoryStore::default();
        store.put(Secret::S3AccessKey, "AK").unwrap();
        store.put(Secret::S3SecretKey, "SK").unwrap();
        let path = settings_file("s3", |s| {
            s.s3_endpoint = "https://acct.r2.cloudflarestorage.com".into();
            s.s3_bucket = "shots".into();
            s.s3_public_url = "https://shots.example.com".into();
        });
        let Outcome::Done { result, .. } = run(
            &job(),
            &path,
            &store,
            &FakeTransport::new(vec![(200, "")]),
            None,
            &|_| {},
        ) else {
            panic!("expected Done")
        };
        assert!(
            result
                .page
                .as_str()
                .starts_with("https://shots.example.com/snapline/")
        );
    }

    #[test]
    fn drive_job_remembers_the_folder_it_made() {
        let path = settings_file("drive", |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
        });
        let fake = FakeTransport::new(vec![
            (200, r#"{"access_token":"A","expires_in":3600}"#),
            (200, r#"{"id":"F1"}"#),
            (
                200,
                r#"{"id":"X1","webViewLink":"https://drive.google.com/file/d/X1/view"}"#,
            ),
            (200, "{}"),
        ]);
        let outcome = run(&job(), &path, &drive_store(), &fake, None, &|_| {});
        assert!(matches!(outcome, Outcome::Done { token: Some(_), .. }));
        assert_eq!(Settings::load(&path).0.drive_folder_id, "F1");
    }

    // Review Focus 2.
    #[test]
    fn expired_sign_in_clears_the_token() {
        let path = settings_file("expired", |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
            s.google_email = "a@example.com".into();
        });
        let store = drive_store();
        let fake = FakeTransport::new(vec![(400, r#"{"error":"invalid_grant"}"#)]);
        let Outcome::Failed(message) = run(&job(), &path, &store, &fake, None, &|_| {}) else {
            panic!("expected Failed")
        };
        assert_eq!(message, snapline_upload::google::EXPIRED);
        assert_eq!(store.get(Secret::GoogleRefreshToken), None);
        assert_eq!(Settings::load(&path).0.google_email, "");
    }

    #[test]
    fn drive_without_sign_in_asks_for_it_and_sends_nothing() {
        let path = settings_file("signed-out", |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
        });
        let store = MemoryStore::default();
        store.put(Secret::GoogleClientSecret, "CS").unwrap();
        let fake = FakeTransport::new(vec![]);
        let Outcome::Failed(message) = run(&job(), &path, &store, &fake, None, &|_| {}) else {
            panic!("expected Failed")
        };
        assert_eq!(message, snapline_upload::google::NOT_SIGNED_IN);
        assert_eq!(fake.requests(), []);
    }

    fn cached(for_refresh: &str, token: &str) -> CachedToken {
        let access = snapline_upload::google::AccessToken {
            token: token.into(),
            expires: std::time::Instant::now() + std::time::Duration::from_mins(50),
        };
        CachedToken {
            for_refresh: fingerprint(for_refresh),
            access,
        }
    }

    fn drive_settings(name: &str) -> std::path::PathBuf {
        settings_file(name, |s| {
            s.upload_target = UploadTarget::Drive;
            s.google_client_id = "ID".into();
        })
    }

    const UPLOADED: &str = r#"{"id":"X1","webViewLink":"https://drive.google.com/file/d/X1/view"}"#;

    fn bearer(request: &snapline_upload::http::Request) -> Option<&str> {
        request
            .headers
            .iter()
            .find(|(k, _)| k == "Authorization")
            .map(|(_, v)| v.as_str())
    }

    // Review finding: after signing in again, the old cached token was still used.
    #[test]
    fn a_cached_token_from_another_sign_in_is_not_used() {
        let fake = FakeTransport::new(vec![
            (200, r#"{"access_token":"NEW","expires_in":3600}"#),
            (200, r#"{"id":"F1"}"#),
            (200, UPLOADED),
            (200, "{}"),
        ]);
        let outcome = run(
            &job(),
            &drive_settings("other"),
            &drive_store(),
            &fake,
            Some(cached("OLD", "STALE")),
            &|_| {},
        );
        assert!(matches!(outcome, Outcome::Done { .. }));
        let sent = fake.requests();
        assert_eq!(sent[0].url.as_str(), snapline_core::google_oauth::TOKEN_URL);
        assert!(sent[1..].iter().all(|r| bearer(r) == Some("Bearer NEW")));
    }

    // Review finding: a token revoked at Google kept failing with 401 for up to an hour.
    #[test]
    fn a_rejected_cached_token_is_refreshed_once_and_retried() {
        let fake = FakeTransport::new(vec![
            (401, r#"{"error":{"message":"Invalid Credentials"}}"#),
            (200, r#"{"access_token":"NEW","expires_in":3600}"#),
            (200, r#"{"id":"F1"}"#),
            (200, UPLOADED),
            (200, "{}"),
        ]);
        let outcome = run(
            &job(),
            &drive_settings("retry"),
            &drive_store(),
            &fake,
            Some(cached("R", "OLD")),
            &|_| {},
        );
        let Outcome::Done {
            token: Some(token), ..
        } = outcome
        else {
            panic!("expected Done")
        };
        assert_eq!(token.access.token, "NEW");
        assert_eq!(fake.requests().len(), 5);
    }

    #[test]
    fn a_rejected_token_with_a_revoked_sign_in_asks_to_sign_in_again() {
        let fake = FakeTransport::new(vec![(401, "{}"), (400, r#"{"error":"invalid_grant"}"#)]);
        let store = drive_store();
        let outcome = run(
            &job(),
            &drive_settings("revoked"),
            &store,
            &fake,
            Some(cached("R", "OLD")),
            &|_| {},
        );
        let Outcome::Failed(message) = outcome else {
            panic!("expected Failed")
        };
        assert_eq!(message, snapline_upload::google::EXPIRED);
        assert_eq!(store.get(Secret::GoogleRefreshToken), None);
    }
}
