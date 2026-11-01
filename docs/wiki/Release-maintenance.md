# Release maintenance

A release is a verified source commit plus platform artifacts and documented limits. A successful packaging job alone does not establish live-provider entitlement, installer trust, OS credential-store behaviour, or a working updater channel.

## Version boundary

The documented published baseline is v0.1.3, which includes saved authentication profiles and desktop refinements. Consult [Releases](https://github.com/ChristianRelf/Magpie/releases) and [STATUS.md](https://github.com/ChristianRelf/Magpie/blob/main/docs/STATUS.md) before asserting which binary includes which feature.

Keep workspace/package/Tauri versions consistent when making an intentional release change. Review the current release guide and manifests rather than inferring all version locations from one file.

## Prepare a candidate

1. Select the exact source commit and ensure the release worktree contains the intended changes.
2. Run appropriate workspace, SDK/UI, lifecycle, and browser checks.
3. Review dependency audits and unresolved findings.
4. Build native packages for each supported platform/architecture.
5. Exercise install/upgrade/credential-store and provider workflows that require actual user environments.
6. Record the source commit, test evidence, artifact hashes, and known limitations.

See [Development and testing](Development-and-testing.md) for commands. Do not transfer a historical successful test count to a new commit. Fixtures validate protocol paths, not all account-specific flows.

## Installer workflow

`.github/workflows/installers.yml` runs on manual dispatch and `v*` tags. Its current matrix builds:

| Runner family       | Target      | Bundles       |
| ------------------- | ----------- | ------------- |
| Ubuntu 24.04        | Linux x64   | deb, AppImage |
| Windows             | Windows x64 | NSIS          |
| macOS Apple Silicon | macOS ARM64 | app, DMG      |
| macOS Intel         | macOS x64   | app, DMG      |

The workflow builds/tests native code, runs lifecycle checks, includes the Claude helper fixtures, and runs the Linux native-window check where configured. It uploads artifacts; it does **not** itself publish a GitHub release. Current artifact retention is 14 days.

Use workflow dispatch for a review build. Tag only the commit intended for the release, since `v*` tags trigger packaging. A locally built Linux package inherits its host library requirements; build/test against the supported distribution baseline.

## Signing and updater configuration

The documented v0.1.3 artifacts are unsigned/unnotarised. Production platform signing and automatic updates remain separate release tasks. Do not label a build signed because its checksum matches or because the application contains an updater library.

The updater requires both build-time values:

| Variable                   | Purpose                                     |
| -------------------------- | ------------------------------------------- |
| `MAGPIE_UPDATE_PUBLIC_KEY` | Public key used to verify updater artifacts |
| `MAGPIE_UPDATE_URL`        | HTTPS update manifest endpoint              |

Tauri signing can generate an updater key pair:

```bash
pnpm --filter @magpie/desktop tauri signer generate
```

Keep private keys/passwords in release secrets. Signed updater builds use `TAURI_SIGNING_PRIVATE_KEY` and optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, with updater artifact creation enabled in the build configuration. Follow the [Tauri updater documentation](https://v2.tauri.app/plugin/updater/) and [distribution documentation](https://v2.tauri.app/distribute/) for current formats and platform signing.

Do not publish an invented manifest/channel or copy a private key into a repository. Verify an actual old-to-new update on every supported platform before claiming the channel works. The application stops the harness before explicit update installation.

## Verification matrix

| Area               | Evidence needed                                                                |
| ------------------ | ------------------------------------------------------------------------------ |
| Source correctness | Relevant tests, full checks where affected, version/source identification      |
| Browser UI         | Real API integration with labelled local fixtures                              |
| Native UI          | Actual Tauri/WebKit window, tray/no-tray and close policies                    |
| Process lifecycle  | Start/stop/restart, duplicate protection, persisted settings                   |
| Credential storage | Actual target OS store write/read/delete and failure handling                  |
| Providers          | Explicitly authorised login/execution/limit checks with suitable test accounts |
| Packaging          | Fresh installation, upgrade, uninstall, startup registration                   |
| Delivery           | Downloaded artifact hashes and any platform/updater signatures                 |
| Accessibility      | Keyboard/focus, labels, reduced motion, themes, supported window size          |

An automated fixture does not satisfy a separate real-credential-manager or account-owner verification item. State which checks ran, which failed, and which remain unverified.

## Publish artifacts and notes

Produce `SHA256SUMS` and a release manifest identifying the tested source/artifacts. Attach the intended unchanged installers, then verify downloaded hashes against the tested files. Include platform/architecture, installation/update instructions, feature/version boundaries, signing status, updater status, and known limitations in release notes.

Keep release documentation grounded in final shipped behaviour. A branch containing a feature does not make it available in an older attached binary. Do not edit historical evidence to imply that a later fix was part of an earlier release.

## Update the documentation

Update README/wiki version notes and any affected setup/troubleshooting pages. Record evidence in the implementation record. Run `python3 scripts/wiki.py check`, inspect staged wiki output, then publish through [Wiki maintenance](Wiki-maintenance.md).

For rollback, retain a known-compatible binary and pre-migration backup. Database downgrades are not automatically safe. Follow [Settings and storage](Settings-and-storage.md#backup-and-restore).

Canonical repository guide: [RELEASE.md](https://github.com/ChristianRelf/Magpie/blob/main/docs/RELEASE.md). CI sources: [checks](https://github.com/ChristianRelf/Magpie/blob/main/.github/workflows/ci.yml), [installers](https://github.com/ChristianRelf/Magpie/blob/main/.github/workflows/installers.yml).
