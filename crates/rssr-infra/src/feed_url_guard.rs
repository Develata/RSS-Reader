//! feed-rs 解析之前对 feed 正文的结构与地址预检。
//!
//! feed-rs 内部有几条可以被构造成超线性的路径，代价都发生在它的解析过程里，解析完再检查已经
//! 来不及：
//!
//! - `url` 处理每个 `..` 段都要从序列化结果末尾反向扫描，根路径上扫过整个 authority；
//!   一个带长用户名和大量 `../` 的链接就是平方级（见 [`rssr_domain::url_guard`]）。
//! - 每个元素都会克隆一份当前 `xml:base`：1 MB 的基地址配 1000 个条目约 9 s。
//! - quick-xml 的命名空间解析对每个元素反向扫描在作用域内的全部绑定；层层声明 `xmlns` 的深层
//!   嵌套是平方级。它的深度计数是 `u16`，65536 层会溢出。
//! - 重复属性在 quick-xml 里退回线性查找，一个元素上成千上万个重复属性是平方级。
//!
//! 所以在解析之前按 feed-rs 自己的读法（同一个 quick-xml、同样的配置、同样的文本合并与实体
//! 解析，但不做命名空间解析，保证预检本身线性）扫一遍：结构越界或会被当作地址的字符串越界，
//! 就拒绝整个 feed。地址只看 feed-rs 会解析成地址的位置，正文里的省略号等不受影响。

use feed_quick_xml::escape::{resolve_predefined_entity, unescape};
use feed_quick_xml::events::Event;
use feed_quick_xml::reader::Reader;
use rssr_domain::{MAX_URL_PATH_DOTS, url_path_dots};
use serde_json::Value;

/// 当前基地址的最大字节数（按百分号编码后的上界估算）。每个元素都会克隆一份当前基地址，
/// 正常 feed 的基地址是站点地址，几十字节。
const MAX_XML_BASE_BYTES: usize = 512;

/// 百分号编码最多把一个字节变成三个（`%XX`），用来估算 `xml:base` 解析后的长度上界。
const PERCENT_ENCODING_GROWTH: usize = 3;

/// 元素最大嵌套深度。正常 feed（含 Atom 的 XHTML 内容）只有几十层；quick-xml 的深度计数是
/// `u16`，这里远在溢出之前截住。
const MAX_XML_DEPTH: usize = 1024;

/// 同时在作用域内的命名空间声明数上限：feed-rs 解析每个元素名都要扫一遍这些绑定。
const MAX_NAMESPACE_BINDINGS: usize = 256;

/// 一个元素上允许的属性错误（重复、格式不对）个数。重复属性在 quick-xml 里要线性回查，
/// 少量容错保持兼容，多了就是在制造平方级。
const MAX_ATTRIBUTE_ERRORS: usize = 16;

/// feed-rs 会把文本内容当作地址解析的元素（RSS 的 `link`/`image/url`，Atom 的 `icon`/`logo`）。
/// 只按本地名判断，比 feed-rs 按命名空间区分得宽，只会多检查。
const URL_TEXT_ELEMENTS: &[&str] = &["link", "icon", "logo", "url"];

/// feed-rs 会当作地址解析的属性（Atom `link@href`、`enclosure@url`、`media:*@url` 等）。
const URL_ATTRIBUTES: &[&str] = &["href", "url", "src", "uri"];

/// feed-rs 判断 XML 还是 JSON 时只看第一次读入的缓冲区（`BufReader` 默认 8 KiB）。
const SNIFF_BYTES: usize = 8 * 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum FeedUrlViolation {
    #[error("feed 中的地址包含过多的点段（超过 {MAX_URL_PATH_DOTS} 个点）")]
    TooManyDots,
    #[error("feed 中的 xml:base 过长（超过 {MAX_XML_BASE_BYTES} 字节）")]
    XmlBaseTooLong,
    #[error("feed 的元素嵌套过深（超过 {MAX_XML_DEPTH} 层）")]
    TooDeep,
    #[error("feed 的命名空间声明过多（同时超过 {MAX_NAMESPACE_BINDINGS} 个）")]
    TooManyNamespaces,
    #[error("feed 中有元素带大量重复或格式错误的属性")]
    BadAttributes,
    #[error("JSON feed 无法解析")]
    InvalidJson,
}

