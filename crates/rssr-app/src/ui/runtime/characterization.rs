//! Characterization of the UI seam: every `UiCommand` variant runs through
//! [`super::execute_ui_command`] against a real library (in memory) and real HTTP (a local
//! server), and the resulting `UiIntent`s are compared with a recorded transcript. Refactors of
//! the seam must leave the transcript unchanged.
//!
//! Volatile parts are normalised: the server port (`<port>`), and timestamps — a time near the
//! moment of the run becomes `<now>`, any other time is rewritten in UTC (so fixed fixture times
//! stay checked whatever the machine's time zone), and text that does not parse as a time is left
//! alone. To record a new transcript after an intended change, run with
//! `RSSR_UPDATE_UI_TRANSCRIPT=1` and review the diff.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use rssr_application::SettingsPatch;
use rssr_domain::{EntryGroupingPreference, EntryQuery, ReadFilter, StarredFilter, StartupView};
use rssr_infra::store::LibraryStore;

use super::execute_ui_command;
use crate::bootstrap::AppServices;
use crate::pages::entries_page::intent::EntriesPageIntent;
use crate::pages::feeds_page::intent::FeedsPageIntent;
use crate::ui::commands::{
    EntriesCommand, FeedsCommand, ReaderCommand, SettingsCommand, ShellCommand, UiCommand,
};
use crate::ui::runtime::backend::test_host;
use crate::ui::snapshot::UiIntent;

/// A 1×1 PNG.
const PIXEL_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

const TRANSCRIPT: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/runtime/characterization.transcript");

fn rss(title: &str, items: &[(&str, &str, &str)]) -> String {
    let items: String = items
        .iter()
        .map(|(guid, item_title, date)| {
            format!(
                "<item><guid isPermaLink=\"false\">{guid}</guid><title>{item_title}</title>\
                 <link>https://example.invalid/{guid}</link><pubDate>{date}</pubDate>\
                 <description>Summary of {item_title}</description>\
                 <content:encoded><![CDATA[<p>Body of {item_title}</p>]]></content:encoded></item>"
            )
        })
        .collect();
    format!(
        "<?xml version=\"1.0\"?><rss version=\"2.0\" \
         xmlns:content=\"http://purl.org/rss/1.0/modules/content/\"><channel>\
         <title>{title}</title><link>https://example.invalid/</link>\
         <description>{title} feed</description>{items}</channel></rss>"
    )
}

/// A local HTTP server: feeds, a discovery page, a failing feed and a WebDAV file.
struct Server {
    port: u16,
}

impl Server {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let dav: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let dav = Arc::clone(&dav);
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                    let mut line = String::new();
                    if reader.read_line(&mut line).is_err() {
                        return;
                    }
                    let mut parts = line.split_whitespace();
                    let (method, path) = (
                        parts.next().unwrap_or("").to_owned(),
                        parts.next().unwrap_or("").to_owned(),
                    );
                    let mut length = 0;
                    loop {
                        let mut header = String::new();
                        if reader.read_line(&mut header).is_err() || header.trim().is_empty() {
                            break;
                        }
                        if let Some((name, value)) = header.split_once(':')
                            && name.eq_ignore_ascii_case("content-length")
                        {
                            length = value.trim().parse().unwrap_or(0);
                        }
                    }
                    let mut body = vec![0; length];
                    let _ = reader.read_exact(&mut body);
                    let (status, content_type, payload) = route(port, &method, &path, body, &dav);
                    let head = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\n\
                         Content-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&payload);
                });
            }
        });
        Self { port }
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }
}

