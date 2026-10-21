//! feed-rs 解析之前对 feed 正文的结构与地址预检。
//!
//! feed-rs 内部有几条可以被构造成超线性的路径，代价都发生在它的解析过程里，解析完再检查已经
//! 来不及：
//!
//! - `url` 处理每个 `..` 段都要从序列化结果末尾反向扫描，根路径上扫过整个 authority；
//!   一个带长用户名和大量 `../` 的链接就是平方级（见 [`rssr_domain::url_guard`]）。
//! - 每个元素都会克隆一份当前 `xml:base`：1 MB 的基地址配 1000 个条目约 9 s。
//! - quick-xml 的命名空间解析对每个元素反向扫描在作用域内的全部绑定，并对每个带前缀的元素
//!   重新解码一遍命名空间值；层层声明 `xmlns` 的深层嵌套或很长的命名空间值都是超线性的。
//!   它的深度计数是 `u16`，65536 层会溢出。
//! - 重复属性在 quick-xml 里退回线性查找，一个元素上成千上万个重复属性是平方级。
//!
//! 所以在解析之前按 feed-rs 的读法（同一个 quick-xml、同样的分词配置、同样的文本合并与实体
//! 解析）扫一遍，结构越界或会被当作地址的字符串越界就拒绝整个 feed。预检自己不做命名空间
//! 解析（那正是要防的路径），自己维护绑定。在读法可能与 feed-rs 分叉的地方（文档中途的 XML
//! 声明会切换解码器，解码失败）一律拒绝，而不是放行。地址只看 feed-rs 会解析成地址的位置，
//! 正文里的省略号等不受影响。

use std::collections::HashSet;

use feed_quick_xml::escape::{resolve_predefined_entity, unescape};
use feed_quick_xml::events::Event;
use feed_quick_xml::reader::Reader;
use rssr_domain::{MAX_URL_PATH_DOTS, url_path_dots};
use serde_json::Value;
use url::Url;

/// 当前基地址（feed-rs 按 `xml:base` 解析、拼接出的地址）序列化后的最大字节数。每个元素都会
/// 克隆一份当前基地址，正常 feed 的基地址是站点地址，几十字节。
const MAX_XML_BASE_BYTES: usize = 512;

/// 元素最大嵌套深度。正常 feed（含 Atom 的 XHTML 内容）只有几十层；quick-xml 的深度计数是
/// `u16`，这里远在溢出之前截住。
const MAX_XML_DEPTH: usize = 1024;

/// 同时在作用域内的命名空间声明数上限：feed-rs 解析每个元素名都要扫一遍这些绑定。
const MAX_NAMESPACE_BINDINGS: usize = 256;

/// 命名空间值的最大字节数：feed-rs 对每个带前缀的元素都会重新解码一遍它。
const MAX_NAMESPACE_URI_BYTES: usize = 256;

/// 一个元素上允许的属性错误（重复、格式不对）个数。重复属性在 quick-xml 里要线性回查，
/// 少量容错保持兼容，多了就是在制造平方级。
const MAX_ATTRIBUTE_ERRORS: usize = 16;

/// feed-rs 会把文本内容当作地址解析的元素（RSS 的 `link`/`image/url`，Atom 的
/// `link`/`icon`/`logo`）。只在元素没有前缀、或前缀绑定到 RSS/Atom 命名空间时检查。
const URL_TEXT_ELEMENTS: &[&str] = &["link", "icon", "logo", "url"];

/// 带前缀时仍需检查的命名空间：RSS 1.0 与 Atom（RSS 2.0 里的 `atom:link`）。
const URL_TEXT_NAMESPACES: &[&str] = &[
    "http://www.w3.org/2005/Atom",
    "http://purl.org/rss/1.0/",
    "http://backend.userland.com/rss2",
];

/// feed-rs 会当作地址解析的属性（Atom `link@href`、`enclosure@url`、`media:*@url` 等）。
/// feed-rs 按完整属性名匹配，只看不带前缀的。
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
    #[error("feed 的命名空间声明过多或过长")]
    TooManyNamespaces,
    #[error("feed 中有元素带大量重复或格式错误的属性")]
    BadAttributes,
    #[error("feed 在文档中途切换了编码声明，或含有无法解码的文本")]
    InconsistentEncoding,
    #[error("JSON feed 无法解析或结构不符")]
    InvalidJson,
}

