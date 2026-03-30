# RSS Reader

A local-first RSS reader built with Rust and Dioxus.

This project prioritizes practical reading workflows over branding or heavy infrastructure:
- desktop app for day-to-day use
- web build for browser testing and static deployment
- local SQLite persistence
- JSON / OPML config exchange
- optional WebDAV config sync
- custom CSS theming with preset themes
- companion CLI for feed and settings automation

## What ships today

- `rssr-app`
  - Dioxus desktop app
  - Dioxus web app
- `rssr-cli`
  - add/remove feeds
  - refresh feeds
  - import/export config JSON
  - import/export OPML
  - inspect/save settings
  - push/pull WebDAV config

## Repository layout

```text
crates/
├── rssr-app/
├── rssr-cli/
├── rssr-application/
├── rssr-domain/
└── rssr-infra/

assets/
docs/
migrations/
specs/
tests/
```

## Local development

### Prerequisites

- Rust stable
- `wasm32-unknown-unknown` target for web builds
- Dioxus CLI `0.7.3`

```bash
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.3 --locked
```

### Run desktop app

```bash
cargo run -p rssr-app
```

If you are running under WSLg and see `libEGL` / `MESA` warnings, try:

```bash
GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 GSK_RENDERER=cairo WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo run -p rssr-app
```

### Run web app

```bash
dx serve --platform web --package rssr-app
```

### Build Android debug APK

Install the Android Rust targets:

```bash
rustup target add aarch64-linux-android x86_64-linux-android
```

Required local tooling:
- JDK 21
- Android SDK command line tools
- Android NDK
- Android platform tools
- Android platform 33
- Android build-tools 34.0.0

Example environment:

```bash
export JAVA_HOME="$HOME/.local/jdks/temurin-21"
export ANDROID_SDK_ROOT="$HOME/.local/android-sdk"
export ANDROID_HOME="$ANDROID_SDK_ROOT"
export ANDROID_NDK_HOME="$(find "$ANDROID_SDK_ROOT/ndk" -maxdepth 1 -mindepth 1 -type d | sort | tail -n 1)"
export ANDROID_NDK_ROOT="$ANDROID_NDK_HOME"
export PATH="$JAVA_HOME/bin:$ANDROID_SDK_ROOT/platform-tools:$ANDROID_SDK_ROOT/cmdline-tools/latest/bin:$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
```

Validate the Android target:

```bash
cargo check -p rssr-app --target aarch64-linux-android
```

Build a debug APK:

```bash
dx bundle --platform android --package rssr-app --release --debug-symbols false
```

Output:

```text
target/dx/rssr-app/release/android/app/app/build/outputs/apk/debug/app-debug.apk
```

### Run CLI

```bash
cargo run -p rssr-cli -- --help
```

Examples:

```bash
cargo run -p rssr-cli -- list-feeds
cargo run -p rssr-cli -- add-feed https://example.com/feed.xml
cargo run -p rssr-cli -- export-config --output ./config.json
cargo run -p rssr-cli -- save-settings --custom-css-file ./assets/themes/newsprint.css
```

## Verification

```bash
cargo fmt --all
cargo test --workspace
cargo check -p rssr-app --target wasm32-unknown-unknown
```

## Desktop packaging

### Windows

Build on a Windows host:

```powershell
cargo build --release -p rssr-app
```

Output:

```text
target\release\rssr-app.exe
```

Notes:
- the desktop app is a native Windows executable
- Microsoft WebView2 Runtime is typically required on the target machine

### GitHub Release artifacts

The release workflow publishes:
- `rssr-app-windows-x86_64.zip`
- `rssr-cli-windows-x86_64.zip`
- `rssr-app-linux-x86_64.tar.gz`
- `rssr-cli-linux-x86_64.tar.gz`
- `rssr-app-macos-x86_64.tar.gz`
- `rssr-cli-macos-x86_64.tar.gz`
- `rssr-app-macos-aarch64.tar.gz`
- `rssr-cli-macos-aarch64.tar.gz`
- `rssr-app-android-debug.apk`
- `rssr-app-web.tar.gz`

If Android signing secrets are configured, the release workflow also publishes:
- `rssr-app-android-release.apk`
- `rssr-app-android-release.aab`

Current automatic release targets are:
- Windows desktop
- Linux desktop
- macOS desktop
- Android debug APK
- Web static bundle

The Android pipeline always publishes an unsigned debug APK for installation testing. If signing secrets are configured, it also publishes a signed release APK and AAB.

Android signing secrets expected by GitHub Actions:
- `ANDROID_KEYSTORE_BASE64`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

`dx serve` supports additional platform modes, but not all of them map cleanly to end-user GitHub Release assets. iOS, server, and liveview targets are not yet published as release attachments.

Tag a release to trigger it:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## Docker and docker compose

Docker support is for the web build.

The image bundles the web app and serves it with Nginx.

### Local image build

```bash
docker build -t rss-reader-web .
```

### Local compose run

```bash
docker compose up --build
```

Then open:

```text
http://127.0.0.1:8080
```

The compose file is also compatible with the GitHub-published image:

```text
ghcr.io/develata/rss-reader:latest
```

## CI/CD

This repository includes three GitHub Actions workflows:

- `ci.yml`
  - formatting
  - workspace tests
  - wasm target check
  - web bundle verification
- `release.yml`
  - builds release artifacts on tags
  - publishes GitHub Release assets
- `docker.yml`
  - builds and pushes a GHCR image

## Theming and docs

Design and theming docs live under [`docs/`](./docs):

- [frontend styling philosophy](./docs/design/frontend-command-and-styling-philosophy.md)
- [theme selector reference](./docs/design/theme-author-selector-reference.md)
- [manual regression notes](./docs/回归手动测试.md)

## License

MIT
