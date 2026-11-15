# Releasing LocalAide

This page explains how a release is built, what CI checks, and the steps that need a person: signing certificates and the update key.

## How a release is built

1. Set one version in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`. The `version-gate` job fails if they differ, or if a tag does not match them.
2. Push a tag such as `v0.2.0`, or run the "CI & Release Pipeline" workflow by hand with "Create a draft GitHub Release" ticked.
3. CI builds `llama-server` for each platform, checks that it starts, then builds the installers.
4. The `publish-release` job makes a **draft** release with the installers, `SHA256SUMS.txt` and `localaide-sbom.spdx.json`. Nothing is public yet.
5. A person downloads the installers from the draft, tries them on a real machine, then presses "Publish".

To test the builds without a release, put `[build-all]` in a commit message on any branch.

## Build an installer on your own computer

CI builds every release, so this is only for trying a package locally.

You need:

- **Rust.** rustup installs the version named in `rust-toolchain.toml` by itself.
- **Node.js 24 or newer.**
- **System build tools.**
  - macOS: Xcode Command Line Tools (`xcode-select --install`).
  - Windows: Visual Studio C++ Build Tools (MSVC), and Git Bash for the shell scripts.
  - Linux: `libwebkit2gtk-4.1-dev build-essential curl wget file libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libasound2-dev`.
- **`cargo-about`**, for the license file: `cargo install cargo-about --locked`.
- **`llama-server` for your platform.** Download the `llama-server-<target>` artifact from a `[build-all]` CI run, or build llama.cpp `v1.0.0` with the flags in `.github/workflows/llama-server.yml`.

Then, from the repository root:

```bash
scripts/prepare-sidecar.sh <folder-with-llama-server> <target-triple>
(cd src-tauri && cargo about generate about.hbs > ../THIRD_PARTY_LICENSES.html)
npm ci
make release
```

The target triple is one of `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc` or `x86_64-unknown-linux-gnu`. `make release` builds with the `src-tauri/tauri.sidecar.conf.json` overlay, which bundles `llama-server` and the license file. A bare `npx tauri build` makes an app that cannot chat.

The installers land in `src-tauri/target/release/bundle/`, in `nsis/` (Windows), `dmg/` and `macos/` (macOS), or `deb/` and `appimage/` (Linux).

## What each release contains

| File | What it is |
|---|---|
| `*-setup.exe` | Windows installer (NSIS) |
| `*_aarch64.dmg`, `*_x64.dmg` | macOS disk images for Apple Silicon and Intel |
| `*.deb`, `*.AppImage` | Linux packages, built on Ubuntu 22.04 |
| `SHA256SUMS.txt` | SHA-256 of every file above. Check a download with `sha256sum -c SHA256SUMS.txt --ignore-missing` (Linux and macOS) or compare with `Get-FileHash <file>` (Windows). |
| `localaide-sbom.spdx.json` | Every Rust and npm package in the build, with versions and licenses |

Inside the app: `LICENSE`, `THIRD_PARTY_NOTICES.md` and `THIRD_PARTY_LICENSES.html` (the full license text of every Rust package, made by `cargo about`).

## CPU and runtime requirements

`llama-server` is built with `GGML_NATIVE=OFF`. On x86_64 that means a CPU with AVX2 (Intel Haswell from 2013, AMD Excavator from 2015, or newer). It is built without OpenMP, and on Windows with the static C runtime, so it needs no Visual C++ Redistributable.

The Windows installer carries the WebView2 bootstrapper. Windows 10 and 11 usually have WebView2 already. When they do not, the installer needs the internet once to fetch it.

## Signing (needs certificates)

Unsigned builds work, but Windows SmartScreen and macOS Gatekeeper warn about them. Signing turns on by itself when these repository secrets exist (Settings → Secrets and variables → Actions):

### macOS

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | A "Developer ID Application" certificate exported as `.p12`, encoded as base64 |
| `APPLE_CERTIFICATE_PASSWORD` | The `.p12` password |
| `APPLE_SIGNING_IDENTITY` | For example `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | For notarization: the Apple ID, an app-specific password, and the team id |

This needs a paid Apple Developer membership.

### Windows

Code-signing certificates now live in hardware or in a cloud signing service, so the workflow runs a signing command instead of importing a key file.

| Secret | Value |
|---|---|
| `WINDOWS_SIGN_COMMAND` | A command that signs the file `%1`. For Azure Trusted Signing: `trusted-signing-cli -e <endpoint> -a <account> -c <profile> -d LocalAide %1` |
| `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | Credentials for Azure Trusted Signing, if that is the service |

If the command needs a tool (such as `trusted-signing-cli`), add a step that installs it before "Build Tauri Package" in `.github/workflows/ci.yml`.

## Updates

LocalAide does not update itself yet. Settings → About → "Check for updates" asks GitHub for the newest release and links to its download page. Nothing is checked at startup, so the app makes no network call unless the user asks.

An in-app updater (`tauri-plugin-updater`) is planned (ADR-0005). It needs:

1. An update signing key pair: `npx tauri signer generate -w ~/.tauri/localaide.key`. Keep the private key and its password safe; without them, existing installs can never receive a signed update.
2. Repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
3. The public key in `tauri.conf.json` under `plugins.updater.pubkey`, and `bundle.createUpdaterArtifacts` set to `true`.
4. A `latest.json` manifest attached to each release, and the plugin wired so the user confirms every update.

## Bundle identifier

The identifier is `com.localaide.app`. Tauri warns that ending in `.app` clashes with macOS app bundle names. Version 0.1.0 is already public with this identifier, and changing it would make an upgrade look like a different app, so it stays. Change it only together with a migration plan for existing installs.
