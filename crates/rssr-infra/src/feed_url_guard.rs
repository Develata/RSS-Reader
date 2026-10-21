//! feed-rs 解析之前对 feed 正文里地址的预检。
//!
//! feed-rs 会把条目链接、图标、附件、媒体地址交给 `url` 解析，并让每个 `xml:base` 与上一级
//! 拼接、再克隆进每一个元素。两条路径都可以被构造成超线性：
//!
//! - `url` 处理每个 `..` 段都要从序列化结果末尾反向扫描，根路径上扫过整个 authority；
//!   一个带长用户名和大量 `../` 的链接就是平方级（见 [`rssr_domain::url_guard`]）。
//! - 很长的 `xml:base` 会在每个元素上复制一遍：1 MB 的基地址配 1000 个条目约 9 s。
//!
//! 这些代价都发生在 feed-rs 内部，解析完再检查已经来不及，所以在解析之前按 feed-rs 自己的
//! 读法（同一个 quick-xml、同样的配置、同样的文本合并与实体解析）把会被当作地址的字符串
//! 扫一遍，越界就拒绝整个 feed。这里只看 feed-rs 会解析成地址的位置，正文里的省略号等
//! 不受影响。

use feed_quick_xml::escape::{resolve_predefined_entity, unescape};
use feed_quick_xml::events::Event;
use feed_quick_xml::reader::NsReader;
use rssr_domain::{MAX_URL_PATH_DOTS, url_path_dots};
use serde_json::Value;

/// 一条元素路径上所有 `xml:base` 加起来的最大字节数。每个元素都会克隆一份当前基地址，
/// 正常 feed 的基地址是站点地址，几十字节。
const MAX_XML_BASE_BYTES: usize = 512;

/// feed-rs 会把文本内容当作地址解析的元素（RSS 的 `link`/`image/url`，Atom 的 `icon`/`logo`）。
const URL_TEXT_ELEMENTS: &[&str] = &["link", "icon", "logo", "url"];

/// feed-rs 会当作地址解析的属性（Atom `link@href`、`enclosure@url`、`media:*@url` 等），
/// 多算几个只会多检查。
const URL_ATTRIBUTES: &[&str] = &["href", "url", "src", "uri"];

/// feed-rs 判断 XML 还是 JSON 时只看第一次读入的缓冲区（`BufReader` 默认 8 KiB）。
const SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum FeedUrlViolation {
    #[error("feed 中的地址包含过多的点段（超过 {MAX_URL_PATH_DOTS} 个点）")]
    TooManyDots,
    #[error("feed 中的 xml:base 过长（超过 {MAX_XML_BASE_BYTES} 字节）")]
    XmlBaseTooLong,
}

/// 检查 feed 正文里会被 feed-rs 当作地址解析的字符串。正文本身格式不对时不在这里报错，
/// 交给 feed-rs：它用同样的读法，会在同一处失败。
pub(crate) fn check_feed_urls(raw: &str) -> Result<(), FeedUrlViolation> {
    let sniff = &raw.as_bytes()[..raw.len().min(SNIFF_BYTES)];
    match sniff.iter().find(|byte| matches!(byte, b'<' | b'{')) {
        Some(b'<') => check_xml(raw),
        Some(b'{') => check_json(raw),
        _ => Ok(()),
    }
}

fn check_url(value: &str) -> Result<(), FeedUrlViolation> {
    if url_path_dots(value) > MAX_URL_PATH_DOTS {
        return Err(FeedUrlViolation::TooManyDots);
    }
    Ok(())
}

struct OpenElement {
    is_url_text: bool,
    /// 自身与祖先的 `xml:base` 字节数之和，是 feed-rs 拼接出的基地址长度的上界。
    base_bytes: usize,
}

fn check_xml(raw: &str) -> Result<(), FeedUrlViolation> {
    // 与 feed-rs 的 `ElementSource::new` 相同的配置。
    let mut reader = NsReader::from_reader(raw.as_bytes());
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);

    let mut buf = Vec::new();
    let mut open: Vec<OpenElement> = Vec::new();
    // 与 feed-rs 一样，把相邻的文本、CDATA 与实体引用合成一段。
    let mut text = String::new();

    loop {
        let event = match reader.read_resolved_event_into(&mut buf) {
            Ok((_, event)) => event,
            Err(_) => return Ok(()),
        };
        let decoder = reader.decoder();
        match event {
            Event::Start(ref start) => {
                flush_text(&mut text, &open)?;
                let qualified = start.name();
                let name = decoder.decode(qualified.as_ref()).unwrap_or_default();
                let local = name.split(':').next_back().unwrap_or_default();
                let mut base_bytes = open.last().map_or(0, |parent| parent.base_bytes);
                for attribute in start.attributes().flatten() {
                    let Ok(key) = decoder.decode(attribute.key.as_ref()) else {
                        continue;
                    };
                    let Ok(decoded) = decoder.decode(&attribute.value) else {
                        continue;
                    };
                    let value = unescape(&decoded).unwrap_or(decoded.clone());
                    if key == "xml:base" {
                        check_url(&value)?;
                        base_bytes = base_bytes.saturating_add(value.len());
                        if base_bytes > MAX_XML_BASE_BYTES {
                            return Err(FeedUrlViolation::XmlBaseTooLong);
                        }
                    }
                    let attribute_local = key.split(':').next_back().unwrap_or_default();
                    if URL_ATTRIBUTES.contains(&attribute_local) {
                        check_url(&value)?;
                    }
                }
                open.push(OpenElement {
                    is_url_text: URL_TEXT_ELEMENTS.contains(&local),
                    base_bytes,
                });
            }
            Event::End(_) => {
                flush_text(&mut text, &open)?;
                open.pop();
            }
            Event::Text(ref content) => match decoder.decode(content) {
                Ok(decoded) => text.push_str(&decoded),
                Err(_) => return Ok(()),
            },
            Event::CData(ref content) => match decoder.decode(content) {
                Ok(decoded) => text.push_str(&decoded),
                Err(_) => return Ok(()),
            },
            Event::GeneralRef(ref reference) => match reference.resolve_char_ref() {
                Ok(Some(character)) => text.push(character),
                Ok(None) => {
                    let Ok(name) = decoder.decode(reference) else {
                        return Ok(());
                    };
                    match resolve_predefined_entity(&name) {
                        Some(resolved) => text.push_str(resolved),
                        None => {
                            text.push('&');
                            text.push_str(&name);
                            text.push(';');
                        }
                    }
                }
                Err(_) => return Ok(()),
            },
            Event::Eof => {
                flush_text(&mut text, &open)?;
                return Ok(());
            }
            _ => {}
        }
        buf.clear();
    }
}

