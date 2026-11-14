# Releasing Snapline for Linux

1. Pull `main` (with the Linux work merged) and check that CI is green on both Linux runners.
2. Set the version in `windows-linux/crates/snapline/Cargo.toml` and add a `<release version="…" date="…"/>` line at
   the top of `<releases>` in `windows-linux/linux/dev.barayuda.Snapline.metainfo.xml`. Commit.
3. Tag that commit `linux-v<version>` (for example `linux-v0.1.0`) and push the tag. The **linux-release** workflow
   checks the version, runs the tests, builds the .deb, the AppImage and the Flatpak for x86_64 and aarch64, and
   creates a **draft** release with `SHA256SUMS.txt`.
4. Download one package per kind, try it in the VM (`windows-linux/linux/dev-vm.sh desktop`), then publish the draft.
5. Update the website: in `site/index.html`, replace the old Linux version everywhere in the "Install it on Linux"
   section (`linux-v…` link, .deb file name) and in the hero's "Latest versions" line, add the version to the top of
   `site/changelog/index.html` (newest first: what changed for users and the date), merge that into `main`, then publish
   the website with `git push origin main:website`.

To redo a release: delete the draft and the tag (`git push --delete origin linux-v<version>`), fix, and tag again.

The Windows release (`windows-v…` tags) is separate: see [releasing-windows.md](releasing-windows.md).
