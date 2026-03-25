use anyhow::Context;
use feed_rs::model::{Entry as FeedRsEntry, Feed as FeedRsFeed, Text};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedFeed {
    pub title: Option<String>,
    pub site_url: Option<Url>,
    pub description: Option<String>,
    pub entries: Vec<ParsedEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Default)]
pub struct FeedParser;

impl FeedParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, raw: &str) -> anyhow::Result<ParsedFeed> {
        let feed = feed_rs::parser::parse(raw.as_bytes()).context("解析 RSS/Atom feed 失败")?;
        self.normalize_feed(feed)
    }

    fn normalize_feed(&self, feed: FeedRsFeed) -> anyhow::Result<ParsedFeed> {
        let title = text_value(feed.title.as_ref());
        let site_url = feed.links.first().and_then(|link| Url::parse(link.href.as_str()).ok());
        let description = feed.description.as_ref().map(text_content);

        let mut entries = Vec::with_capacity(feed.entries.len());
        for entry in feed.entries {
            if let Some(parsed_entry) = normalize_entry(entry)? {
                entries.push(parsed_entry);
            }
        }

        Ok(ParsedFeed { title, site_url, description, entries })
    }
}

fn normalize_entry(entry: FeedRsEntry) -> anyhow::Result<Option<ParsedEntry>> {
    let title = text_value(entry.title.as_ref()).unwrap_or_else(|| "Untitled entry".to_string());
    let url = entry.links.first().and_then(|link| Url::parse(link.href.as_str()).ok());
    let author = entry.authors.first().map(|author| author.name.clone());
    let summary = entry.summary.as_ref().map(text_content);
    let content_html = entry.content.as_ref().and_then(|content| content.body.clone());
    let content_text = summary.clone();
    if content_html.is_none() && content_text.is_none() {
        tracing::warn!(entry_title = %title, "跳过缺少 summary 和 content 的 feed 条目");
        return Ok(None);
    }

    let published_at = entry.published.map(to_offset_datetime);
    let updated_at_source = entry.updated.map(to_offset_datetime);
    let stable_source_id = normalize_source_entry_id(&entry.id, url.as_ref());
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

    Ok(Some(ParsedEntry {
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
    }))
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

fn normalize_source_entry_id(raw: &str, url: Option<&Url>) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if url.is_some() && looks_like_synthetic_hash(trimmed) {
        return String::new();
    }

    trimmed.to_string()
}

fn looks_like_synthetic_hash(value: &str) -> bool {
    matches!(value.len(), 32 | 40 | 64) && value.chars().all(|ch| ch.is_ascii_hexdigit())
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

fn to_offset_datetime<Tz>(value: chrono::DateTime<Tz>) -> OffsetDateTime
where
    Tz: chrono::TimeZone,
    Tz::Offset: Send + Sync,
{
    OffsetDateTime::from_unix_timestamp(value.timestamp()).expect("valid unix timestamp")
}
