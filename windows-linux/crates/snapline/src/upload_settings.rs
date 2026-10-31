//! The upload settings window (the Upload tab of Preferences, brought forward to Phase 3). It runs as a second
//! process, `snapline --upload-settings`, and shares `settings.toml` and Credential Manager with the tray app.

use crate::secrets::{self, Secret, SecretStore};
use crate::settings::{Settings, UploadTarget};
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct UploadForm {
    pub target: UploadTarget,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub prefix: String,
    pub public_url: String,
    pub access_key: String,
    pub secret_key: String,
    pub client_id: String,
    pub client_secret: String,
    pub email: String,
    pub signed_in: bool,
    /// The result of the last Save: text, and whether everything is in order.
    pub status: Option<(String, bool)>,
}

impl UploadForm {
    #[must_use]
    pub fn load(settings: &Settings, secrets: &dyn SecretStore) -> Self {
        Self {
            target: settings.upload_target,
            endpoint: settings.s3_endpoint.clone(),
            region: settings.s3_region.clone(),
            bucket: settings.s3_bucket.clone(),
            prefix: settings.s3_prefix.clone(),
            public_url: settings.s3_public_url.clone(),
            access_key: secrets.get(Secret::S3AccessKey).unwrap_or_default(),
            secret_key: secrets.get(Secret::S3SecretKey).unwrap_or_default(),
            client_id: settings.google_client_id.clone(),
            client_secret: secrets.get(Secret::GoogleClientSecret).unwrap_or_default(),
            email: settings.google_email.clone(),
            signed_in: secrets.get(Secret::GoogleRefreshToken).is_some(),
            status: None,
        }
    }

    /// Saves the plain fields to `settings.toml` and the secrets to the store, and sets `status`.
    pub fn save(&mut self, path: &Path, store: &dyn SecretStore) {
        let form = self.clone();
        let written = Settings::update(path, |s| {
            s.upload_target = form.target;
            form.endpoint.trim().clone_into(&mut s.s3_endpoint);
            form.region.trim().clone_into(&mut s.s3_region);
            form.bucket.trim().clone_into(&mut s.s3_bucket);
            form.prefix.trim().clone_into(&mut s.s3_prefix);
            form.public_url.trim().clone_into(&mut s.s3_public_url);
            form.client_id.trim().clone_into(&mut s.google_client_id);
        });
        if let Err(e) = written {
            self.status = Some((e, false));
            return;
        }
        let (stored, fields) = match self.target {
            UploadTarget::S3 => (
                secrets::save(store, Secret::S3AccessKey, &self.access_key)
                    .and_then(|()| secrets::save(store, Secret::S3SecretKey, &self.secret_key)),
                [
                    ("access key ID", &self.access_key),
                    ("secret access key", &self.secret_key),
                ],
            ),
            UploadTarget::Drive => (
                secrets::save(store, Secret::GoogleClientSecret, &self.client_secret),
                [
                    ("Client ID", &self.client_id),
                    ("Client secret", &self.client_secret),
                ],
            ),
        };
        let status = if let Err(e) = stored {
            (format!("Could not save to Credential Manager: {e}"), false)
        } else {
            let empty: Vec<&str> = fields
                .iter()
                .filter(|(_, v)| v.trim().is_empty())
                .map(|(n, _)| *n)
                .collect();
            if empty.is_empty() {
                ("Saved".into(), true)
            } else {
                (format!("Saved. Still empty: {}.", empty.join(", ")), false)
            }
        };
        self.status = Some(status);
    }
}

use crate::secrets::Keyring;
use snapline_upload::google::{self, GoogleClient};
use snapline_upload::http::UreqTransport;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

const CONSOLE: &str = "https://console.cloud.google.com/auth/clients";
const GUIDE: &str =
    "https://github.com/barayuda/Snapline#google-drive-one-time-setup-about-10-minutes";
const GREEN: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(0x34, 0xC7, 0x59);
const ORANGE: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(0xFF, 0x95, 0x00);
const RED: eframe::egui::Color32 = eframe::egui::Color32::from_rgb(0xFF, 0x3B, 0x30);

struct SettingsApp {
    form: UploadForm,
    path: std::path::PathBuf,
    store: Keyring,
    /// A Google sign-in or sign-out running on another thread; its result is the new email, or an error.
    pending: Option<Receiver<Result<Option<String>, String>>>,
    google_error: Option<String>,
}

