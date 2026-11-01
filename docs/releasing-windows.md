# Releasing Snapline for Windows

Releases are built by the **windows-release** workflow (`.github/workflows/windows-release.yml`) from a
`windows-v<version>` tag. It runs the same checks as CI, builds the installer, the portable zip and
`SHA256SUMS.txt` with `windows-linux/installer/package.ps1`, and creates a **draft** release. Nothing is public until
you click Publish.

## Cut a release

1. Set the version in `windows-linux/crates/snapline/Cargo.toml` (for example `0.2.0`), commit, and merge to `main`.
2. Pull `main` (`git switch main && git pull`), then tag the merged commit and push the tag. The tag must match the
   version, or the workflow stops before building.

   ```bash
   git tag -s windows-v0.2.0 -m "Snapline for Windows 0.2.0"
   git push origin windows-v0.2.0
   ```

3. Wait for the **windows-release** run to finish, then open the draft under **Releases**. Check that it has
   `Snapline-Setup-<version>.exe`, `Snapline-<version>-windows-x64.zip` and `SHA256SUMS.txt`, and that the notes read
   right. Install it once on a Windows PC if the release changes the installer.
4. Click **Publish release**.
5. After the first release only: the website says "Windows and Linux: coming soon" until then. Bring back the download
   buttons and the "Install it on Windows" steps (`git show 596fee4:site/index.html` has them), merge that into `main`,
   then publish the website.

   ```bash
   git push origin main:website
   ```

If the run fails, or you need to build the same version again: delete the draft release if one was made, then delete
the tag (`git push --delete origin windows-v0.2.0` and `git tag -d windows-v0.2.0`). Fix the problem on `main`, then
tag again. `gh release create` stops if a release for that tag already exists.

## Turn on code signing later

Until signing is set up, releases are unsigned and Windows SmartScreen asks before the first run (the release notes
say so). The workflow signs `snapline.exe` and the installer as soon as these exist:

1. In Azure, create an **Artifact Signing** account and a certificate profile (identity validation is part of this).
2. In the repository's **Settings → Environments**, open **release** (the first signed release run creates it) and limit
   it to tags matching `windows-v*`.
3. Create an app registration with a federated credential for **GitHub Actions**, entity type **Environment**, name
   `release`; its subject is `repo:barayuda/Snapline:environment:release`. Azure matches the subject exactly, so a
   tag pattern would never match. Give the app the signing role on the account (its name ends in
   **Certificate Profile Signer**).
4. In the repository's **Settings → Secrets and variables → Actions**:
   - variables: `AZURE_SIGNING_ENDPOINT` (for example `https://wus2.codesigning.azure.net/`),
     `AZURE_SIGNING_ACCOUNT`, `AZURE_SIGNING_PROFILE`;
   - secrets: `AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_SUBSCRIPTION_ID`.

The next tag is signed. No client secret is stored: the workflow signs in with OIDC. Signing runs in its own job,
which builds nothing with cargo, so no dependency's build script runs while the token can be requested. Every step
in that job can still request it (checkout, the artifact download, Inno Setup), so keep that job to those steps.

## What the installer does

- Installs for the current user in `%LOCALAPPDATA%\Programs\Snapline`, without admin rights, with a Start menu
  shortcut and an uninstaller (Settings → Apps).
- "Launch Snapline when I sign in to Windows" writes the same sign-in entry as Snapline's General tab, and clears
  "Disabled" from Task Manager for it.
- An upgrade closes a running Snapline first and keeps your earlier choices.
- Uninstalling removes the program, the shortcut and the sign-in entry. Your settings
  (`%APPDATA%\Snapline\settings.toml`) and the keys in Windows Credential Manager stay, so a reinstall picks them up.