/// 一段文本结束：如果它在会被当作地址的元素里，检查它。
fn flush_text(text: &mut String, open: &[OpenElement]) -> Result<(), FeedUrlViolation> {
    if open.last().is_some_and(|element| element.is_url_text) {
        check_url(text)?;
    }
    text.clear();
    Ok(())
}

/// JSON Feed：feed-rs 把 `home_page_url`、`feed_url`、条目的 `url`/`external_url`、附件的 `url`
/// 都交给 `url` 解析。检查所有以 `url` 结尾的键。
fn check_json(raw: &str) -> Result<(), FeedUrlViolation> {
    let Ok(root) = serde_json::from_str::<Value>(raw) else {
        return Ok(());
    };
    let mut pending = vec![&root];
    while let Some(value) = pending.pop() {
        match value {
            Value::Object(map) => {
                for (key, child) in map {
                    if key.ends_with("url")
                        && let Value::String(url) = child
                    {
                        check_url(url)?;
                    }
                    pending.push(child);
                }
            }
            Value::Array(items) => pending.extend(items),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FeedUrlViolation, check_feed_urls};

    fn rss(item_link: &str) -> String {
        format!(
            r#"<?xml version="1.0"?><rss version="2.0"><channel><title>t</title><link>https://e.test/</link><description>Wait... what?</description><item><title>x</title><link>{item_link}</link><description>a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h.i.j.k.l.m.n.o.p.q.r.s.t.u.v.w.x.y.z.a.b.c.d.e.f.g.h</description></item></channel></rss>"#
        )
    }

    #[test]
    fn ordinary_feeds_pass() {
        assert_eq!(check_feed_urls(&rss("https://e.test/posts/1.html")), Ok(()));
        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/blog/"><title>t</title><entry><link href="../posts/1.html"/></entry></feed>"#;
        assert_eq!(check_feed_urls(atom), Ok(()));
        let json = r#"{"version":"https://jsonfeed.org/version/1.1","title":"t","items":[{"id":"1","url":"https://e.test/1"}]}"#;
        assert_eq!(check_feed_urls(json), Ok(()));
        assert_eq!(check_feed_urls("not a feed"), Ok(()));
    }

    #[test]
    fn dot_segments_in_link_text_are_rejected() {
        let link = format!("https://{}@e.test/{}", "a".repeat(64), "../".repeat(64));
        assert_eq!(check_feed_urls(&rss(&link)), Err(FeedUrlViolation::TooManyDots));
        // 字符引用、CDATA 拆分与换行都按 feed-rs 合并后的文本计算。
        let encoded = format!("https://e.test/{}", "&#46;&#46;/".repeat(40));
        assert_eq!(check_feed_urls(&rss(&encoded)), Err(FeedUrlViolation::TooManyDots));
        let split = format!("https://e.test/{}", ".<![CDATA[.]]>/".repeat(40));
        assert_eq!(check_feed_urls(&rss(&split)), Err(FeedUrlViolation::TooManyDots));
        let newlines = format!("https://e.test/{}", "%2\ne%2\ne/".repeat(40));
        assert_eq!(check_feed_urls(&rss(&newlines)), Err(FeedUrlViolation::TooManyDots));
    }

    #[test]
    fn dot_segments_in_url_attributes_are_rejected() {
        let href = "../".repeat(64);
        let atom = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><link href="{href}"/></entry></feed>"#
        );
        assert_eq!(check_feed_urls(&atom), Err(FeedUrlViolation::TooManyDots));
        let enclosure = format!(
            r#"<rss version="2.0"><channel><item><enclosure url="https://e.test/{href}"/></item></channel></rss>"#
        );
        assert_eq!(check_feed_urls(&enclosure), Err(FeedUrlViolation::TooManyDots));
    }

    #[test]
    fn long_or_stacked_xml_base_is_rejected() {
        let long = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/{}/"></feed>"#,
            "a".repeat(600)
        );
        assert_eq!(check_feed_urls(&long), Err(FeedUrlViolation::XmlBaseTooLong));
        let stacked = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom">{}{}</feed>"#,
            r#"<x xml:base="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/">"#.repeat(6),
            "</x>".repeat(6)
        );
        assert_eq!(check_feed_urls(&stacked), Err(FeedUrlViolation::XmlBaseTooLong));
    }

    #[test]
    fn json_feed_url_fields_are_checked() {
        let url = format!("https://e.test/{}", "../".repeat(64));
        let json = format!(r#"{{"version":"1","items":[{{"id":"1","external_url":"{url}"}}]}}"#);
        assert_eq!(check_feed_urls(&json), Err(FeedUrlViolation::TooManyDots));
    }
}
