use html5ever::tokenizer::{
    BufferQueue, EndTag, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
};
use rssr_application::{
    FeedDiscoveryCandidate, MAX_DISCOVERY_CANDIDATES, MAX_NAME_BYTES, MAX_URL_BYTES,
    truncate_marked_str,
};
use std::{cell::RefCell, collections::HashSet};
use url::Url;

/// Input fed to the tokenizer at a time.
const CHUNK_BYTES: usize = 4 * 1024;
/// Input read without any token completing before reading stops (review C3-R4 R4-01). Checked
/// once per chunk, so a token that does complete spans at most this plus two chunks (40 KiB,
/// review C3-R5 R5-05). Text, scripts and styles are emitted as they are read and do not count.
const MAX_TOKEN_BYTES: usize = 32 * 1024;
/// Attributes read at most (in all tags of the head, a duplicate counted where it is refused).
/// The tokenizer compares each attribute of a tag with the earlier ones of that tag: the work of
/// a tag is at most its attributes times the attributes of the largest tag (about 10 000 within
/// the token bound), so this bound keeps the work of a page under about 10^8 comparisons, however
/// many tags carry them (review C3-R5 R5-01).
const MAX_HEAD_ATTRIBUTES: usize = 16 * 1024;
/// Distinct links collected at most; enough for [`MAX_DISCOVERY_CANDIDATES`] after resolution.
const MAX_COLLECTED_LINKS: usize = 4 * MAX_DISCOVERY_CANDIDATES;

/// The page's first `<base href>`.
#[derive(Default)]
enum BaseHref {
    #[default]
    Absent,
    Href(String),
    /// Longer than [`MAX_URL_BYTES`]: relative links cannot be resolved within the bound.
    Unusable,
}

#[derive(Default)]
struct Head {
    finished: bool,
    raw: Option<String>,
    template_depth: usize,
    base: BaseHref,
    links: Vec<(String, Option<String>)>,
    hrefs: HashSet<String>,
    /// Tokens other than parse errors: a tag still being read emits none (its duplicate
    /// attributes emit errors, which do not count).
    progress: usize,
    /// Attributes of the tags read, and parse errors (each duplicate attribute is one).
    attributes: usize,
}

struct Sink(RefCell<Head>);

impl TokenSink for Sink {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
        match &token {
            Token::ParseError(_) => self.0.borrow_mut().attributes += 1,
            Token::TagToken(tag) => {
                let mut head = self.0.borrow_mut();
                head.progress += 1;
                head.attributes += tag.attrs.len();
            }
            _ => self.0.borrow_mut().progress += 1,
        }
        if let Token::CharacterTokens(text) = &token {
            let mut head = self.0.borrow_mut();
            if head.raw.is_none() && head.template_depth == 0 && !text.trim().is_empty() {
                head.finished = true;
            }
        }
        let Token::TagToken(tag) = token else {
            return TokenSinkResult::Continue;
        };
        let mut head = self.0.borrow_mut();
        let name = tag.name.as_ref();
        if tag.kind == EndTag && head.raw.as_deref() == Some(name) {
            head.raw = None;
        }
        if name == "template" {
            if tag.kind == StartTag {
                head.template_depth += 1;
            } else {
                head.template_depth = head.template_depth.saturating_sub(1);
            }
            return TokenSinkResult::Continue;
        }
        if head.template_depth > 0 {
            return TokenSinkResult::Continue;
        }
        if (name == "head" && tag.kind == EndTag) || (name == "body" && tag.kind == StartTag) {
            head.finished = true;
        }
        if head.finished || tag.kind != StartTag {
            return TokenSinkResult::Continue;
        }
        // Borrowed: a value is copied only once it is known to be within its bound.
        let attr =
            |key: &str| tag.attrs.iter().find(|a| a.name.local.as_ref() == key).map(|a| &*a.value);
        if name == "base"
            && matches!(head.base, BaseHref::Absent)
            && let Some(href) = attr("href")
        {
            // The first `<base href>` applies, even one that is too long (review C3-R4 R4-03).
            head.base = if href.trim().len() > MAX_URL_BYTES {
                BaseHref::Unusable
            } else {
                BaseHref::Href(href.trim().to_owned())
            };
        }
        if name == "link"
            && attr("rel").is_some_and(|rel| {
                rel.split_ascii_whitespace().any(|part| part.eq_ignore_ascii_case("alternate"))
            })
            && attr("type").is_some_and(|t| {
                matches!(
                    t.trim().to_ascii_lowercase().as_str(),
                    "application/rss+xml" | "application/atom+xml"
                )
            })
            && let Some(href) = attr("href").map(str::trim)
            && href.len() <= MAX_URL_BYTES
            && head.hrefs.len() < MAX_COLLECTED_LINKS
            && !head.hrefs.contains(href)
        {
            let title = attr("title").map(|title| truncate_marked_str(title, MAX_NAME_BYTES));
            head.hrefs.insert(href.to_owned());
            head.links.push((href.to_owned(), title));
        }
        // 正文开始的普通元素结束隐式 head，脚本内容交给 tokenizer 原生 raw-text 状态。
        if matches!(name, "script" | "style" | "title" | "noscript") {
            head.raw = Some(name.to_string());
            return TokenSinkResult::RawData(if name == "script" {
                html5ever::tokenizer::states::RawKind::ScriptData
            } else if matches!(name, "title" | "textarea") {
                html5ever::tokenizer::states::RawKind::Rcdata
            } else {
                html5ever::tokenizer::states::RawKind::Rawtext
            });
        }
        if !matches!(name, "html" | "head" | "base" | "link" | "meta" | "noscript" | "template") {
            head.finished = true;
        }
        TokenSinkResult::Continue
    }
}

