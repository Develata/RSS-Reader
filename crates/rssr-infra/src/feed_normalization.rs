use anyhow::Context;
use feed_rs::model::{Entry as FeedRsEntry, Feed as FeedRsFeed, Link, Text};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub site_url: Option<Url>,
    pub description: Option<String>,
    pub entries: Vec<ParsedEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedEntry {
    pub external_id: String,
    pub dedup_key: String,
    pub url: Option<Url>,
    pub title: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub content_html: Option<String>,
    pub content_text: Option<String>,
    pub published_at: Option<OffsetDateTime>,
    pub updated_at_source: Option<OffsetDateTime>,
}

pub fn parse_feed_xml(raw: &str) -> anyhow::Result<ParsedFeed> {
    // feed-rs normally synthesizes IDs for entries that omit one. Downstream we need to
    // distinguish a source-provided GUID from that fallback; guessing by "looks like a hash"
    // incorrectly discards legitimate hexadecimal GUIDs. Keep missing IDs empty here and let
    // our normalization choose the URL/title+timestamp fallback explicitly.
    let parser = feed_rs::parser::Builder::new().id_generator(|_, _, _| String::new()).build();
    let feed = parser.parse(raw.as_bytes()).context("解析 RSS/Atom feed 失败")?;
    Ok(normalize_feed(feed))
}

pub fn hash_content(html: Option<&str>, text: Option<&str>, title: Option<&str>) -> Option<String> {
    let mut hasher = Sha256::new();
    let mut used = false;
    for part in [title, text, html].into_iter().flatten() {
        hasher.update(part.as_bytes());
        used = true;
    }
    used.then(|| format!("{:x}", hasher.finalize()))
}

fn normalize_feed(feed: FeedRsFeed) -> ParsedFeed {
    let title = text_value(feed.title.as_ref());
    // 只认 alternate：`self` 是 feed 自身、`hub` 是 WebSub 服务。没有时留空，
    // 添加订阅会回落到发现页地址，刷新则保留已存的值（`COALESCE`）。
    let site_url = alternate_url(&feed.links);
    let description = feed.description.as_ref().map(text_content);
    let entries = feed.entries.into_iter().map(normalize_entry).collect();

    ParsedFeed { title, site_url, description, entries }
}

/// 没有摘要也没有正文的条目同样保留：只建索引（`has_content = false`），阅读页给出原文链接。
/// YouTube 频道（正文在 `media:group`）和只有标题与链接的条目都属于这一类；
/// 正文缓存边界不变——feed 没给的正文不去原网页补抓。
fn normalize_entry(entry: FeedRsEntry) -> ParsedEntry {
    let title = text_value(entry.title.as_ref()).unwrap_or_else(|| "Untitled entry".to_string());
    // 文章页是 alternate，不是 `replies`/`enclosure`；全无 alternate 时沿用第一条可解析链接。
    let url = alternate_url(&entry.links)
        .or_else(|| entry.links.iter().find_map(|link| Url::parse(&link.href).ok()));
    let author = entry.authors.first().map(|author| author.name.clone());
    let summary = entry.summary.as_ref().map(text_content);
    let content_html = entry.content.as_ref().and_then(|content| content.body.clone());
    let content_text = summary.clone();

    let published_at = entry.published.and_then(to_offset_datetime);
    let updated_at_source = entry.updated.and_then(to_offset_datetime);
    let stable_source_id = normalize_source_entry_id(&entry.id);
    let external_id = if stable_source_id.is_empty() {
        url.as_ref()
            .map(|url| url.to_string())
            .unwrap_or_else(|| dedup_key_fallback(&title, published_at))
    } else {
        stable_source_id.clone()
    };
    let dedup_key = if !stable_source_id.is_empty() {
        stable_source_id
    } else if let Some(url) = &url {
        url.to_string()
    } else {
        dedup_key_fallback(&title, published_at)
    };

    ParsedEntry {
        external_id,
        dedup_key,
        url,
        title,
        author,
        summary,
        content_html,
        content_text,
        published_at,
        updated_at_source,
    }
}

/// RFC 4287 §4.2.7.2：没有 rel 即 alternate（feed-rs 已为 Atom 补齐，RSS 的 `<link>` 则没有 rel），
/// IANA 注册名的绝对 IRI 形式与短名等价。
fn alternate_url(links: &[Link]) -> Option<Url> {
    const IANA_RELATIONS: &str = "http://www.iana.org/assignments/relation/";
    links
        .iter()
        .filter(|link| {
            link.rel.as_deref().map(str::trim).is_none_or(|rel| {
                rel.strip_prefix(IANA_RELATIONS).unwrap_or(rel).eq_ignore_ascii_case("alternate")
            })
        })
        .find_map(|link| Url::parse(&link.href).ok())
}