fn route(
    port: u16,
    method: &str,
    path: &str,
    body: Vec<u8>,
    dav: &Mutex<HashMap<String, Vec<u8>>>,
) -> (&'static str, &'static str, Vec<u8>) {
    const XML: &str = "application/rss+xml";
    match (method, path) {
        ("GET", "/img.xml") => (
            "200 OK",
            XML,
            format!(
                "<?xml version=\"1.0\"?><rss version=\"2.0\" \
                 xmlns:content=\"http://purl.org/rss/1.0/modules/content/\"><channel>\
                 <title>Feed I</title><link>https://example.invalid/</link>\
                 <description>images</description><item><guid>i1</guid><title>Image one</title>\
                 <link>https://example.invalid/i1</link><pubDate>Sun, 05 Apr 2026 10:00:00 GMT</pubDate>\
                 <content:encoded><![CDATA[<p>Picture</p><img src=\"http://127.0.0.1:{port}/pixel.png\">]]>\
                 </content:encoded></item></channel></rss>"
            )
            .into_bytes(),
        ),
        ("GET", "/pixel.png") => ("200 OK", "image/png", PIXEL_PNG.to_vec()),
        ("GET", "/a.xml") => (
            "200 OK",
            XML,
            rss(
                "Feed A",
                &[
                    ("a1", "Alpha one", "Wed, 01 Apr 2026 10:00:00 GMT"),
                    ("a2", "Alpha two", "Thu, 02 Apr 2026 10:00:00 GMT"),
                    ("a3", "Alpha three", "Fri, 03 Apr 2026 10:00:00 GMT"),
                ],
            )
            .into_bytes(),
        ),
        ("GET", "/b.xml") => (
            "200 OK",
            XML,
            rss("Feed B", &[("b1", "Bravo one", "Sat, 04 Apr 2026 10:00:00 GMT")]).into_bytes(),
        ),
        ("GET", "/page.html") => (
            "200 OK",
            "text/html",
            format!(
                "<html><head><title>Page</title>\
                 <link rel=\"alternate\" type=\"application/rss+xml\" title=\"First\" \
                 href=\"http://127.0.0.1:{port}/a.xml\">\
                 <link rel=\"alternate\" type=\"application/rss+xml\" title=\"Second\" \
                 href=\"http://127.0.0.1:{port}/b.xml\"></head><body></body></html>"
            )
            .into_bytes(),
        ),
        ("PUT", path) if path.starts_with("/dav/") => {
            dav.lock().expect("dav").insert(path.to_owned(), body);
            ("201 Created", "text/plain", Vec::new())
        }
        ("GET", path) if path.starts_with("/dav/") => match dav.lock().expect("dav").get(path) {
            Some(stored) => ("200 OK", "application/json", stored.clone()),
            None => ("404 Not Found", "text/plain", Vec::new()),
        },
        ("GET", "/broken.xml") => ("500 Internal Server Error", "text/plain", b"down".to_vec()),
        _ => ("404 Not Found", "text/plain", Vec::new()),
    }
}

/// Normalises what changes between runs and machines: the server port, and times (see the
/// module documentation).
fn normalise(text: &str, port: u16) -> String {
    let text = text
        .replace(&format!("127.0.0.1:{port}"), "127.0.0.1:<port>")
        .replace(&format!("port: Some({port})"), "port: Some(<port>)");
    let now = time::OffsetDateTime::now_utc();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < text.len() {
        if let Some((length, at)) = timestamp_at(&text.as_bytes()[index..]) {
            if (at - now).abs() < time::Duration::days(2) {
                out.push_str("<now>");
            } else {
                let utc = at.to_offset(time::UtcOffset::UTC);
                out.push_str(&format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    utc.year(),
                    u8::from(utc.month()),
                    utc.day(),
                    utc.hour(),
                    utc.minute(),
                    utc.second()
                ));
            }
            index += length;
        } else {
            let ch = text[index..].chars().next().expect("char");
            out.push(ch);
            index += ch.len_utf8();
        }
    }
    out
}

