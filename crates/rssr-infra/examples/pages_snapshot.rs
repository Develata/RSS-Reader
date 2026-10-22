//! Fetch the curated public feeds before building Pages; no visitor-side feed requests.
//! cargo run --locked -p rssr-infra --example pages_snapshot -- [--sources FILE] [--output FILE]
use std::{cmp::Reverse, collections::HashSet, path::PathBuf, time::Duration};

use anyhow::{Context, Result, ensure};
use rssr_infra::parser::{FeedParser, ParsedFeed};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

#[path = "../../../build-support/pages_snapshot.rs"]
mod snapshot;
use snapshot::{Snapshot, SnapshotFeed, Source};

const MAX_FEED_BYTES: usize = 2 * 1024 * 1024;
const MAX_SNAPSHOT_BYTES: usize = 2 * 1024 * 1024;

fn select_entries(mut feed: ParsedFeed, limit: usize) -> Result<ParsedFeed> {
    ensure!((1..=10).contains(&limit), "max_entries must be between 1 and 10");
    feed.entries.sort_by_key(|entry| Reverse(entry.published_at.or(entry.updated_at_source)));
    let mut seen = HashSet::new();
    feed.entries.retain(|entry| {
        entry.url.as_ref().is_some_and(|url| matches!(url.scheme(), "https" | "http"))
            && entry
                .content_html
                .as_ref()
                .or(entry.content_text.as_ref())
                .is_some_and(|s| !s.trim().is_empty())
            && seen.insert(entry.dedup_key.clone())
    });
    feed.entries.truncate(limit);
    ensure!(!feed.entries.is_empty(), "feed has no readable articles with original URLs");
    Ok(feed)
}

async fn fetch(client: &reqwest::Client, source: Source) -> Result<SnapshotFeed> {
    ensure!(source.url.scheme() == "https", "demo sources must use HTTPS");
    ensure!((1..=10).contains(&source.max_entries), "max_entries must be between 1 and 10");
    let mut response = client.get(source.url.clone()).send().await?.error_for_status()?;
    ensure!(
        response.content_length().is_none_or(|n| n <= MAX_FEED_BYTES as u64),
        "RSS exceeds 2 MiB"
    );
    let final_url = response.url().clone();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(bytes.len() + chunk.len() <= MAX_FEED_BYTES, "RSS exceeds 2 MiB");
        bytes.extend_from_slice(&chunk);
    }
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    // Both curated feeds serve UTF-8. Refuse invalid bytes instead of silently changing text.
    let raw = std::str::from_utf8(&bytes).context("RSS must be UTF-8")?;
    let feed = select_entries(FeedParser::new().parse(raw)?, source.max_entries)?;
    Ok(SnapshotFeed { source, final_url, sha256, feed })
}

async fn run() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = root.join("assets/pages-demo/sources.json");
    let mut output = root.join("assets/pages-demo/snapshot.json");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().context("expected a path after the option")?;
        match flag.as_str() {
            "--sources" => sources = value.into(),
            "--output" => output = value.into(),
            _ => anyhow::bail!("unknown option {flag}"),
        }
    }
    let sources: Vec<Source> = serde_json::from_slice(&std::fs::read(sources)?)?;
    ensure!(!sources.is_empty() && sources.len() <= 5, "expected 1 to 5 curated sources");
    let client = reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(40))
        .redirect(reqwest::redirect::Policy::limited(3))
        .user_agent("RSS-Reader-Pages/0.2 (+https://github.com/Develata/RSS-Reader)")
        .build()?;
    let mut feeds = Vec::new();
    for source in sources {
        let label = source.url.to_string();
        let feed = fetch(&client, source).await.with_context(|| format!("fetch {label}"))?;
        println!(
            "{}: {} articles, RSS SHA-256 {}",
            feed.source.title,
            feed.feed.entries.len(),
            feed.sha256
        );
        feeds.push(feed);
    }
    let snapshot = Snapshot { schema_version: 1, fetched_at: OffsetDateTime::now_utc(), feeds };
    let mut bytes = serde_json::to_vec_pretty(&snapshot)?;
    bytes.push(b'\n');
    ensure!(bytes.len() <= MAX_SNAPSHOT_BYTES, "snapshot exceeds 2 MiB");
    // Do not replace the previous snapshot if any source fails. Readers see one complete file.
    let temporary = output.with_extension("json.partial");
    std::fs::write(&temporary, &bytes)?;
    std::fs::rename(&temporary, &output)?;
    println!("Wrote {} bytes to {} at {}", bytes.len(), output.display(), snapshot.fetched_at);
    Ok(())
}

fn main() -> Result<()> {
    tokio::runtime::Builder::new_current_thread().enable_all().build()?.block_on(run())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_readable_unique_articles_keep_source_metadata() -> Result<()> {
        let raw = r#"<rss version="2.0"><channel><title>Example</title>
          <item><guid>old</guid><title>Older</title><link>https://example.com/old</link><pubDate>Mon, 01 Jun 2026 00:00:00 GMT</pubDate><description>Old body</description></item>
          <item><guid>new</guid><title>Newer</title><link>https://example.com/new</link><pubDate>Tue, 02 Jun 2026 00:00:00 GMT</pubDate><description>New body</description></item>
          <item><guid>new</guid><title>Duplicate</title><link>https://example.com/new</link><pubDate>Tue, 02 Jun 2026 00:00:00 GMT</pubDate><description>New body</description></item>
          <item><guid>empty</guid><title>Empty</title><link>https://example.com/empty</link></item>
        </channel></rss>"#;
        let feed = select_entries(FeedParser::new().parse(raw)?, 2)?;
        assert_eq!(feed.entries.len(), 2);
        assert_eq!(feed.entries[0].title, "Newer");
        assert_eq!(feed.entries[0].url.as_ref().unwrap().as_str(), "https://example.com/new");
        assert!(feed.entries[0].published_at.is_some());
        assert_eq!(feed.entries[1].title, "Older");
        Ok(())
    }

    #[test]
    fn empty_feed_and_invalid_limits_fail() -> Result<()> {
        let feed = FeedParser::new()
            .parse("<rss version=\"2.0\"><channel><title>Empty</title></channel></rss>")?;
        assert!(select_entries(feed.clone(), 0).is_err());
        assert!(select_entries(feed.clone(), 11).is_err());
        assert!(select_entries(feed, 5).is_err());
        Ok(())
    }
}
