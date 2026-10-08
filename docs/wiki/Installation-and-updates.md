# Installation and updates

Use the assets attached to a [Magpie release](https://github.com/ChristianRelf/Magpie/releases). The documented installer baseline is v0.1.3. Release tags, source versions, and unreleased `main` features are not interchangeable.

## Choose the correct file

| Computer                 | v0.1.3 filename               | Installation                                  |
| ------------------------ | ----------------------------- | --------------------------------------------- |
| Linux x64, Debian/Ubuntu | `Magpie_0.1.3_amd64.deb`      | Install with the distribution package manager |
| Linux x64, portable      | `Magpie_0.1.3_amd64.AppImage` | Make executable and run                       |
| Windows x64              | `Magpie_0.1.3_x64-setup.exe`  | Run the NSIS installer                        |
| macOS Apple Silicon      | `Magpie_0.1.3_aarch64.dmg`    | Open the DMG and install the app              |
| macOS Intel              | `Magpie_0.1.3_x64.dmg`        | Open the DMG and install the app              |

Select the architecture of your machine. The release workflow currently builds Linux x64, Windows x64, and both macOS architectures; it does not establish support for every Linux distribution or Windows/macOS architecture.

The desktop installer is separate from the optional standalone `magpie` CLI. See [CLI installation](CLI-reference.md#install-the-cli). Rust, Node, and pnpm are development tools, not ordinary desktop installation requirements.

## Verify the download

Download `SHA256SUMS` and compare the hash for the exact filename you obtained. In the directory containing the assets, Linux can verify the downloaded subset:

```bash
sha256sum --check --ignore-missing SHA256SUMS
```

On macOS, calculate an individual file's hash and compare its line in `SHA256SUMS`:

```bash
shasum -a 256 Magpie_0.1.3_aarch64.dmg
```

On Windows PowerShell:

```powershell
Get-FileHash .\Magpie_0.1.3_x64-setup.exe -Algorithm SHA256
```

A mismatch means the downloaded file differs from the manifest. Download it again from the release and do not treat the mismatching file as verified. `release-manifest.json` provides additional provenance. A matching checksum is not code signing or notarisation.

## Linux

For a downloaded Debian package, from its directory:

```bash
sudo apt install ./Magpie_0.1.3_amd64.deb
```

For an AppImage:

```bash
chmod +x Magpie_0.1.3_amd64.AppImage
./Magpie_0.1.3_amd64.AppImage
```

If the system cannot mount an AppImage because FUSE support is unavailable, its extraction/run fallback may help:

```bash
./Magpie_0.1.3_amd64.AppImage --appimage-extract-and-run
```

The installer workflow uses Ubuntu 24.04. Native builds inherit their build environment's library requirements; successful packaging does not guarantee older-distribution compatibility. A real desktop session and usable Secret Service matter for provider credential storage. If that store is locked or unavailable, production credential writes fail rather than falling back to plaintext.

Keep portable executables in a stable location. Startup entries and the Claude reporting helper can refer to that location. After moving the AppImage, recreate affected registrations through Settings and disable/re-enable Claude reporting.

## Windows and macOS

Install using the appropriate package and start Magpie as your ordinary user. The documented v0.1.3 installers are unsigned/unnotarised, so OS trust prompts can occur. Review the source/release provenance and your organisation's policy; do not disable system-wide security controls as a troubleshooting step.

If an OS blocks an app, record the exact message and consult that OS's approved per-app workflow. Check architecture and file hash before treating the block as a Magpie runtime failure. The app may never have started, in which case there will be no harness log.

## Update an existing installation

1. Check the release notes and whether you are using published installers or a source build.
2. Finish/cancel active work. Shutdown can cancel current requests.
3. If you need a backup, follow [Settings and storage](Settings-and-storage.md#backup-and-restore) before changing versions.
4. Quit the desktop and stop a background harness if it was left running.
5. Install the new package using your platform's normal process.
6. Open the updated app and verify both app and harness versions. Restart an older background harness if necessary.
7. Verify connections and read-only API access before testing generation.

Database migrations run when the service opens its database. Do not assume an older binary can read a database after a newer version migrates it. A rollback should use a compatible pre-upgrade backup with the corresponding binary.

The updater implementation requires a release-time public key and HTTPS manifest URL. Builds without them report that the update channel is unavailable. The **Automatically check for updates** setting cannot create a missing channel. v0.1.3 uses manual installation; maintainers can configure a signed channel using [Release maintenance](Release-maintenance.md).

## Uninstall or start fresh

Before uninstalling, disable launch at login and disable the Claude status-line bridge if enabled. Stop active work and the service. Remove the application using the OS package/app workflow. Application data and provider authentication may live separately; uninstalling an executable should not be assumed to erase them.

Use **Integrations** to revoke unwanted client keys and **Providers** to disconnect credentials deliberately. For a clean diagnostic test, use a separate `MAGPIE_HOME` and port rather than deleting your normal database. Provider CLI login state and the OS credential manager are outside an ordinary database copy.

If installation succeeds but the service fails, see [Troubleshooting](Troubleshooting.md). For developer installation, use [Development and testing](Development-and-testing.md).