/// 检查 feed 正文的结构与会被 feed-rs 当作地址解析的字符串。XML 本身格式不对时不在这里报错，
/// 交给 feed-rs：它用同样的分词器，会在同一处失败。
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
    /// feed-rs 在这个元素上使用的基地址长度上界。
    base_bytes: usize,
    /// 自身与祖先声明的命名空间绑定数。
    bindings: usize,
}

/// 是否带 scheme（`xml:base` 为绝对地址时 feed-rs 直接替换当前基地址，不与上级拼接）。
fn has_scheme(value: &str) -> bool {
    let trimmed = value.trim_start_matches(|character: char| character <= ' ');
    let mut characters = trimmed.chars();
    characters.next().is_some_and(|first| first.is_ascii_alphabetic())
        && characters
            .take_while(|character| *character != ':')
            .all(|character| character.is_ascii_alphanumeric() || "+-.".contains(character))
        && trimmed.contains(':')
}

fn check_xml(raw: &str) -> Result<(), FeedUrlViolation> {
    // 与 feed-rs 的 `ElementSource::new` 相同的分词配置；不用 `NsReader`，命名空间解析本身
    // 就是要防的平方级路径，这里自己数绑定。
    let mut reader = Reader::from_reader(raw.as_bytes());
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);

    let mut buf = Vec::new();
    let mut open: Vec<OpenElement> = Vec::new();
    // 与 feed-rs 一样，把相邻的文本、CDATA 与实体引用合成一段。
    let mut text = String::new();

    loop {
        let event = match reader.read_event_into(&mut buf) {
            Ok(event) => event,
            Err(_) => return Ok(()),
        };
        let decoder = reader.decoder();
        match event {
            Event::Start(ref start) => {
                flush_text(&mut text, &open)?;
                if open.len() >= MAX_XML_DEPTH {
                    return Err(FeedUrlViolation::TooDeep);
                }
                let qualified = start.name();
                let name = decoder.decode(qualified.as_ref()).unwrap_or_default();
                let local = name.split(':').next_back().unwrap_or_default();
                let parent = open.last();
                let mut base_bytes = parent.map_or(0, |parent| parent.base_bytes);
                let mut bindings = parent.map_or(0, |parent| parent.bindings);
                let mut attribute_errors = 0_usize;
                for attribute in start.attributes() {
                    let Ok(attribute) = attribute else {
                        attribute_errors += 1;
                        if attribute_errors > MAX_ATTRIBUTE_ERRORS {
                            return Err(FeedUrlViolation::BadAttributes);
                        }
                        continue;
                    };
                    let Ok(key) = decoder.decode(attribute.key.as_ref()) else {
                        continue;
                    };
                    if key == "xmlns" || key.starts_with("xmlns:") {
                        bindings += 1;
                        if bindings > MAX_NAMESPACE_BINDINGS {
                            return Err(FeedUrlViolation::TooManyNamespaces);
                        }
                    }
                    let Ok(decoded) = decoder.decode(&attribute.value) else {
                        continue;
                    };
                    let value = unescape(&decoded).unwrap_or(decoded.clone());
                    if key == "xml:base" {
                        check_url(&value)?;
                        let own = value.len().saturating_mul(PERCENT_ENCODING_GROWTH);
                        base_bytes =
                            if has_scheme(&value) { own } else { base_bytes.saturating_add(own) };
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
                    bindings,
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

/// JSON Feed：只检查 feed-rs 会交给 `url` 解析的字段——顶层 `home_page_url`、`feed_url`，
/// 条目的 `url`、`external_url`，附件的 `url`。无法整体解析的 JSON（包括嵌套超过 serde_json
/// 默认深度的）直接拒绝：feed-rs 对未知字段的跳过不受这个深度限制，放行就等于不检查。
fn check_json(raw: &str) -> Result<(), FeedUrlViolation> {
    let root = serde_json::from_str::<Value>(raw).map_err(|_| FeedUrlViolation::InvalidJson)?;
    let field = |object: &Value, key: &str| -> Result<(), FeedUrlViolation> {
        match object.get(key) {
            Some(Value::String(url)) => check_url(url),
            _ => Ok(()),
        }
    };
    field(&root, "home_page_url")?;
    field(&root, "feed_url")?;
    for item in root.get("items").and_then(Value::as_array).into_iter().flatten() {
        field(item, "url")?;
        field(item, "external_url")?;
        for attachment in item.get("attachments").and_then(Value::as_array).into_iter().flatten() {
            field(attachment, "url")?;
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
        let json = r#"{"version":"https://jsonfeed.org/version/1.1","title":"t","items":[{"id":"1","url":"https://e.test/1"}],"_diagnostics":{"curl":"Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait... Wait..."}}"#;
        assert_eq!(check_feed_urls(json), Ok(()));
        assert_eq!(check_feed_urls("not a feed"), Ok(()));
        // 同一个绝对 xml:base 在多层重复声明，不累加。
        let base = format!("https://e.test/{}/", "a".repeat(100));
        let nested = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="{base}"><entry xml:base="{base}"><link xml:base="{base}" href="x"/></entry></feed>"#
        );
        assert_eq!(check_feed_urls(&nested), Ok(()));
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
    fn long_stacked_or_expanding_xml_base_is_rejected() {
        let long = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/{}/"></feed>"#,
            "a".repeat(600)
        );
        assert_eq!(check_feed_urls(&long), Err(FeedUrlViolation::XmlBaseTooLong));
        let stacked = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom">{}{}</feed>"#,
            r#"<x xml:base="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/">"#
                .repeat(6),
            "</x>".repeat(6)
        );
        assert_eq!(check_feed_urls(&stacked), Err(FeedUrlViolation::XmlBaseTooLong));
        // 非 ASCII 字符经百分号编码会变长。
        let expanding = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/{}/"></feed>"#,
            "é".repeat(100)
        );
        assert_eq!(check_feed_urls(&expanding), Err(FeedUrlViolation::XmlBaseTooLong));
    }

    #[test]
    fn structural_abuse_is_rejected() {
        let deep = format!("<feed>{}{}</feed>", "<x>".repeat(2_000), "</x>".repeat(2_000));
        assert_eq!(check_feed_urls(&deep), Err(FeedUrlViolation::TooDeep));
        let namespaces = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom">{}{}</feed>"#,
            r#"<x xmlns:a="urn:a">"#.repeat(300),
            "</x>".repeat(300)
        );
        assert_eq!(check_feed_urls(&namespaces), Err(FeedUrlViolation::TooManyNamespaces));
        let duplicates = format!("<feed a=\"\"{}></feed>", r#" a="""#.repeat(1_000));
        assert_eq!(check_feed_urls(&duplicates), Err(FeedUrlViolation::BadAttributes));
    }

    #[test]
    fn json_feed_url_fields_are_checked() {
        let url = format!("https://e.test/{}", "../".repeat(64));
        let json = format!(r#"{{"version":"1","items":[{{"id":"1","external_url":"{url}"}}]}}"#);
        assert_eq!(check_feed_urls(&json), Err(FeedUrlViolation::TooManyDots));
        let attachment = format!(
            r#"{{"version":"1","items":[{{"id":"1","attachments":[{{"url":"{url}"}}]}}]}}"#
        );
        assert_eq!(check_feed_urls(&attachment), Err(FeedUrlViolation::TooManyDots));
        // 深层扩展字段让通用 JSON 解析失败，但 feed-rs 仍会读到后面的地址，必须拒绝。
        let deep = format!(
            r#"{{"version":"1","_ext":{{"data":{}0{}}},"items":[{{"id":"1","url":"{url}"}}]}}"#,
            "[".repeat(200),
            "]".repeat(200)
        );
        assert_eq!(check_feed_urls(&deep), Err(FeedUrlViolation::InvalidJson));
    }
}