/// 检查 feed 正文的结构与会被 feed-rs 当作地址解析的字符串。XML 分词本身出错时不在这里报错，
/// 交给 feed-rs：它用同一个分词器，会在同一处失败。
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
    /// 这个元素声明的命名空间绑定数（结束时从绑定表里弹出）。
    declared: usize,
    /// 这个元素是否压入了新的基地址。
    pushed_base: bool,
}

/// 预检维护的状态：与 feed-rs 的命名空间绑定、`xml:base` 栈对应。
#[derive(Default)]
struct XmlState {
    open: Vec<OpenElement>,
    /// 在作用域内的 (前缀, 命名空间) 绑定，按声明顺序。
    bindings: Vec<(String, String)>,
    /// 与 feed-rs 的 `base_uris` 相同语义的基地址栈。
    bases: Vec<Url>,
}

impl XmlState {
    fn namespace_of(&self, prefix: &str) -> Option<&str> {
        self.bindings.iter().rev().find(|(bound, _)| bound == prefix).map(|(_, uri)| uri.as_str())
    }

    /// 与 feed-rs 的 `xml_base_push` 相同：绝对地址直接压栈，相对地址与当前基地址拼接，
    /// 其它解析错误让 feed-rs 整体失败（这里同样停止检查）。预检通过后才交给 `url`，
    /// 所以这里的解析本身有界。
    fn push_base(&mut self, value: &str) -> Result<Option<bool>, FeedUrlViolation> {
        check_url(value)?;
        if value.len() > MAX_XML_BASE_BYTES {
            return Err(FeedUrlViolation::XmlBaseTooLong);
        }
        let resolved = match Url::parse(value) {
            Ok(url) => url,
            Err(url::ParseError::RelativeUrlWithoutBase) => match self.bases.last() {
                Some(base) => match base.join(value) {
                    Ok(url) => url,
                    Err(_) => return Ok(Some(false)),
                },
                None => return Ok(Some(false)),
            },
            Err(_) => return Ok(None),
        };
        // feed-rs 还会把基地址本身当链接再解析一遍（RSS `content:encoded`）。
        if resolved.as_str().len() > MAX_XML_BASE_BYTES {
            return Err(FeedUrlViolation::XmlBaseTooLong);
        }
        check_url(resolved.as_str())?;
        self.bases.push(resolved);
        Ok(Some(true))
    }
}

