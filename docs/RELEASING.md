# Releasing Yolite

Yolite uses GitHub Releases as its update service. Tauri signs every updater artifact, publishes `latest.json`, and verifies the signature in the desktop app before installation. Arch/CachyOS packages use the same release AppImage but remain owned by pacman.

## One-time setup

The updater keypair was generated outside the repository:

```text
~/.config/yolite-release/updater.key
~/.config/yolite-release/updater.key.pub
```

The public key is embedded in `src-tauri/tauri.conf.json`. Back up the private key in an encrypted password manager or offline encrypted storage. Never commit or share it. Losing it prevents existing installations from trusting future releases.

Add the private key to the GitHub repository:

```bash
gh auth login
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.config/yolite-release/updater.key
```

This key is passwordless. Do not create `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`; the workflow accepts it only if a future key uses one.

For automatic AUR publication, create a dedicated SSH key, add its public half to the maintainer's AUR account, and store the private half as the repository secret `AUR_SSH_PRIVATE_KEY`:

```bash
ssh-keygen -t ed25519 -f ~/.ssh/yolite_aur -C yolite-release
gh secret set AUR_SSH_PRIVATE_KEY < ~/.ssh/yolite_aur
```

Without that secret, GitHub Releases still publish normally and the AUR job safely skips itself.

## Publish a release

1. Update the version in `src-tauri/tauri.conf.json` and `package.json`.
2. Add user-facing release notes to the release commit or edit the generated GitHub release afterward.
3. Run the checks documented in the README.
4. Commit the version bump, then create and push a matching tag:

   ```bash
   git tag -a v0.2.0 -m "Yolite 0.2.0"
   git push origin main v0.2.0
   ```

The release workflow rejects a tag that does not match the Tauri configuration. It then tests the project, builds the AppImage, creates its signature and `latest.json`, and publishes all three to GitHub Releases. If AUR credentials exist, it calculates the AppImage checksum and pushes an updated `yolite-bin` `PKGBUILD` and `.SRCINFO` to the AUR.

Do not rebuild or replace assets under an existing tag. Release a new patch version so signatures, checksums, and package metadata stay reproducible.

## Update ownership

- Direct AppImage users update through Settings > Updates.
- AUR users update through `paru -Syu`, `yay -Syu`, or another package-aware workflow.
- The AUR launcher exports `YOLITE_UPDATE_MODE=package-manager`; the app reports that ownership and disables self-update.

This split prevents the application from modifying files tracked by pacman while keeping direct downloads fully self-updating.
