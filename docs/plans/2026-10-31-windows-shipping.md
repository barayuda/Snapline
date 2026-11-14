# Phase 5b: Windows installer, releases and website — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Windows users can download a per-user Snapline installer, or a portable zip, from GitHub Releases, linked from the website. The privacy policy covers Windows. Pushing a tag builds a draft release, and Barayuda reviews and publishes it. Code signing is wired into the workflow and switches on once a signing account exists. Until then releases are unsigned, and SmartScreen still warns.

**Architecture:**
- `snapline.exe` gets its icon and version info at build time. `build.rs` makes the `.ico` from `Assets/icon-1024.png` and embeds it with `winresource`, so nothing binary is committed.
- An Inno Setup script, `windows-linux/installer/snapline.iss`, builds `Snapline-Setup-<version>.exe`. It installs for the current user without admin and can launch Snapline at sign-in. It writes the same registry value the app's General tab reads, so the two agree.
- **On every push**, CI builds the installer and smoke-tests it: silent install, check, silent uninstall, check again.
- **On a `windows-v*` tag**, a release workflow builds and tests again. It checks the tag against the crate version, signs only when signing is set up, and writes SHA-256 checksums. It then creates a **draft** release with the installer, the zip and the checksums.

**Tech Stack:** Rust 2024; `winresource` and `ico` (build dependencies, Windows target); tiny-skia (already used); Inno Setup 6, which is preinstalled on GitHub's `windows-latest` image (Task 2 checks this); GitHub Actions with `gh release`; the Azure Artifact Signing action, run only when configured; the static site in `site/`.

**Spec:** [docs/windows-linux.md](../windows-linux.md) (Phase 5b).

## Decisions made for this phase

Barayuda's choices on 31 October 2026:

| Question | Choice |
|---|---|
| Installer | **Inno Setup, per user.** It installs to `%LOCALAPPDATA%\Programs\Snapline` without admin. It adds a Start menu shortcut, an uninstaller, and an optional "Launch at sign-in". A portable zip is still offered. |
| Code signing | **Ready, not signed yet.** The release workflow has a signing step that runs only when signing is set up, so releases are unsigned for now and can be signed later without code changes. |
| Where builds go | **GitHub Releases.** A `windows-v<version>` tag builds a draft release. Barayuda checks it and clicks Publish. The website links to the latest release. |

Rules that apply here:
- Under the organisation policy, publishing is Barayuda's step. The agent never pushes a release tag, never publishes a release, and never pushes to `website` (every push there deploys the live site). The plan prepares everything and documents those steps.
- The website change is committed on this branch. It goes live only when Barayuda merges to `main` and pushes `main:website`, after the first release is published, so the download link works on day one.

Choices made in this plan:
- **Signing hook:** the step is written for Azure Artifact Signing, the cloud option that works on hosted CI without a hardware token. It is gated on the repository variable `AZURE_SIGNING_ACCOUNT`. Another provider means swapping that one step.
- **Architecture:** x64 only. Windows on Arm runs x64 apps through emulation; a native ARM64 build can come later.
- **Uninstall keeps your settings:** `%APPDATA%\Snapline\settings.toml` and the Credential Manager entries stay, as uninstalling a Mac app keeps its preferences. The uninstaller removes the program, the shortcut, and the sign-in entries.
- **Upgrades:** the installer closes a running Snapline first, using `AppMutex` on the app's single-instance mutex plus Restart Manager. An upgrade keeps the user's choice of "Launch at sign-in".
- **Versioning:** the tag's version must equal `snapline`'s `Cargo.toml` version, or the release workflow stops before building.

Left for later: buying a certificate or account and switching signing on; an ARM64 build; winget; checking for updates inside the app; Linux (Phase 6).

## Global Constraints