/// Runs the window until it is closed. One window at a time.
pub fn run_window() {
    if !crate::platform::claim("Local\\dev.barayuda.snapline.upload-settings") {
        return; // ponytail: the open window is not brought to the front
    }
    let (Some(path), Ok(store)) = (Settings::path(), Keyring::open()) else {
        return crate::platform::show_error(
            "Snapline cannot reach its settings or Credential Manager.",
        );
    };
    let (settings, problem) = Settings::load(&path);
    let mut form = UploadForm::load(&settings, &store);
    form.status = problem.map(|p| {
        (
            format!("This file could not be read, so Save is off until it is fixed:\n{p}"),
            false,
        )
    });
    let icon = eframe::egui::IconData {
        rgba: crate::icon::tray_rgba(64, crate::icon::TrayState::Normal),
        width: 64,
        height: 64,
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Snapline upload settings")
            .with_inner_size([480.0, 620.0])
            .with_icon(std::sync::Arc::new(icon)),
        ..Default::default()
    };
    let app = SettingsApp {
        form,
        path,
        store,
        pending: None,
        google_error: None,
    };
    if let Err(e) = eframe::run_native(
        "Snapline upload settings",
        options,
        Box::new(|_| Ok(Box::new(app))),
    ) {
        crate::platform::show_error(&format!("Snapline could not open its upload settings: {e}"));
    }
}

impl SettingsApp {
    fn sign_in(&mut self) {
        self.form.save(&self.path, &self.store);
        let client = GoogleClient {
            id: self.form.client_id.clone(),
            secret: self.form.client_secret.clone(),
        };
        if !client.is_complete() {
            self.google_error =
                Some("Paste the Client ID and Client secret above, then click Save.".into());
            return;
        }
        let (tx, rx) = channel();
        let (path, store) = (self.path.clone(), self.store);
        std::thread::spawn(move || {
            let open = |url: &url::Url| open::that(url.as_str()).is_ok();
            let result = google::sign_in(
                &UreqTransport::new(),
                &client,
                &open,
                Duration::from_secs(300),
            )
            .map_err(|e| e.to_string())
            .and_then(|signed_in| {
                secrets::save(&store, Secret::GoogleRefreshToken, &signed_in.refresh_token)?;
                Settings::update(&path, |s| {
                    s.google_email = signed_in.email.clone().unwrap_or_default();
                    s.drive_folder_id.clear();
                })?;
                Ok(Some(signed_in.email.unwrap_or_default()))
            });
            let _ = tx.send(result);
        });
        self.pending = Some(rx);
        self.google_error = None;
    }

    fn sign_out(&mut self) {
        let (tx, rx) = channel();
        let (path, store) = (self.path.clone(), self.store);
        std::thread::spawn(move || {
            if let Some(token) = store.get(Secret::GoogleRefreshToken) {
                google::sign_out(&UreqTransport::new(), &token);
            }
            let result = store.remove(Secret::GoogleRefreshToken).and_then(|()| {
                Settings::update(&path, |s| {
                    s.google_email.clear();
                    s.drive_folder_id.clear();
                })
                .map(|_| None)
            });
            let _ = tx.send(result);
        });
        self.pending = Some(rx);
        self.google_error = None;
    }

    /// Picks up a finished sign-in or sign-out.
    fn poll(&mut self, ctx: &eframe::egui::Context) {
        let Some(rx) = &self.pending else { return };
        match rx.try_recv() {
            Ok(result) => {
                self.pending = None;
                match result {
                    Ok(email) => {
                        self.form.signed_in = email.is_some();
                        self.form.email = email.unwrap_or_default();
                        ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Focus);
                    }
                    Err(e) => self.google_error = Some(e),
                }
            }
            Err(_) => ctx.request_repaint_after(Duration::from_millis(200)),
        }
    }

    fn s3_fields(&mut self, ui: &mut eframe::egui::Ui) {
        eframe::egui::Grid::new("s3").num_columns(2).show(ui, |ui| {
            field(
                ui,
                "Endpoint",
                &mut self.form.endpoint,
                "https://<account>.r2.cloudflarestorage.com",
                false,
            );
            field(
                ui,
                "Region",
                &mut self.form.region,
                "auto, or e.g. ap-southeast-1",
                false,
            );
            field(ui, "Bucket", &mut self.form.bucket, "", false);
            field(ui, "Key prefix", &mut self.form.prefix, "snapline", false);
            field(
                ui,
                "Public URL",
                &mut self.form.public_url,
                "https://shots.example.com",
                false,
            );
        });
        ui.label(eframe::egui::RichText::new("Keys (stored in Credential Manager)").strong());
        eframe::egui::Grid::new("keys")
            .num_columns(2)
            .show(ui, |ui| {
                field(ui, "Access key ID", &mut self.form.access_key, "", false);
                field(ui, "Secret access key", &mut self.form.secret_key, "", true);
            });
    }

    fn drive_fields(&mut self, ui: &mut eframe::egui::Ui) {
        ui.label(
            eframe::egui::RichText::new("Google Cloud OAuth client (type: Desktop app)").strong(),
        );
        eframe::egui::Grid::new("google")
            .num_columns(2)
            .show(ui, |ui| {
                field(
                    ui,
                    "Client ID",
                    &mut self.form.client_id,
                    "Paste: …apps.googleusercontent.com",
                    false,
                );
                field(
                    ui,
                    "Client secret",
                    &mut self.form.client_secret,
                    "Paste: GOCSPX-…",
                    true,
                );
            });
        ui.horizontal(|ui| {
            ui.hyperlink_to("Open Google Cloud Console", CONSOLE);
            ui.hyperlink_to("Step-by-step guide", GUIDE);
        });
    }

    fn account(&mut self, ui: &mut eframe::egui::Ui) {
        ui.label(eframe::egui::RichText::new("Account").strong());
        let busy = self.pending.is_some();
        if self.form.signed_in {
            let who = if self.form.email.is_empty() {
                "Google account"
            } else {
                &self.form.email
            };
            ui.label(format!("Signed in as {who}"));
            if ui
                .add_enabled(!busy, eframe::egui::Button::new("Sign out"))
                .clicked()
            {
                self.sign_out();
            }
        } else {
            let ready = !busy && !self.form.client_id.trim().is_empty();
            if ui
                .add_enabled(ready, eframe::egui::Button::new("Sign in with Google…"))
                .clicked()
            {
                self.sign_in();
            }
        }
        if busy {
            ui.spinner();
        }
        if let Some(e) = &self.google_error {
            ui.colored_label(RED, e);
        }
        ui.small(
            "Screenshots go to a \"Snapline\" folder in My Drive and are shared as \"anyone with the link can view\". \
             Snapline can only see files it created.",
        );
    }
}