/// The head of `body`, read in chunks: reading stops where the head ends, after
/// [`MAX_TOKEN_BYTES`] without a token, and past [`MAX_HEAD_ATTRIBUTES`] (the links read until
/// then are kept).
fn read_head(body: &str) -> Head {
    // The byte order mark is removed where the body is decoded (`feed_body::read_feed_text`, on
    // every platform); a U+FEFF here is text (reviews C3-R6 R6-04, C3-R7 R7B-02). The
    // tokenizer's own stripping applies to every `feed`, so chunked reading would drop one in the
    // middle of an address (review C3-R5 R5-04).
    let options = TokenizerOpts { discard_bom: false, ..TokenizerOpts::default() };
    let tokenizer = Tokenizer::new(Sink(RefCell::new(Head::default())), options);
    let queue = BufferQueue::default();
    let mut rest = body;
    let mut without_progress = 0;
    while !rest.is_empty() {
        let mut split = rest.len().min(CHUNK_BYTES);
        while !rest.is_char_boundary(split) {
            split -= 1;
        }
        let (chunk, tail) = rest.split_at(split);
        rest = tail;
        let before = tokenizer.sink.0.borrow().progress;
        queue.push_back(chunk.into());
        let _ = tokenizer.feed(&queue);
        let head = tokenizer.sink.0.borrow();
        if head.finished || head.attributes > MAX_HEAD_ATTRIBUTES {
            drop(head);
            return tokenizer.sink.0.into_inner();
        }
        if head.progress == before {
            without_progress += chunk.len();
            if without_progress > MAX_TOKEN_BYTES {
                drop(head);
                return tokenizer.sink.0.into_inner();
            }
        } else {
            without_progress = 0;
        }
    }
    tokenizer.end();
    tokenizer.sink.0.into_inner()
}

/// The feeds a page offers, at most [`MAX_DISCOVERY_CANDIDATES`] distinct ones.
///
/// Bounded while it is built (reviews C3-R3 R3-01, C3-R4, C3-R5): reading stops at a token
/// over [`MAX_TOKEN_BYTES`] and past [`MAX_HEAD_ATTRIBUTES`]; at most [`MAX_COLLECTED_LINKS`] distinct links are collected, a link or
/// `<base>` longer than [`MAX_URL_BYTES`] is ignored before it is copied, titles are cut before
/// they are copied, and a resolved address longer than the bound is ignored before it is compared.
/// Without a usable base (one too long) only absolute links are offered.
pub(super) fn discover(page_url: &Url, body: &str) -> Vec<FeedDiscoveryCandidate> {
    let head = read_head(body);
    let base = match head.base {
        BaseHref::Unusable => None,
        BaseHref::Href(href) => Some(
            rssr_domain::join_untrusted_url(page_url, &href)
                .filter(|u| matches!(u.scheme(), "http" | "https"))
                .unwrap_or_else(|| page_url.clone()),
        ),
        BaseHref::Absent => Some(page_url.clone()),
    }
    .filter(|base| base.as_str().len() <= MAX_URL_BYTES);
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    for (href, title) in head.links {
        if result.len() == MAX_DISCOVERY_CANDIDATES {
            break;
        }
        let resolved = match &base {
            Some(base) => rssr_domain::join_untrusted_url(base, &href),
            None => rssr_domain::parse_untrusted_url(&href),
        };
        if let Some(url) = resolved
            && matches!(url.scheme(), "http" | "https")
            && url.as_str().len() <= MAX_URL_BYTES
        {
            let url = rssr_domain::normalize_feed_url(&url);
            if seen.insert(url.clone()) {
                result.push(FeedDiscoveryCandidate {
                    url,
                    title: title.filter(|s| !s.trim().is_empty()),
                });
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_bounds_dot_segments_without_hiding_ordinary_feeds() {
        let page: Url = "https://e.test/blog/".parse().unwrap();
        let abusive = "%2\ne%2\re/".repeat(40);
        let html = format!(
            r#"<html><head><base href="{abusive}"><link rel="alternate" type="application/rss+xml" href="{abusive}bad.xml"><link rel="alternate" type="application/rss+xml" href="../feed.xml"></head></html>"#
        );
        let feeds = discover(&page, &html);
        assert_eq!(feeds.len(), 1);
        assert_eq!(feeds[0].url.as_str(), "https://e.test/feed.xml");
    }

    /// Links are collected once each and at most `MAX_COLLECTED_LINKS` of them, whatever the
    /// page repeats or adds (review C3-R4 R4-02).
    #[test]
    fn collection_is_bounded() {
        let link =
            |href: &str| format!("<link rel=alternate type=application/rss+xml href='{href}'>");
        let repeated = link("/same.xml").repeat(5000);
        let distinct = (0..300).map(|index| link(&format!("/{index}.xml"))).collect::<String>();
        let head = read_head(&format!("<html><head>{repeated}{distinct}</head></html>"));
        assert_eq!(head.links.len(), MAX_COLLECTED_LINKS);
        assert_eq!(head.links[0].0, "/same.xml");
        assert_eq!(head.links[1].0, "/0.xml");
    }
}