/// A time at the start of `bytes` and its length: `YYYY-MM-DD`, a space or `T`, `H:MM` or `HH:MM`,
/// optional `:SS` and fraction, then an offset (`Z`, `+hh:mm`, ` +hh:mm:ss` as `time` prints it,
/// ` UTC+hh:mm` as the reader shows it). Only text that is a valid instant counts.
fn timestamp_at(bytes: &[u8]) -> Option<(usize, time::OffsetDateTime)> {
    let digit = |index: usize| bytes.get(index).is_some_and(u8::is_ascii_digit);
    let at = |index: usize, byte: u8| bytes.get(index) == Some(&byte);
    let number = |from: usize, count: usize| -> Option<u32> {
        (from..from + count).all(digit).then(|| {
            bytes[from..from + count]
                .iter()
                .fold(0, |value, byte| value * 10 + u32::from(byte - b'0'))
        })
    };
    let (year, month, day) = (number(0, 4)?, number(5, 2)?, number(8, 2)?);
    if !(at(4, b'-') && at(7, b'-') && (at(10, b' ') || at(10, b'T'))) {
        return None;
    }
    let mut end = 11;
    let hour_digits = if digits_at(bytes, end, 2) { 2 } else { 1 };
    let hour = number(end, hour_digits)?;
    end += hour_digits;
    if !at(end, b':') {
        return None;
    }
    let minute = number(end + 1, 2)?;
    end += 3;
    let mut second = 0;
    let mut nanosecond = 0;
    if at(end, b':') && digits_at(bytes, end + 1, 2) {
        second = number(end + 1, 2)?;
        end += 3;
        if at(end, b'.') && digit(end + 1) {
            end += 1;
            let mut scale = 100_000_000;
            while digit(end) {
                nanosecond += u32::from(bytes[end] - b'0') * scale;
                scale /= 10;
                end += 1;
            }
        }
    }
    let offset_at = |start: usize| -> Option<(usize, i32)> {
        let sign = match bytes.get(start) {
            Some(b'+') => 1,
            Some(b'-') => -1,
            _ => return None,
        };
        let hours = number(start + 1, 2)? as i32;
        if !at(start + 3, b':') {
            return None;
        }
        let minutes = number(start + 4, 2)? as i32;
        let (stop, seconds) = match number(start + 7, 2) {
            Some(seconds) if at(start + 6, b':') => (start + 9, seconds as i32),
            _ => (start + 6, 0),
        };
        Some((stop, sign * (hours * 3600 + minutes * 60 + seconds)))
    };
    let (stop, offset_seconds) = if at(end, b'Z') {
        (end + 1, 0)
    } else if let Some(found) = offset_at(end) {
        found
    } else if at(end, b' ')
        && let Some(found) = offset_at(end + 1)
    {
        found
    } else if bytes.get(end..end + 4) == Some(b" UTC")
        && let Some(found) = offset_at(end + 4)
    {
        found
    } else {
        return None;
    };
    let date = time::Date::from_calendar_date(
        i32::try_from(year).ok()?,
        time::Month::try_from(u8::try_from(month).ok()?).ok()?,
        u8::try_from(day).ok()?,
    )
    .ok()?;
    let clock = time::Time::from_hms_nano(
        u8::try_from(hour).ok()?,
        u8::try_from(minute).ok()?,
        u8::try_from(second).ok()?,
        nanosecond,
    )
    .ok()?;
    let offset = time::UtcOffset::from_whole_seconds(offset_seconds).ok()?;
    Some((stop, time::PrimitiveDateTime::new(date, clock).assume_offset(offset)))
}

fn digits_at(bytes: &[u8], from: usize, count: usize) -> bool {
    bytes.get(from..from + count).is_some_and(|slice| slice.iter().all(u8::is_ascii_digit))
}

struct Transcript {
    port: u16,
    text: String,
}

impl Transcript {
    async fn run(&mut self, label: &str, command: UiCommand) -> Vec<UiIntent> {
        let intents = execute_ui_command(command).await;
        self.text.push_str(&format!("## {label}\n"));
        for intent in &intents {
            self.text.push_str(&normalise(&format!("{intent:?}"), self.port));
            self.text.push('\n');
        }
        intents
    }
}

