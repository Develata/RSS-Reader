# RSS-Reader

A local-first RSS reader built with Rust and Dioxus.

This project focuses on practical reading workflows instead of branding-heavy UI or backend-heavy infrastructure:

- desktop app for everyday use
- web build for browser testing and static deployment
- Android debug APK build path
- local SQLite persistence
- JSON / OPML config exchange
- optional WebDAV config sync
- custom CSS theming with preset themes
- companion CLI for automation

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
├── rssr-web/
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

Notes:
- browser builds can only refresh remote feeds that allow cross-origin requests
- some feeds work on desktop/mobile but fail in web due to CORS
- the web build adds a cache-busting query when refreshing feeds to avoid browser `304` cache behavior blocking updates

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
- Android platform 33+
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
dx bundle --platform android --package rssr-app --target aarch64-linux-android --release --debug-symbols false
python3 scripts/prepare_android_bundle.py target/dx/rssr-app/release/android/app/app/src/main
```

Output:

```text
target/dx/rssr-app/release/android/app/app/build/outputs/apk/debug/app-debug.apk
```

The extra patch step rewrites the generated Android launcher resources so the packaged app name and icon stay aligned with the desktop release (`RSS-Reader` / `RSSR`).

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
target\\release\\rssr-app.exe
```

Notes:
- the desktop app is a native Windows executable
- Microsoft WebView2 Runtime is typically required on the target machine

### GitHub Release artifacts

The release workflow publishes:

- `RSS-Reader-windows-x86_64.zip`
- `rssr-cli-windows-x86_64.zip`
- `RSS-Reader-linux-x86_64.tar.gz`
- `rssr-cli-linux-x86_64.tar.gz`
- `RSS-Reader-macos-x86_64.tar.gz`
- `rssr-cli-macos-x86_64.tar.gz`
- `RSS-Reader-macos-aarch64.tar.gz`
- `rssr-cli-macos-aarch64.tar.gz`
- `RSS-Reader-android-arm64-v8a-debug.apk`
- `RSS-Reader-web.tar.gz`

If Android signing secrets are configured, the release workflow also publishes:

- `RSS-Reader-android-arm64-v8a-release.apk`
- `RSS-Reader-android-arm64-v8a-release.aab`

Current automatic release targets are:

- Windows desktop
- Linux desktop
- macOS desktop
- Android debug APK
- Web static bundle

The Android pipeline always publishes an unsigned `arm64-v8a` debug APK for installation testing on modern devices. If signing secrets are configured, it also publishes a signed `arm64-v8a` release APK and AAB.

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

The published image runs a small `rssr-web` process.

It handles:

- a username/password login page
- server-side credential validation
- signed `HttpOnly` session cookies
- serving the Dioxus web bundle only after login

It is **not** a runtime dependency of the desktop app, CLI, Android build, or local development workflow.

You do not need this deployment-time web service for:

- `cargo run -p rssr-app`
- `dx serve --platform web --package rssr-app`

### Pull and run the published image

The default [docker-compose.yml](../docker-compose.yml) is a pull-only deployment template for the image published to GHCR:

```bash
export RSS_READER_WEB_USERNAME=admin
export RSS_READER_WEB_PASSWORD='replace-this-with-a-strong-password'
export RSS_READER_WEB_SESSION_SECRET='use-a-random-secret-with-at-least-32-characters'
docker compose up -d
```

Then open:

```text
http://127.0.0.1:8039
```

Override the image tag or host port if needed:

```bash
RSS_READER_WEB_USERNAME=admin \
RSS_READER_WEB_PASSWORD='replace-this-with-a-strong-password' \
RSS_READER_WEB_SESSION_SECRET='use-a-random-secret-with-at-least-32-characters' \
RSS_READER_IMAGE=ghcr.io/develata/rss-reader:latest \
RSS_READER_PORT=8090 \
docker compose up -d
```

You can also run the image directly:

```bash
docker run --rm \
  -p 8039:8080 \
  -e RSS_READER_WEB_USERNAME=admin \
  -e RSS_READER_WEB_PASSWORD='replace-this-with-a-strong-password' \
  -e RSS_READER_WEB_SESSION_SECRET='use-a-random-secret-with-at-least-32-characters' \
  ghcr.io/develata/rss-reader:latest
```

Notes:
- `RSS_READER_WEB_SESSION_SECRET` should be a random string with at least 32 characters
- if the container is exposed behind HTTPS, set `RSS_READER_WEB_SECURE_COOKIE=true`
- for local HTTP testing you can keep it at the default `false`

### Local image build

```bash
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build
```

This keeps the same compose defaults, but overrides the service to build from the current workspace instead of pulling from GHCR.

## CI/CD

This repository includes three GitHub Actions workflows:

- `ci.yml`
  - formatting
  - workspace tests
  - wasm target check
  - Android target smoke check
- `release.yml`
  - builds release artifacts on tags
  - publishes GitHub Release assets
- `docker.yml`
  - builds and pushes a GHCR image

## Reading cache behavior

- the reader caches whatever body HTML/text the feed already provides
- desktop and Android can localize many body images into cached HTML for better offline reading
- web builds are limited by browser CORS rules, so remote body images may stay on their original URLs even when the article body itself is cached locally
- when the web app is deployed through `rssr-web`, the server can proxy feed fetches, so feeds such as `https://www.ruanyifeng.com/blog/atom.xml` still work even though a plain browser build would hit CORS

## Docs

See the docs index:

- [docs index](./README.md)
- [design index](./design/README.md)
- [frontend styling philosophy](./design/frontend-command-and-styling-philosophy.md)
- [theme selector reference](./design/theme-author-selector-reference.md)
- [testing index](./testing/README.md)
- [Android release roadmap](./roadmaps/android-release-roadmap.md)
- [manual regression notes](./testing/manual-regression.md)

## License

MIT