- Every commit chain is gated: `cargo clippy … >/dev/null 2>&1 && cargo test … && git commit …`.
- Workflow and installer changes can only run on CI. Push them and read the run before calling a task done.
- Check new crate and action versions against their current releases before use (crates.io, the action's GitHub page), and pin actions to a major version.
- Website copy: plain English, sentence case, no new tracking or third-party scripts (the privacy policy promises none).
- Git: one commit per task, GPG-signed, plain sentence-case messages, no AI co-author or "generated with" lines. Check that `git log --format='%h %G?'` shows `G` before pushing.

## Review Focus

1. **Launch at sign-in from the installer and the app.** The installer writes the value the app's General tab reads (`"<path>\snapline.exe"`) and clears Task Manager's "Disabled". The uninstaller removes both. Checked by the CI smoke test (Task 2) and the Windows check.
2. **Upgrade over a running Snapline.** The installer must close it, not fail on a locked `snapline.exe`, and must not leave two copies running.
3. **Release workflow safety.**
   - Only a `windows-v*` tag runs it, and it only creates a draft.
   - A tag/version mismatch stops it before building.
   - Without signing set up it still works, unsigned.
   - Its permissions are `contents: write` for the release job only.
4. **Privacy policy accuracy.** Every statement about Windows matches the code:
   - OCR uses Windows.Media.Ocr on the PC;
   - secrets are in Credential Manager;
   - settings are in `%APPDATA%\Snapline`;
   - temporary PNGs go to `%TEMP%\Snapline`;
   - Share hands the file to the app you pick.
5. **No secret ever reaches a log or an artifact.** Signing credentials come from secrets through OIDC, and the release notes and checksums contain nothing private.

---

## File structure

```
.github/workflows/
├── windows-linux.yml          + installer build and install/uninstall smoke test (Windows)
└── windows-release.yml        new: tag → test, version check, (sign), installer, zip, checksums, draft release
windows-linux/
├── crates/snapline/build.rs   new: .ico from Assets/icon-1024.png, version info (Windows target only)
├── crates/snapline/Cargo.toml + [build-dependencies] winresource, ico, tiny-skia
└── installer/snapline.iss     new: Inno Setup script
site/index.html                + Windows download
site/privacy/index.html        Windows covered
docs/windows-linux.md, README.md   release steps, decisions
docs/releasing-windows.md      new: how Barayuda cuts and publishes a release
```

---

### Task 1: Icon and version info in snapline.exe

**Files:**
- Create: `windows-linux/crates/snapline/build.rs`
- Modify: `windows-linux/crates/snapline/Cargo.toml`, `.github/workflows/windows-linux.yml`

**Interfaces:**
- Produces: on Windows targets, `snapline.exe` carries:
  - an icon with sizes 16, 24, 32, 48, 64 and 256, made from `Assets/icon-1024.png`;
  - `FileVersion` and `ProductVersion` set to the crate version, `ProductName` "Snapline", `FileDescription` "Snapline screenshot tool", and `LegalCopyright` "Barayuda".
- On other targets, `build.rs` does nothing.

- [ ] **Step 1: Write the failing check**

Add to the Windows job in `windows-linux.yml`, after "Build snapline.exe":

```yaml
      - name: Check the exe's version info
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $want = (cargo pkgid -p snapline) -replace '.*[#@]', ''
          $info = (Get-Item target/release/snapline.exe).VersionInfo
          "ProductVersion=$($info.ProductVersion) ProductName=$($info.ProductName) want=$want"
          if ($info.ProductVersion -ne $want -or $info.ProductName -ne 'Snapline') { exit 1 }
```

Push. Expected: CI fails on this step, because the exe has no version info yet. Read the log to confirm it failed for that reason.

- [ ] **Step 2: Implement**

Run `cargo add -p snapline --build --target 'cfg(windows)' winresource ico tiny-skia@0.12` (check the latest `winresource` and `ico` versions first).

`build.rs`:

```rust
//! On Windows: gives snapline.exe the Snapline icon and version info. The .ico is made here from
//! Assets/icon-1024.png, so no generated binary is committed.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Assets/icon-1024.png");
    println!("cargo::rerun-if-changed={}", source.display());
    #[cfg(windows)]
    windows::embed(&source);
}

#[cfg(windows)]
mod windows {
    use std::path::Path;
    use tiny_skia::{FilterQuality, Pixmap, PixmapPaint, Transform};

    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];

    pub fn embed(source: &Path) {
        let big = Pixmap::load_png(source).expect("Assets/icon-1024.png");
        let mut icons = ico::IconDir::new(ico::ResourceType::Icon);
        for size in SIZES {
            let mut small = Pixmap::new(size, size).expect("size");
            let k = size as f32 / big.width() as f32;
            let paint = PixmapPaint { quality: FilterQuality::Bicubic, ..PixmapPaint::default() };
            small.draw_pixmap(0, 0, big.as_ref(), &paint, Transform::from_scale(k, k), None);
            // tiny-skia stores premultiplied pixels; the icon wants straight alpha.
            let rgba: Vec<u8> = small.pixels().iter().flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            }).collect();
            let image = ico::IconImage::from_rgba_data(size, size, rgba);
            icons.add_entry(ico::IconDirEntry::encode(&image).expect("icon entry"));
        }
        let out = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("snapline.ico");
        icons.write(std::fs::File::create(&out).expect("snapline.ico")).expect("write snapline.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(out.to_str().expect("utf-8 path"))
            .set("ProductName", "Snapline")
            .set("FileDescription", "Snapline screenshot tool")
            .set("LegalCopyright", "Barayuda");
        res.compile().expect("embed the icon and version info");
    }
}
```

`winresource` takes the version from `CARGO_PKG_VERSION`. Clippy runs on build scripts too, so fix any pedantic findings. A panic in `build.rs` is the right failure here: a missing asset should stop the build.

- [ ] **Step 3: Push and read CI**

Expected:
- the "Check the exe's version info" step passes, with `ProductVersion=0.1.0 ProductName=Snapline`;
- the artifact's `snapline.exe` shows the Snapline icon in Explorer (Barayuda checks this in the Windows check).

- [ ] **Step 4: Commit** (already pushed, as Step 3 needs CI): "Give snapline.exe its icon and version info".

---

### Task 2: The installer, built and smoke-tested on every push

**Files:**
- Create: `windows-linux/installer/snapline.iss`
- Modify: `.github/workflows/windows-linux.yml`

**Interfaces:**
- Produces: `windows-linux/target/installer/Snapline-Setup-<version>.exe`, built by `ISCC.exe /DVersion=<version> /DSource=<path to snapline.exe> installer/snapline.iss`. It is uploaded as the `snapline-windows-installer` artifact.

The installer:
- `AppId` is a fixed GUID. Generate it once with `[guid]::NewGuid()` and never change it, because upgrades find the installed app by it.
- `PrivilegesRequired=lowest`, `DefaultDirName={localappdata}\Programs\Snapline`, `DisableProgramGroupPage=yes`, `DisableDirPage=auto`.
- `AppName=Snapline`, `AppVersion={#Version}`, `AppPublisher=Barayuda`, `AppPublisherURL=https://github.com/barayuda/Snapline`, `UninstallDisplayIcon={app}\snapline.exe`, `OutputBaseFilename=Snapline-Setup-{#Version}`.
- `ArchitecturesAllowed=x64compatible`, `ArchitecturesInstallIn64BitMode=x64compatible`.
- `AppMutex=Local\dev.barayuda.snapline`, which is the app's single-instance mutex, plus `CloseApplications=yes` and `RestartApplications=no`.
- `LicenseFile` is the repo's `LICENSE.md`, shown before install.
- `[Files]`: `snapline.exe`, from `{#Source}`.
- `[Icons]`: `{userprograms}\Snapline`.
- `[Tasks]`: `startup` "Launch Snapline when I sign in to Windows", unchecked by default.
- `[Registry]`:
  - `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `Snapline` = `"""{app}\snapline.exe"""`, with `Tasks: startup` and `Flags: uninsdeletevalue`.
  - `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`, value `Snapline`, with `ValueType: none`, `Flags: deletevalue uninsdeletevalue` and `Tasks: startup`. This clears Task Manager's "Disabled" on install and removes it on uninstall.
- `[UninstallRun]`: nothing. The settings and Credential Manager entries stay.
- `[Run]`: `{app}\snapline.exe`, with `Flags: nowait postinstall skipifsilent` and `Description: "Start Snapline"`.

The `Run` value must equal what `platform::run_value` writes: the path in double quotes. The doubled quotes in Inno produce exactly that. Review Focus 1.

- [ ] **Step 1: Write the failing smoke test**

Add to the Windows job, after the exe is built:

```yaml
      - name: Build the installer
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
          if (-not (Test-Path $iscc)) { choco install innosetup --no-progress -y; }
          $version = (cargo pkgid -p snapline) -replace '.*[#@]', ''
          & $iscc /Qp /DVersion=$version "/DSource=$PWD\target\release\snapline.exe" "/O$PWD\target\installer" installer\snapline.iss
          if ($LASTEXITCODE) { exit $LASTEXITCODE }
      - name: Install, check and uninstall
        if: runner.os == 'Windows'
        shell: pwsh
        run: |
          $setup = Get-ChildItem target\installer\Snapline-Setup-*.exe | Select-Object -First 1
          Start-Process $setup.FullName -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART','/TASKS=startup' -Wait
          $exe = "$env:LOCALAPPDATA\Programs\Snapline\snapline.exe"
          if (-not (Test-Path $exe)) { throw "not installed" }
          $run = (Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run').Snapline
          if ($run -ne "`"$exe`"") { throw "Run value is '$run'" }
          $uninstall = "$env:LOCALAPPDATA\Programs\Snapline\unins000.exe"
          Start-Process $uninstall -ArgumentList '/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART' -Wait
          Start-Sleep -Seconds 2
          if (Test-Path $exe) { throw "still installed" }
          if ((Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run').Snapline) { throw "Run value left" }
          "install, sign-in entry and uninstall: ok"
      - uses: actions/upload-artifact@v7
        if: runner.os == 'Windows'
        with:
          name: snapline-windows-installer
          path: windows-linux/target/installer/*.exe
          if-no-files-found: error
```

Commit only the workflow and push. Expected: "Build the installer" fails because `installer\snapline.iss` does not exist. Read the log for that message. The log also shows whether Inno Setup was found preinstalled or needed `choco`; record which.

- [ ] **Step 2: Write `installer/snapline.iss`** as described above. Then push.

- [ ] **Step 3: Read CI**

Expected:
- the installer builds;
- the smoke test prints `install, sign-in entry and uninstall: ok`;
- the `snapline-windows-installer` artifact exists.

If the smoke test fails, read which check failed, fix it, and push again.

- [ ] **Step 4: Commit**: "Build a per-user installer and test install and uninstall on CI".

---

### Task 3: Release workflow

**Files:**
- Create: `.github/workflows/windows-release.yml`

**Interfaces:**
- Trigger: `push: tags: ['windows-v*']` only.
- Produces a draft GitHub Release named `Snapline for Windows <version>`, with `Snapline-Setup-<version>.exe`, `Snapline-<version>-windows-x64.zip` (containing `snapline.exe`, `LICENSE.md` and `README.md`) and `SHA256SUMS.txt`.

Jobs:
1. **`build`** (`windows-latest`, `permissions: contents: read`, `id-token: write` for signing only):
   1. Check out and run `rustup toolchain install`.
   2. **Version check:** `$tag = "${{ github.ref_name }}" -replace '^windows-v',''`; compare it with `cargo pkgid -p snapline`, and `throw` on a mismatch. Review Focus 3.
   3. `cargo fmt --check`, `cargo clippy … -D warnings`, `cargo test --workspace`, `cargo build --release -p snapline`.
   4. **Sign the exe**, only when configured:

      ```yaml
      - name: Sign snapline.exe
        if: vars.AZURE_SIGNING_ACCOUNT != ''
        uses: azure/artifact-signing-action@v1   # check the current name and major version on its GitHub page
        with:
          endpoint: ${{ vars.AZURE_SIGNING_ENDPOINT }}
          signing-account-name: ${{ vars.AZURE_SIGNING_ACCOUNT }}
          certificate-profile-name: ${{ vars.AZURE_SIGNING_PROFILE }}
          files: ${{ github.workspace }}\windows-linux\target\release\snapline.exe
          timestamp-rfc3161: http://timestamp.acs.microsoft.com
          timestamp-digest: SHA256
      ```

      Sign-in uses `azure/login` with OIDC (`client-id`, `tenant-id` and `subscription-id` from secrets), placed before this step with the same `if`. No client secret is stored.
   5. Build the installer as in Task 2, then **sign the installer** with the same gated step.
   6. Zip the exe, `LICENSE.md` and `README.md`. Write `SHA256SUMS.txt` with `Get-FileHash -Algorithm SHA256` (lowercase hex, two spaces, then the file name).
   7. Upload all three as an artifact.
2. **`release`** (`ubuntu-latest`, `needs: build`, `permissions: contents: write`): download the artifact, then run `gh release create "$GITHUB_REF_NAME" --draft --verify-tag --title "Snapline for Windows $VERSION" --notes-file notes.md <files>`. `notes.md` says:
   - how to install (setup, or zip);
   - that the build is unsigned for now, so SmartScreen asks first (More info → Run anyway), whenever no signing was done;
   - the SHA-256 lines.

- [ ] **Step 1: Check it without publishing**

The agent may not push a release tag (organisation policy), so test the workflow in two ways:
1. **`workflow_dispatch` dry run:** add a `workflow_dispatch` trigger with an input `dry_run` (default `true`). On a dispatch run, skip the version check against the tag and skip the `release` job. Run it with `gh workflow run windows-release.yml --ref feat/windows-linux-rust`. That builds, zips and writes checksums as an artifact, and publishes nothing. GitHub only offers dispatch for workflows on the default branch, so if the run is refused, record a ruling and rely on 2.
2. **`actionlint`:** run it on the workflow, using the official binary in a temporary folder, or `docker run rhysd/actionlint`.

Expected: the dry run produces the three files (when dispatch is available), and actionlint is clean.

- [ ] **Step 2: Commit and push**: "Add a release workflow: a windows-v tag builds a draft release".

---

### Task 4: Website and privacy policy

**Files:**
- Modify: `site/index.html`, `site/privacy/index.html`

- [ ] **Step 1: Home page**

Next to the GitHub button in the hero, add "Download for Windows", linking to `https://github.com/barayuda/Snapline/releases/latest`. Use the existing `.button` style; add no new script.

In the page's platform or requirements area, or a short new section if there is none, say:
- "Windows 10 (version 1809) and 11, 64-bit. Unsigned for now: Windows SmartScreen asks first.";
- the Mac instructions stay as they are.

Keep the page's copy rules: plain English, sentence case.

- [ ] **Step 2: Privacy policy** (Review Focus 4). Change "Mac" to "Mac or Windows PC", or "your computer", where the text applies to both. Then:
- Rename the section "What Snapline does on your Mac" to "What Snapline does on your computer", and add a Windows bullet:
  - capture, drawing, auto-hide and Copy Text run on your PC, using Windows' built-in text recognition (Windows.Media.Ocr) and face detection;
  - settings are in `%APPDATA%\Snapline\settings.toml`;
  - secrets are in Windows Credential Manager;
  - Edit, Share and Print write a temporary copy to `%TEMP%\Snapline`, which Windows cleans up.
- In "When an image leaves your Mac" (renamed "…your computer"), add: **Share** (Windows) hands the image to the app you pick in the Windows Share window, and that app handles it under its own policy. **Email** stays, marked (Mac).
- Leave the Google and S3 sections alone; they apply to both.
- Set "Last updated" to the day of the change.

Check every Windows statement against the code (`settings.rs` `Settings::path`, `secrets.rs`, `windows_ocr.rs`, `actions.rs` `temp_png`, `share.rs`). If a statement and the code disagree, the code wins: change the text, never the code.

- [ ] **Step 3: Look at both pages**

Open `site/index.html` and `site/privacy/index.html` from the file system in the browser pane. Check the new button, the wording, and the light and dark themes. Then commit: "Offer the Windows download on the website and cover Windows in the privacy policy". Do **not** push to `website`.

---

### Task 5: Release guide, docs and the Windows check

**Files:**
- Create: `docs/releasing-windows.md`
- Modify: `docs/windows-linux.md`, `README.md`

- [ ] **Step 1: `docs/releasing-windows.md`**, the steps Barayuda runs:
  1. Bump `version` in `windows-linux/crates/snapline/Cargo.toml`, commit, and merge to `main`.
  2. `git tag -s windows-v<version> -m "Snapline for Windows <version>"`, then `git push origin windows-v<version>`.
  3. Wait for the **windows-release** run, then open the draft under Releases. Check the files, the checksums and the notes, then click **Publish**.
  4. After the first release only, publish the website (`git push origin main:website`).
  5. Signing later:
     - create an Azure Artifact Signing account and certificate profile;
     - add the repository variables `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT` and `AZURE_SIGNING_PROFILE`;
     - add the secrets `AZURE_CLIENT_ID`, `AZURE_TENANT_ID` and `AZURE_SUBSCRIPTION_ID`, plus a federated credential for this repo's tags.
     
     The next tag is signed.

- [ ] **Step 2: Docs.**
  - `docs/windows-linux.md`: add the three decisions as table rows ("Barayuda, 31 Oct 2026"), and link this plan from the Phase 5b line.
  - README "Windows (preview)": "To try it" becomes "Download **Snapline-Setup** from Releases (or the portable zip)". Keep the SmartScreen note and the Actions artifact for preview builds.

- [ ] **Step 3: Full check, commit, push**

```bash
cd windows-linux && cargo fmt --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cd ../macOS && swift test
cd .. && git add docs README.md && git commit -m "Document Windows releases" && git push
```

- [ ] **Step 4: Barayuda checks it on Windows** (pass or fail, with a screenshot for any fail)

1. Run `Snapline-Setup-<version>.exe` from the latest CI artifact. It needs no admin prompt, shows the licence, and offers "Launch Snapline when I sign in". After install, Snapline is in the Start menu with its icon.
2. In Settings → Apps → Installed apps, Snapline shows its icon, version and publisher "Barayuda".
3. Tick "Launch at sign-in" in the installer, then sign out and in: Snapline starts. The General tab shows "Launch at sign-in" ticked.
4. With Snapline running, run the installer again (an upgrade). It closes Snapline itself and finishes without "file in use", and only one Snapline runs afterwards.
5. Uninstall from Settings → Apps. Snapline is gone, including from Task Manager → Startup apps, and `%APPDATA%\Snapline\settings.toml` is still there.
6. `snapline.exe` in the portable zip shows the Snapline icon in Explorer, and Properties → Details shows version and product name.
