# RSS-Reader

> Subscribe, then read.

<p align="center">
  <a href="https://github.com/Develata/RSS-Reader/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Develata/RSS-Reader?display_name=tag"></a>
  <a href="https://github.com/Develata/RSS-Reader/actions/workflows/ci.yml?query=branch%3Amain"><img alt="CI" src="https://github.com/Develata/RSS-Reader/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="../LICENSE"><img alt="MIT License" src="https://img.shields.io/github/license/Develata/RSS-Reader"></a>
  <a href="#get-started"><img alt="Release targets: Windows, Linux, macOS, Android, Web" src="https://img.shields.io/badge/Targets-Windows%20%7C%20Linux%20%7C%20macOS%20%7C%20Android%20%7C%20Web-0078D4"></a>
</p>

![RSS-Reader Web article list with reading navigation and time directory](../assets/readme/rss-reader-overview.png)

_Actual Web interface with sample subscriptions and articles._

RSS-Reader is a local-first RSS reader built with Rust and Dioxus. Desktop, Web, Android, and CLI share the same core subscription and reading behavior. Your article library stays on the current device; JSON, OPML, and optional WebDAV exchange subscriptions and settings.

[Latest release](https://github.com/Develata/RSS-Reader/releases/latest) · [Release notes](https://github.com/Develata/RSS-Reader/releases) · [中文](../README.md) · [Documentation](./README.md) · [Issues](https://github.com/Develata/RSS-Reader/issues)

## Get started

Download the appropriate asset from [Releases](https://github.com/Develata/RSS-Reader/releases/latest):

| Device | App asset | Start |
| --- | --- | --- |
| Windows x64 | `RSS-Reader-windows-x86_64.zip` | Extract to a writable directory and run `RSS-Reader.exe`; WebView2 Runtime is usually required |
| Linux x64 | `RSS-Reader-linux-x86_64.deb` | Install where the package dependencies are available; data is stored under the user's XDG data directory |
| macOS Intel / Apple Silicon | `RSS-Reader-macos-x86_64.tar.gz` / `RSS-Reader-macos-aarch64.tar.gz` | Extract to a writable directory and open `RSS-Reader.app` |
| Android ARM64 | `RSS-Reader-android-arm64-v8a-release.apk` | Install the APK; the AAB is for app stores |
| Web | `RSS-Reader-web.tar.gz` | Static site bundle; use [`rssr-web`](./deployment/web.md) for login and a same-origin feed proxy |

Open Subscribe (the RSS-wave icon in the top bar), add an RSS or Atom URL, then select **R** (Read / Home). From another page, R navigates home; when already home, R manually refreshes all feeds. Repeated clicks share the in-flight refresh. On mobile, pull down at the top of Home to request the same refresh.

The table matches the published `v0.1.19` assets; see the [release notes](https://github.com/Develata/RSS-Reader/releases/tag/v0.1.19) for the exact changes and validation scope. Android's signed APK/AAB was built and checked, but system back, long-press text selection, pull-to-refresh, and image gestures still need real-device acceptance. macOS interaction also remains unverified on a physical machine.

**Linux package:** `v0.1.17` fixes the earlier `.deb` data-directory permission problem and declares its linked library dependencies. The `v0.1.19` release workflow also installed its package on Ubuntu 24.04 and launched it twice as an ordinary user under Xvfb, checking database creation and reuse in a path with Chinese characters and spaces. Minimum library versions come from the Ubuntu 24.04 build environment; check the package's `Depends` before installing on another distribution, especially an older one.

## Reading workflow

- Use the search icon to expand title search; Enter searches, Esc closes it. Filter entries by source, unread status, or favorite status. Long source names remain readable and pagination stays reachable.
- In Reader, use the top-left back button, toggle read/favorite, move to nearby articles, or open a body image in the viewer. Native text selection and copy remain available. Refreshing does not replace the article you are reading.
- Reader shortcuts: `M` toggles read status, `F` toggles favorite, and `←` / `→` move to the previous / next unread article. Native editing shortcuts with Ctrl or Cmd are left alone.
- Settings provide built-in themes and custom CSS. `rssr-cli` covers feed management, refresh, settings, and configuration import/export.

## Local data and limits

The Linux `/usr/bin` installation stores data in `$XDG_DATA_HOME/rss-reader/` (or `~/.local/share/rss-reader/` when unset). Windows, macOS, and portable Linux builds continue to use `RSS-Reader/` next to the executable. The directory contains the SQLite index and article-body databases plus `shell-prefs.json`. Exit the app before backing up the whole directory, including any SQLite WAL files. Data created by an earlier root-run package under `/usr/bin/RSS-Reader/` is not migrated automatically; after exiting, back it up and have an administrator copy the complete directory to the new location and give it to the user. Android stores its databases in the app sandbox; uninstalling removes local data. Web stores serialized state in that browser's `localStorage`; clearing site data removes the local article library.

A separately extracted Linux CLI remains portable and uses its own executable-adjacent database by default. To manage the installed desktop app's database, pass the global `--database-url` option pointing to the installed data directory's `rss-reader.db`.

The reader caches the body provided by the feed; it does not fetch the source page to reconstruct full text. Desktop and Android attempt to localize body images. Direct browser builds can be blocked by feed CORS policies; the login-protected `rssr-web` host offers a same-origin `/feed-proxy`. JSON, OPML, and WebDAV exchange subscriptions and settings, **not** article bodies, read history, or favorites across devices. See the [Web deployment guide](./deployment/web.md).

The product stays focused on subscriptions, reading, basic settings, and basic configuration exchange. See the [functional design philosophy](./design/functional-design-philosophy.md).

## Build and verify

Rust stable is required. Web development also needs the `wasm32-unknown-unknown` target and Dioxus CLI `0.7.9`.

```bash
cargo run --locked -p rssr-app
cargo run --locked -p rssr-cli -- --help

rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.9 --locked
dx serve --platform web --package rssr-app
```

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo check --locked -p rssr-app --target wasm32-unknown-unknown
```

The [mainline validation matrix](./testing/mainline-validation-matrix.md) explains the parallel CI jobs, platform checks, and remaining manual acceptance. A successful build alone does not establish Android or macOS device behavior.

## More documentation

- [Detailed user guide](./user-guide.md) (Chinese): subscriptions, reading, themes, WebDAV, and backups.
- [Web / Docker deployment](./deployment/web.md): GHCR, Compose, login, and `/feed-proxy`.
- [Android build and acceptance status](./roadmaps/android-release-roadmap.md).
- [Design](./design/README.md), [testing](./testing/README.md), and [contributing](../CONTRIBUTING.md).
- [MIT License](../LICENSE).
