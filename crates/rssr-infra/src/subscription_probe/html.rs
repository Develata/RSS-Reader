use html5ever::tokenizer::{
    BufferQueue, EndTag, StartTag, Token, TokenSink, TokenSinkResult, Tokenizer,
};
use rssr_application::FeedDiscoveryCandidate;
use std::cell::RefCell;
use url::Url;

/// 一个页面里最多采纳的 `<link rel="alternate">` 订阅声明数。
const MAX_DISCOVERED_LINKS: usize = 64;

#[derive(Default)]
struct Head {
    finished: bool,
    raw: Option<String>,
    template_depth: usize,
    base: Option<String>,
    links: Vec<(String, Option<String>)>,
}
struct Sink(RefCell<Head>);
impl TokenSink for Sink {
    type Handle = ();
    fn process_token(&self, token: Token, _: u64) -> TokenSinkResult<()> {
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
        let attr = |key: &str| {
            tag.attrs.iter().find(|a| a.name.local.as_ref() == key).map(|a| a.value.to_string())
        };
        if name == "base" && head.base.is_none() {
            head.base = attr("href");
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
            && let Some(href) = attr("href")
        {
            // 页面头部声明的订阅地址最多看这么多个：每个都要相对基地址拼接一次。
            if head.links.len() < MAX_DISCOVERED_LINKS {
                head.links.push((href, attr("title")));
            }
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

pub(super) fn discover(page_url: &Url, body: &str) -> Vec<FeedDiscoveryCandidate> {
    let queue = BufferQueue::default();
    queue.push_back(body.into());
    let tokenizer = Tokenizer::new(Sink(RefCell::new(Head::default())), Default::default());
    let _ = tokenizer.feed(&queue);
    tokenizer.end();
    let head = tokenizer.sink.0.into_inner();
    let base = head
        .base
        .and_then(|href| rssr_domain::join_untrusted_url(page_url, href.trim()))
        .filter(|u| matches!(u.scheme(), "http" | "https"))
        .unwrap_or_else(|| page_url.clone());
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (href, title) in head.links {
        if let Some(url) = rssr_domain::join_untrusted_url(&base, href.trim())
            && matches!(url.scheme(), "http" | "https")
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
    use url::Url;

    use super::{MAX_DISCOVERED_LINKS, discover};

    #[test]
    fn declared_feed_links_are_capped_and_base_href_is_guarded() {
        let page = Url::parse("https://e.test/blog/").unwrap();
        let links: String = (0..200)
            .map(|i| {
                format!(r#"<link rel="alternate" type="application/rss+xml" href="/f{i}.xml">"#)
            })
            .collect();
        let html = format!("<html><head>{links}</head><body></body></html>");
        assert_eq!(discover(&page, &html).len(), MAX_DISCOVERED_LINKS);

        // 点段过多的 `<base href>` 不参与拼接，退回页面地址。
        let base = format!("https://{}@e.test/{}", "a".repeat(1 << 12), "../".repeat(1 << 12));
        let html = format!(
            r#"<html><head><base href="{base}"><link rel="alternate" type="application/atom+xml" href="feed.xml"></head></html>"#
        );
        let found = discover(&page, &html);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].url.as_str(), "https://e.test/blog/feed.xml");
    }
}