fn field(ui: &mut eframe::egui::Ui, label: &str, value: &mut String, hint: &str, secret: bool) {
    ui.label(label);
    ui.add(
        eframe::egui::TextEdit::singleline(value)
            .hint_text(hint)
            .password(secret)
            .desired_width(f32::INFINITY),
    );
    ui.end_row();
}

impl eframe::App for SettingsApp {
    fn logic(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        self.poll(ctx);
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Upload settings");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.form.target, UploadTarget::S3, "S3-compatible bucket");
                ui.selectable_value(&mut self.form.target, UploadTarget::Drive, "Google Drive");
            });
            ui.separator();
            match self.form.target {
                UploadTarget::S3 => self.s3_fields(ui),
                UploadTarget::Drive => self.drive_fields(ui),
            }
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.form.save(&self.path, &self.store);
                }
                if let Some((text, ok)) = &self.form.status {
                    ui.colored_label(if *ok { GREEN } else { ORANGE }, text);
                }
            });
            ui.separator();
            match self.form.target {
                UploadTarget::S3 => {
                    ui.small("Uploads are public to anyone with the link. Do not upload guest or customer data.");
                }
                UploadTarget::Drive => self.account(ui),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::MemoryStore;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("snapline-form-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("settings.toml")
    }

    fn s3_form() -> UploadForm {
        UploadForm {
            target: UploadTarget::S3,
            endpoint: "https://acct.r2.cloudflarestorage.com".into(),
            bucket: "shots".into(),
            public_url: "https://shots.example.com".into(),
            access_key: "AKIDEXAMPLE".into(),
            secret_key: ["wJalr", "XUtnFEMI", "K7MDENG"].concat(),
            ..UploadForm::default()
        }
    }

    // Review Focus 4.
    #[test]
    fn secrets_never_reach_the_settings_file() {
        let (path, store) = (temp_path("secrets"), MemoryStore::default());
        let mut form = s3_form();
        form.client_secret = "client-secret-value".into();
        form.save(&path, &store);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("shots")
                && !text.contains("AKIDEXAMPLE")
                && !text.contains(&form.secret_key),
            "{text}"
        );
        assert!(!text.contains("client-secret-value"));
        assert_eq!(
            store.get(Secret::S3SecretKey).as_deref(),
            Some(form.secret_key.as_str())
        );
        assert_eq!(form.status, Some(("Saved".into(), true)));
    }

    #[test]
    fn saving_lists_what_is_still_empty() {
        let (path, store) = (temp_path("empty"), MemoryStore::default());
        let mut form = UploadForm {
            secret_key: String::new(),
            ..s3_form()
        };
        form.save(&path, &store);
        assert_eq!(
            form.status,
            Some(("Saved. Still empty: secret access key.".into(), false))
        );
        let mut drive = UploadForm {
            target: UploadTarget::Drive,
            client_id: "ID".into(),
            ..UploadForm::default()
        };
        drive.save(&path, &store);
        assert_eq!(
            drive.status,
            Some(("Saved. Still empty: Client secret.".into(), false))
        );
    }

    #[test]
    fn a_broken_settings_file_is_reported_and_left_alone() {
        let (path, store) = (temp_path("broken"), MemoryStore::default());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "s3_bucket = \"x\n").unwrap();
        let mut form = s3_form();
        form.save(&path, &store);
        let (text, ok) = form.status.clone().unwrap();
        assert!(!ok && text.contains("could not be read"), "{text}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "s3_bucket = \"x\n");
    }

    #[test]
    fn the_form_shows_what_is_saved() {
        let (path, store) = (temp_path("load"), MemoryStore::default());
        s3_form().save(&path, &store);
        let form = UploadForm::load(&Settings::load(&path).0, &store);
        assert_eq!(
            (form.bucket.as_str(), form.access_key.as_str()),
            ("shots", "AKIDEXAMPLE")
        );
        assert!(!form.signed_in);
    }
}