fn check_xml(raw: &str) -> Result<(), FeedUrlViolation> {
    // 与 feed-rs 的 `ElementSource::new` 相同的分词配置；不用 `NsReader`，命名空间解析本身
    // 就是要防的平方级路径，这里自己维护绑定。
    let mut reader = Reader::from_reader(raw.as_bytes());
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);

    let mut buf = Vec::new();
    let mut state = XmlState::default();
    let mut root_started = false;
    // 与 feed-rs 一样，把相邻的文本、CDATA 与实体引用合成一段。
    let mut text = String::new();

    loop {
        let event = match reader.read_event_into(&mut buf) {
            Ok(event) => event,
            Err(_) => return Ok(()),
        };
        let decoder = reader.decoder();
        match event {
            // feed-rs 在一轮读取开始时取一次解码器；文档中途的声明会让两边的解码分叉。
            Event::Decl(_) if root_started => return Err(FeedUrlViolation::InconsistentEncoding),
            Event::Start(ref start) => {
                root_started = true;
                flush_text(&mut text, &state.open)?;
                if state.open.len() >= MAX_XML_DEPTH {
                    return Err(FeedUrlViolation::TooDeep);
                }
                let qualified = start.name();
                let name = decoder
                    .decode(qualified.as_ref())
                    .map_err(|_| FeedUrlViolation::InconsistentEncoding)?;
                let (prefix, local) = name.split_once(':').unwrap_or(("", &name));

                // 与 NsReader 一样不做重复检查地遍历属性，自己找重复（NsReader 会把重复的
                // 命名空间声明都压进绑定表）。
                let mut seen = HashSet::new();
                let mut attribute_errors = 0_usize;
                let mut declared = 0_usize;
                let mut pushed_base = false;
                for attribute in start.attributes().with_checks(false) {
                    let Ok(attribute) = attribute else {
                        attribute_errors += 1;
                        if attribute_errors > MAX_ATTRIBUTE_ERRORS {
                            return Err(FeedUrlViolation::BadAttributes);
                        }
                        continue;
                    };
                    if !seen.insert(attribute.key.as_ref().to_vec()) {
                        attribute_errors += 1;
                        if attribute_errors > MAX_ATTRIBUTE_ERRORS {
                            return Err(FeedUrlViolation::BadAttributes);
                        }
                    }
                    let key = decoder
                        .decode(attribute.key.as_ref())
                        .map_err(|_| FeedUrlViolation::InconsistentEncoding)?;
                    let decoded = decoder
                        .decode(&attribute.value)
                        .map_err(|_| FeedUrlViolation::InconsistentEncoding)?;
                    let value = unescape(&decoded).unwrap_or(decoded.clone());

                    if let Some(bound) = key.strip_prefix("xmlns") {
                        if !(bound.is_empty() || bound.starts_with(':')) {
                            continue;
                        }
                        if value.len() > MAX_NAMESPACE_URI_BYTES
                            || state.bindings.len() >= MAX_NAMESPACE_BINDINGS
                        {
                            return Err(FeedUrlViolation::TooManyNamespaces);
                        }
                        let bound = bound.strip_prefix(':').unwrap_or_default();
                        state.bindings.push((bound.to_string(), value.into_owned()));
                        declared += 1;
                        continue;
                    }
                    if key == "xml:base" && !pushed_base {
                        match state.push_base(&value)? {
                            Some(pushed) => pushed_base = pushed,
                            None => return Ok(()),
                        }
                        continue;
                    }
                    if URL_ATTRIBUTES.contains(&key.as_ref()) {
                        check_url(&value)?;
                    }
                }

                let is_url_text = URL_TEXT_ELEMENTS.contains(&local)
                    && (prefix.is_empty()
                        || state
                            .namespace_of(prefix)
                            .is_some_and(|namespace| URL_TEXT_NAMESPACES.contains(&namespace)));
                state.open.push(OpenElement { is_url_text, declared, pushed_base });
            }
            Event::End(_) => {
                flush_text(&mut text, &state.open)?;
                if let Some(element) = state.open.pop() {
                    let kept = state.bindings.len().saturating_sub(element.declared);
                    state.bindings.truncate(kept);
                    if element.pushed_base {
                        state.bases.pop();
                    }
                }
            }
            Event::Text(ref content) => text.push_str(
                &decoder.decode(content).map_err(|_| FeedUrlViolation::InconsistentEncoding)?,
            ),
            Event::CData(ref content) => text.push_str(
                &decoder.decode(content).map_err(|_| FeedUrlViolation::InconsistentEncoding)?,
            ),
            Event::GeneralRef(ref reference) => match reference.resolve_char_ref() {
                Ok(Some(character)) => text.push(character),
                Ok(None) => {
                    let name = decoder
                        .decode(reference)
                        .map_err(|_| FeedUrlViolation::InconsistentEncoding)?;
                    match resolve_predefined_entity(&name) {
                        Some(resolved) => text.push_str(resolved),
                        None => {
                            text.push('&');
                            text.push_str(&name);
                            text.push(';');
                        }
                    }
                }
                // feed-rs 在同一处报错。
                Err(_) => return Ok(()),
            },
            Event::Eof => {
                flush_text(&mut text, &state.open)?;
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
/// 条目的 `url`、`external_url`，附件的 `url`。
///
/// 无法整体解析的 JSON（包括嵌套超过 serde_json 默认深度的）直接拒绝：feed-rs 对未知字段的
/// 跳过不受这个深度限制，放行就等于不检查。根、条目、附件必须是对象：serde 也接受按字段顺序
/// 写成数组的结构体，那样字段名就不出现了。
fn check_json(raw: &str) -> Result<(), FeedUrlViolation> {
    let root = serde_json::from_str::<Value>(raw).map_err(|_| FeedUrlViolation::InvalidJson)?;
    json_object(&root)?;
    json_url_field(&root, "home_page_url")?;
    json_url_field(&root, "feed_url")?;
    for item in json_array(root.get("items"))? {
        json_object(item)?;
        json_url_field(item, "url")?;
        json_url_field(item, "external_url")?;
        for attachment in json_array(item.get("attachments"))? {
            json_object(attachment)?;
            json_url_field(attachment, "url")?;
        }
    }
    Ok(())
}

fn json_object(value: &Value) -> Result<(), FeedUrlViolation> {
    if value.is_object() { Ok(()) } else { Err(FeedUrlViolation::InvalidJson) }
}

fn json_url_field(value: &Value, key: &str) -> Result<(), FeedUrlViolation> {
    match value.get(key) {
        Some(Value::String(url)) => check_url(url),
        _ => Ok(()),
    }
}

fn json_array(value: Option<&Value>) -> Result<&[Value], FeedUrlViolation> {
    match value {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(items)) => Ok(items),
        Some(_) => Err(FeedUrlViolation::InvalidJson),
    }
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
        // 相对 xml:base 逐层拼接，拼出来的基地址越来越长。
        let stacked = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/">{}{}</feed>"#,
            r#"<x xml:base="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/">"#
                .repeat(9),
            "</x>".repeat(9)
        );
        assert_eq!(check_feed_urls(&stacked), Err(FeedUrlViolation::XmlBaseTooLong));
        // 没有绝对基地址时，相对 xml:base 被 feed-rs 忽略，不拒绝。
        let ignored = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom">{}{}</feed>"#,
            r#"<x xml:base="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/">"#
                .repeat(9),
            "</x>".repeat(9)
        );
        assert_eq!(check_feed_urls(&ignored), Ok(()));
        // 各自不超过点数的绝对与相对基地址，拼起来也要检查。
        let dotted = format!(
            r#"<rss version="2.0" xml:base="http://e.test/{}/"><channel><item xml:base="{}/"></item></channel></rss>"#,
            "a.".repeat(30),
            "b.".repeat(40)
        );
        assert_eq!(check_feed_urls(&dotted), Err(FeedUrlViolation::TooManyDots));
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
    fn decoder_divergence_and_namespace_abuse_are_rejected() {
        // 文档中途的 XML 声明会切换预检的解码器而不切换 feed-rs 的。
        let url = format!("https://e.test/{}", "../".repeat(40));
        let midway = format!(
            r#"<rss version="2.0"><channel><link><?xml version="1.0" encoding="UTF-16LE"?>{url}</link></channel></rss>"#
        );
        assert_eq!(check_feed_urls(&midway), Err(FeedUrlViolation::InconsistentEncoding));
        // feed-rs 对每个带前缀的元素都会重新解码命名空间值。
        let long_namespace = format!(
            r#"<rss version="2.0"><channel xmlns:x="urn:{}"><x:a/></channel></rss>"#,
            "a".repeat(300)
        );
        assert_eq!(check_feed_urls(&long_namespace), Err(FeedUrlViolation::TooManyNamespaces));
        // 重复的命名空间声明也会进 feed-rs 的绑定表，照样计数。
        let repeated = format!(
            "<feed>{}{}</feed>",
            format!("<x{}>", r#" xmlns:x="urn:x""#.repeat(17)).repeat(16),
            "</x>".repeat(16)
        );
        assert_eq!(check_feed_urls(&repeated), Err(FeedUrlViolation::TooManyNamespaces));
        // 未知命名空间里叫 link 的元素不会被 feed-rs 当作地址。
        let prose = format!(
            r#"<rss version="2.0"><channel><x:link xmlns:x="urn:example:prose">{}</x:link></channel></rss>"#,
            "Wait... ".repeat(30)
        );
        assert_eq!(check_feed_urls(&prose), Ok(()));
        // Atom 命名空间前缀下的 link 文本仍然检查。
        let atom_prefixed = format!(
            r#"<rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom"><channel><atom:link>{url}</atom:link></channel></rss>"#
        );
        assert_eq!(check_feed_urls(&atom_prefixed), Err(FeedUrlViolation::TooManyDots));
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
        // serde 接受按字段顺序写成数组的结构体，字段名不出现；根、条目、附件必须是对象。
        let positional = format!(
            r#"{{"version":"1","title":"t","items":[{{"id":"1","attachments":[["{url}","audio/mpeg",null,null]]}}]}}"#
        );
        assert_eq!(check_feed_urls(&positional), Err(FeedUrlViolation::InvalidJson));
        // 根是数组时，标题字符串里的 `{` 也会让 feed-rs 走 JSON 分支。
        let array_root = format!(r#"["{{t","{url}",null,null,null,null,null,null,null,[]]"#);
        assert_eq!(check_feed_urls(&array_root), Err(FeedUrlViolation::InvalidJson));
    }
}