fn dedup_key_fallback(title: &str, published_at: Option<OffsetDateTime>) -> String {
    let timestamp = published_at
        .and_then(|value| value.format(&time::format_description::well_known::Rfc3339).ok())
        .unwrap_or_else(|| "unknown".to_string());
    let normalized_title = title.trim().to_lowercase();
    let mut hasher = Sha256::new();
    hasher.update(normalized_title.as_bytes());
    hasher.update(timestamp.as_bytes());
    format!("title-ts:{:x}", hasher.finalize())
}

fn normalize_source_entry_id(raw: &str) -> String {
    raw.trim().to_string()
}

fn text_value(text: Option<&Text>) -> Option<String> {
    text.map(text_content).and_then(|value| {
        let trimmed = value.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn text_content(text: &Text) -> String {
    text.content.clone()
}

/// feed 的日期完全由远端控制，`chrono` 能表示的年份区间比 `time::OffsetDateTime` 宽，因此越界的
/// 日期只丢弃这一个时间戳，不要让整轮刷新 panic。
fn to_offset_datetime<Tz>(value: chrono::DateTime<Tz>) -> Option<OffsetDateTime>
where
    Tz: chrono::TimeZone,
    Tz::Offset: Send + Sync,
{
    OffsetDateTime::from_unix_timestamp(value.timestamp()).ok()
}

#[cfg(test)]
mod tests {
    use super::parse_feed_xml;

    /// 产品决策（2026-10-09）：无摘要、无正文的条目保留为仅索引条目，而不是整条丢弃。
    #[test]
    fn keeps_entries_without_summary_or_content_as_index_only() {
        let youtube_like = parse_feed_xml(
            r#"<feed xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
 <title>Channel</title><id>yt:channel:UC123</id>
 <entry>
  <id>yt:video:abc</id><title>Video One</title>
  <link rel="alternate" href="https://www.youtube.com/watch?v=abc"/>
  <published>2026-10-01T00:00:00+00:00</published>
  <media:group><media:title>Video One</media:title><media:description>Desc</media:description></media:group>
 </entry>
</feed>"#,
        )
        .expect("parse youtube-like feed");
        let title_only = parse_feed_xml(
            r#"<rss version="2.0"><channel><title>Links</title>
 <item><title>Only a title</title><link>https://example.com/a</link><guid>a</guid></item>
</channel></rss>"#,
        )
        .expect("parse title-only feed");

        for (feed, title, url) in [
            (&youtube_like, "Video One", "https://www.youtube.com/watch?v=abc"),
            (&title_only, "Only a title", "https://example.com/a"),
        ] {
            assert_eq!(feed.entries.len(), 1);
            let entry = &feed.entries[0];
            assert_eq!(entry.title, title);
            assert_eq!(entry.url.as_ref().map(url::Url::as_str), Some(url));
            assert_eq!(entry.content_html, None);
            assert_eq!(entry.content_text, None);
            assert_eq!(entry.summary, None);
        }
    }

    /// WordPress 等 RSS 2.0 在 `<link>` 之前输出 `atom:link rel="self"` 与 WebSub 的 `rel="hub"`。
    #[test]
    fn rss_site_url_skips_self_and_hub_links() {
        let feed = parse_feed_xml(
            r#"<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom"><channel><title>Blog</title>
 <atom:link href="https://example.com/feed/" rel="self" type="application/rss+xml"/>
 <atom:link href="https://hub.example.net/" rel="hub"/>
 <link>https://example.com/</link>
 <item><title>Post</title><link>https://example.com/post</link><guid>p</guid></item>
</channel></rss>"#,
        )
        .expect("parse rss");
        assert_eq!(feed.site_url.as_ref().map(url::Url::as_str), Some("https://example.com/"));
        assert_eq!(
            feed.entries[0].url.as_ref().map(url::Url::as_str),
            Some("https://example.com/post")
        );
    }

    /// 只有 `self` 时不把 feed 地址当作站点；条目没有 alternate 时沿用第一条链接（如播客的 enclosure）。
    #[test]
    fn missing_alternate_leaves_site_empty_and_entry_on_its_first_link() {
        let feed = parse_feed_xml(
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><title>F</title><id>f</id>
 <link rel="self" href="https://example.com/feed.atom"/>
 <entry><id>e1</id><title>E</title>
  <link rel="enclosure" href="https://cdn.example.com/e1.mp3"/>
  <link rel="http://www.iana.org/assignments/relation/alternate" href="https://example.com/e1"/>
 </entry>
 <entry><id>e2</id><title>E2</title><link rel="enclosure" href="https://cdn.example.com/e2.mp3"/></entry>
</feed>"#,
        )
        .expect("parse atom");
        assert_eq!(feed.site_url, None);
        let urls =
            feed.entries.iter().map(|e| e.url.as_ref().map(url::Url::as_str)).collect::<Vec<_>>();
        assert_eq!(urls, [Some("https://example.com/e1"), Some("https://cdn.example.com/e2.mp3")]);
    }
}