fn library() -> Arc<AppServices> {
    let store = std::thread::spawn(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(LibraryStore::open_in_memory())
            .expect("library")
    })
    .join()
    .expect("open");
    AppServices::over(store)
}

#[tokio::test(flavor = "current_thread")]
async fn ui_commands_produce_the_recorded_intents() {
    let server = Server::start();
    let mut transcript = Transcript { port: server.port, text: String::new() };
    let _host = test_host::install(library());

    let feeds = |command| UiCommand::Feeds(command);
    let entries = |command| UiCommand::Entries(command);
    let reader = |command| UiCommand::Reader(command);
    let settings = |command| UiCommand::Settings(command);
    let shell = |command| UiCommand::Shell(command);

    transcript.run("startup route, empty", shell(ShellCommand::ResolveStartupRoute)).await;
    transcript.run("feeds snapshot, empty", feeds(FeedsCommand::LoadSnapshot)).await;
    for (label, path) in [
        ("add feed a", "/a.xml"),
        ("add feed b", "/b.xml"),
        ("add discovery page", "/page.html"),
        ("add failing feed", "/broken.xml"),
        ("add duplicate feed a", "/a.xml"),
        ("add feed with an image", "/img.xml"),
    ] {
        let command = FeedsCommand::AddFeed { raw_url: server.url(path), fallback_site_url: None };
        transcript.run(label, feeds(command)).await;
    }
    transcript.run("feeds snapshot", feeds(FeedsCommand::LoadSnapshot)).await;

    transcript
        .run(
            "entries bootstrap, all",
            entries(EntriesCommand::Bootstrap {
                feed_id: None,
                load_preferences: true,
                load_feeds: true,
            }),
        )
        .await;
    transcript
        .run(
            "entries bootstrap, feed 1",
            entries(EntriesCommand::Bootstrap {
                feed_id: Some(1),
                load_preferences: false,
                load_feeds: false,
            }),
        )
        .await;
    transcript
        .run("list entries", entries(EntriesCommand::LoadEntries { query: EntryQuery::default() }))
        .await;
    transcript
        .run(
            "list entries of feed 2",
            entries(EntriesCommand::LoadEntries {
                query: EntryQuery { feed_id: Some(2), ..EntryQuery::default() },
            }),
        )
        .await;
    transcript
        .run(
            "toggle read in list",
            entries(EntriesCommand::SetRead {
                entry_id: 1,
                entry_title: "Alpha one".into(),
                read: true,
            }),
        )
        .await;
    transcript
        .run(
            "toggle starred in list",
            entries(EntriesCommand::SetStarred {
                entry_id: 2,
                entry_title: "Alpha two".into(),
                starred: true,
            }),
        )
        .await;
    // Target values: a repeat changes nothing; a missing entry is reported, not an error.
    transcript
        .run(
            "repeat read in list",
            entries(EntriesCommand::SetRead {
                entry_id: 1,
                entry_title: "Alpha one".into(),
                read: true,
            }),
        )
        .await;
    transcript
        .run(
            "set read on a missing entry",
            entries(EntriesCommand::SetRead {
                entry_id: 999,
                entry_title: "Gone".into(),
                read: true,
            }),
        )
        .await;
    transcript
        .run(
            "save browsing preferences",
            entries(EntriesCommand::SaveBrowsingPreferences {
                grouping_mode: EntryGroupingPreference::Source,
                show_archived: true,
                read_filter: ReadFilter::UnreadOnly,
                starred_filter: StarredFilter::All,
                selected_feed_urls: vec![server.url("/a.xml")],
            }),
        )
        .await;
    transcript
        .run(
            "save the same browsing preferences",
            entries(EntriesCommand::SaveBrowsingPreferences {
                grouping_mode: EntryGroupingPreference::Source,
                show_archived: true,
                read_filter: ReadFilter::UnreadOnly,
                starred_filter: StarredFilter::All,
                selected_feed_urls: vec![server.url("/a.xml")],
            }),
        )
        .await;
    transcript
        .run(
            "entries bootstrap after saving preferences",
            entries(EntriesCommand::Bootstrap {
                feed_id: None,
                load_preferences: true,
                load_feeds: false,
            }),
        )
        .await;
    let previewed = transcript
        .run(
            "preview mark read",
            entries(EntriesCommand::PreviewMarkRead { query: EntryQuery::default() }),
        )
        .await;
    let preview = previewed
        .into_iter()
        .find_map(|intent| match intent {
            UiIntent::EntriesPage(EntriesPageIntent::BulkPreview(preview)) => Some(preview),
            _ => None,
        })
        .expect("a preview");
    transcript
        .run(
            "confirm mark read",
            entries(EntriesCommand::ConfirmMarkRead { preview: preview.clone() }),
        )
        .await;
    transcript
        .run("confirm a stale preview", entries(EntriesCommand::ConfirmMarkRead { preview }))
        .await;

    transcript.run("reader load", reader(ReaderCommand::LoadEntry { entry_id: 1 })).await;
    transcript.run("reader load missing", reader(ReaderCommand::LoadEntry { entry_id: 999 })).await;
    transcript
        .run(
            "reader toggle read",
            reader(ReaderCommand::SetRead { entry_id: 1, read: false, via_shortcut: false }),
        )
        .await;
    transcript
        .run(
            "reader toggle starred by shortcut",
            reader(ReaderCommand::SetStarred { entry_id: 1, starred: true, via_shortcut: true }),
        )
        .await;
    transcript
        .run(
            "reader set read on a missing entry",
            reader(ReaderCommand::SetRead { entry_id: 999, read: true, via_shortcut: true }),
        )
        .await;
    transcript
        .run(
            "reader repeat starred by shortcut",
            reader(ReaderCommand::SetStarred { entry_id: 1, starred: true, via_shortcut: true }),
        )
        .await;
    transcript
        .run("reader localize assets", reader(ReaderCommand::LocalizeEntryAssets { entry_id: 1 }))
        .await;
    // The entry with an image: localizing stores it locally; loading it again shows that.
    let image_entry = 5;
    transcript
        .run(
            "reader localize the image entry",
            reader(ReaderCommand::LocalizeEntryAssets { entry_id: image_entry }),
        )
        .await;
    transcript
        .run(
            "reader load the image entry",
            reader(ReaderCommand::LoadEntry { entry_id: image_entry }),
        )
        .await;

    transcript
        .run(
            "refresh feed 1",
            feeds(FeedsCommand::RefreshFeed { feed_id: 1, feed_title: "Feed A".into() }),
        )
        .await;
    transcript.run("manual refresh", shell(ShellCommand::ManualRefresh)).await;

    let exported = transcript.run("export config", feeds(FeedsCommand::ExportConfig)).await;
    let config = exported
        .into_iter()
        .find_map(|intent| match intent {
            UiIntent::FeedsPage(FeedsPageIntent::ConfigTextExported(text)) => Some(text),
            _ => None,
        })
        .expect("exported config");
    transcript.run("export opml", feeds(FeedsCommand::ExportOpml)).await;
    let opml = format!(
        "<?xml version=\"1.0\"?><opml version=\"2.0\"><head><title>t</title></head><body>\
         <outline type=\"rss\" text=\"Extra\" xmlUrl=\"{}\"/></body></opml>",
        server.url("/extra.xml")
    );
    transcript.run("import opml", feeds(FeedsCommand::ImportOpml { raw: opml })).await;
    transcript
        .run("import opml, broken", feeds(FeedsCommand::ImportOpml { raw: "<opml".into() }))
        .await;
    let config_before_settings = config.clone();
    transcript.run("import config", feeds(FeedsCommand::ImportConfig { raw: config })).await;
    transcript
        .run("import config, broken", feeds(FeedsCommand::ImportConfig { raw: "{".into() }))
        .await;

    transcript.run("settings load", settings(SettingsCommand::Load)).await;
    let changed = SettingsPatch {
        startup_view: Some(StartupView::LastFeed),
        custom_css: Some("body { color: red }".into()),
        ..SettingsPatch::default()
    };
    transcript
        .run(
            "settings save",
            settings(SettingsCommand::UpdateSettings {
                patch: changed,
                success_message: "已保存".into(),
            }),
        )
        .await;
    transcript
        .run(
            "settings save, invalid",
            settings(SettingsCommand::UpdateSettings {
                patch: SettingsPatch { entries_page_size: Some(0), ..SettingsPatch::default() },
                success_message: "已保存".into(),
            }),
        )
        .await;
    transcript
        .run(
            "settings save, invalid CSS",
            settings(SettingsCommand::UpdateSettings {
                patch: SettingsPatch { custom_css: Some("a {".into()), ..SettingsPatch::default() },
                success_message: "已保存".into(),
            }),
        )
        .await;
    // The configuration exported before the settings changed brings the old settings back.
    transcript
        .run(
            "import config with other settings",
            feeds(FeedsCommand::ImportConfig { raw: config_before_settings }),
        )
        .await;
    transcript
        .run(
            "push config",
            settings(SettingsCommand::PushConfig {
                endpoint: server.url("/dav/"),
                remote_path: "rssr.json".into(),
            }),
        )
        .await;
    transcript
        .run(
            "pull config",
            settings(SettingsCommand::PullConfig {
                endpoint: server.url("/dav/"),
                remote_path: "rssr.json".into(),
            }),
        )
        .await;
    transcript
        .run(
            "pull missing config",
            settings(SettingsCommand::PullConfig {
                endpoint: server.url("/dav/"),
                remote_path: "missing.json".into(),
            }),
        )
        .await;

    transcript.run("startup route, last feed", shell(ShellCommand::ResolveStartupRoute)).await;
    transcript
        .run(
            "remove feed 2",
            feeds(FeedsCommand::RemoveFeed { feed_id: 2, feed_title: "Feed B".into() }),
        )
        .await;
    // Removing purges the entries: re-adding brings them back unread (they were marked read).
    transcript
        .run(
            "re-add feed b",
            feeds(FeedsCommand::AddFeed { raw_url: server.url("/b.xml"), fallback_site_url: None }),
        )
        .await;
    transcript
        .run(
            "list entries of feed 2 after re-adding",
            entries(EntriesCommand::LoadEntries {
                query: EntryQuery { feed_id: Some(2), ..EntryQuery::default() },
            }),
        )
        .await;
    transcript.run("feeds snapshot, at the end", feeds(FeedsCommand::LoadSnapshot)).await;

    // A library without subscriptions: starting the shell also starts the auto-refresh loop,
    // which then has nothing to fetch.
    drop(_host);
    let _empty = test_host::install(library());
    transcript.run("authenticated shell", shell(ShellCommand::LoadAuthenticatedShell)).await;

    if std::env::var("RSSR_UPDATE_UI_TRANSCRIPT").as_deref() == Ok("1") {
        std::fs::write(TRANSCRIPT, &transcript.text).expect("write transcript");
        return;
    }
    let expected = std::fs::read_to_string(TRANSCRIPT)
        .expect("recorded transcript (run with RSSR_UPDATE_UI_TRANSCRIPT=1 to record)")
        .replace("\r\n", "\n");
    if expected != transcript.text {
        let mismatch = expected
            .lines()
            .zip(transcript.text.lines())
            .enumerate()
            .find(|(_, (left, right))| left != right)
            .map(|(line, (left, right))| {
                format!("line {}:\n  recorded: {left}\n  now:      {right}", line + 1)
            })
            .unwrap_or_else(|| "lengths differ".to_owned());
        panic!("UI transcript changed; {mismatch}");
    }
}
