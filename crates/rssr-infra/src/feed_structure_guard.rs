//! feed-rs 解析之前对 XML feed 的结构预检。
//!
//! 地址里大量 `..` 段造成的平方级解析已经在根上修掉（`vendor/url` 的补丁），这里只挡 feed-rs
//! 与 quick-xml 自身随结构放大的代价。全部是在字节层面数出来的上限：不解码文本，也不复刻
//! feed-rs 对哪些字符串当作地址的判断（那种复刻在独立评审中一轮轮出现分叉，不收敛）。
//!
//! - quick-xml 的命名空间解析对每个元素反向扫描在作用域内的全部绑定，并对每个带前缀的元素
//!   重新解码一遍命名空间的原始值；深度计数是 `u16`，65536 层会溢出。
//! - 重复属性在 quick-xml 里退回线性查找，一个元素上成千上万个重复属性是平方级。
//! - feed-rs 把当前 `xml:base` 克隆进每个元素，并让每个链接与它拼接：1 MB 的基地址配 1000 个
//!   条目约 9 s。
//!
//! 所有计数都按“可能发生”的最大值算（重复的、出现在正文子树里的 `xml:base` 也计），只会多拒绝、
//! 不会漏。越界就拒绝整个 feed；XML 本身写错时不在这里判断，交给 feed-rs。

use std::collections::HashSet;

use feed_quick_xml::events::Event;
use feed_quick_xml::reader::Reader;

/// 元素最大嵌套深度。正常 feed（含 Atom 的 XHTML 内容）只有几十层；quick-xml 的深度计数是
/// `u16`，这里远在溢出之前截住。
const MAX_XML_DEPTH: usize = 1024;

/// 同时在作用域内的命名空间声明数上限：feed-rs 解析每个元素名都要扫一遍这些绑定。
const MAX_NAMESPACE_BINDINGS: usize = 256;

/// 命名空间声明原始值（未反转义）的最大字节数：feed-rs 对每个带前缀的元素都会重新解码它。
const MAX_NAMESPACE_VALUE_BYTES: usize = 256;

/// 一个元素上允许的属性错误（重复、格式不对）个数。重复属性在 quick-xml 里要线性回查，
/// 少量容错保持兼容，多了就是在制造平方级。
const MAX_ATTRIBUTE_ERRORS: usize = 16;

/// 基地址克隆与拼接的总量：每个元素按当前作用域内 `xml:base` 原始字节数之和计（遇到 ASCII
/// scheme 开头的绝对地址从它重新算；路径替换等让真实基地址更短的情况不细分，只会多算），
/// 总和不超过输入长度的这个倍数加上 [`BASE_COPY_SLACK`]。百分号编码与 IDNA 让实际基地址比
/// 原始值最多长一个常数倍（百分号编码每字节至多 3 倍），所以 feed-rs 的真实复制量仍是输入
/// 长度的常数倍。正常 feed 的基地址是站点地址，几十字节，离这个上限很远。
const BASE_COPY_PER_BYTE: u64 = 16;
const BASE_COPY_SLACK: u64 = 1 << 20;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum FeedStructureViolation {
    #[error("feed 的元素嵌套过深（超过 {MAX_XML_DEPTH} 层）")]
    TooDeep,
    #[error("feed 的命名空间声明过多或过长")]
    TooManyNamespaces,
    #[error("feed 中有元素带大量重复或格式错误的属性")]
    BadAttributes,
    #[error("feed 的 xml:base 过长、重复或作用范围过大")]
    XmlBaseTooLarge,
    #[error("feed 声明了与 ASCII 不兼容的编码")]
    IncompatibleEncoding,
}

/// 检查 XML feed 的结构上限；JSON 或其它内容直接放行。
pub(crate) fn check_feed_structure(raw: &str) -> Result<(), FeedStructureViolation> {
    // feed-rs 判断 XML 还是 JSON 时只看第一次读入的缓冲区（`BufReader` 默认 8 KiB）里第一个
    // `<` 或 `{`。
    let sniff = &raw.as_bytes()[..raw.len().min(8 * 1024)];
    match sniff.iter().find(|byte| matches!(byte, b'<' | b'{')) {
        Some(b'<') => check_xml(raw),
        _ => Ok(()),
    }
}

struct OpenElement {
    /// 自身与祖先声明的命名空间数。
    bindings: usize,
    /// feed-rs 在这个元素上使用的基地址长度上界（按原始字节估算）。
    base_bytes: usize,
    /// 最近一个绝对 `xml:base` 原始值的字节数：以 `/` 开头的相对值替换路径、保留它的
    /// scheme 与 authority，长度不超过它加上自身。
    absolute_bytes: usize,
}

/// `xml:base` 原始值是否以 scheme 开头（绝对地址替换上级基地址，不与它拼接）。只认纯 ASCII
/// 的写法；用实体或控制字符拼出来的 scheme 会被当作相对地址累加，只会多算。
fn starts_with_scheme(value: &[u8]) -> bool {
    let Some((first, rest)) = value.split_first() else {
        return false;
    };
    first.is_ascii_alphabetic()
        && rest.iter().position(|byte| *byte == b':').is_some_and(|colon| {
            rest[..colon].iter().all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(byte))
        })
}

fn check_xml(raw: &str) -> Result<(), FeedStructureViolation> {
    // 与 feed-rs 的 `ElementSource::new` 相同的分词配置；不做命名空间解析（那正是要防的路径）。
    let mut reader = Reader::from_reader(raw.as_bytes());
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);

    let base_copy_budget =
        BASE_COPY_SLACK.saturating_add(BASE_COPY_PER_BYTE.saturating_mul(raw.len() as u64));
    let mut base_copies = 0_u64;
    let mut open: Vec<OpenElement> = Vec::new();
    let mut buf = Vec::new();

    loop {
        let position = reader.buffer_position();
        let read = reader.read_event_into(&mut buf);
        // 这里按原始字节找 `xml:base`、`xmlns`；一旦解码器换成 UTF-16 之类与 ASCII 不兼容的
        // 编码（编码声明，或 quick-xml 按开头字节自动识别，后者不产生声明事件），feed-rs 会按
        // 那种编码解码属性名，字节层面就看不见了。正文已经是 UTF-8 字符串，这样的编码本来就
        // 不对，quick-xml 也要求不带转码时遇到它就停止处理。所以每读一次都检查。
        if !reader.decoder().encoding().is_ascii_compatible() {
            return Err(FeedStructureViolation::IncompatibleEncoding);
        }
        let event = match read {
            Ok(event) => event,
            // feed-rs 吞掉部分错误后会继续读，这里也继续；读不动了就交给 feed-rs。
            Err(_) if reader.buffer_position() > position => {
                buf.clear();
                continue;
            }
            Err(_) => return Ok(()),
        };
        match event {
            Event::Start(ref start) => {
                if open.len() >= MAX_XML_DEPTH {
                    return Err(FeedStructureViolation::TooDeep);
                }
                let parent = open.last();
                let mut bindings = parent.map_or(0, |parent| parent.bindings);
                let parent_base_bytes = parent.map_or(0, |parent| parent.base_bytes);
                let mut base_bytes = parent_base_bytes;
                let mut absolute_bytes = parent.map_or(0, |parent| parent.absolute_bytes);
                // 与 NsReader 一样不做重复检查地遍历（它会把重复的命名空间声明都压进绑定表），
                // 重复自己判断。
                let mut seen = HashSet::new();
                let mut attribute_errors = 0_usize;
                let mut has_base = false;
                for attribute in start.attributes().with_checks(false) {
                    let attribute = match attribute {
                        Ok(attribute) if seen.insert(attribute.key.as_ref().to_vec()) => attribute,
                        duplicate_or_error => {
                            attribute_errors += 1;
                            if attribute_errors > MAX_ATTRIBUTE_ERRORS {
                                return Err(FeedStructureViolation::BadAttributes);
                            }
                            match duplicate_or_error {
                                Ok(attribute) => attribute,
                                Err(_) => continue,
                            }
                        }
                    };
                    let key = attribute.key.as_ref();
                    if key == b"xmlns" || key.starts_with(b"xmlns:") {
                        bindings += 1;
                        if bindings > MAX_NAMESPACE_BINDINGS
                            || attribute.value.len() > MAX_NAMESPACE_VALUE_BYTES
                        {
                            return Err(FeedStructureViolation::TooManyNamespaces);
                        }
                    } else if key == b"xml:base" {
                        // feed-rs 只采用第一个（重复的属性被它丢弃）；后面的重复值不能用来
                        // 重新计算，直接拒绝。
                        if has_base {
                            return Err(FeedStructureViolation::XmlBaseTooLarge);
                        }
                        has_base = true;
                        let value = attribute.value.as_ref();
                        base_bytes = if starts_with_scheme(value) {
                            absolute_bytes = value.len();
                            value.len()
                        } else if value.starts_with(b"/") && !value.starts_with(b"//") {
                            absolute_bytes.saturating_add(value.len())
                        } else {
                            base_bytes.saturating_add(value.len())
                        };
                    }
                }
                // 有的处理器（RSS `image` 下的 `url`/`link`）用父元素的基地址解析子元素的文本，
                // 所以按父子两者中较大的计。
                let charged = base_bytes.max(parent_base_bytes);
                base_copies = base_copies.saturating_add(charged as u64);
                if base_copies > base_copy_budget {
                    return Err(FeedStructureViolation::XmlBaseTooLarge);
                }
                open.push(OpenElement { bindings, base_bytes, absolute_bytes });
            }
            Event::End(_) => {
                open.pop();
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
        buf.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{FeedStructureViolation, check_feed_structure};

    #[test]
    fn ordinary_feeds_pass() {
        let rss = r#"<?xml version="1.0"?><rss version="2.0" xmlns:atom="http://www.w3.org/2005/Atom"><channel><title>t</title><link>https://e.test/</link><description>Wait... what?</description><item><title>x</title><link>https://e.test/../1.html</link></item></channel></rss>"#;
        assert_eq!(check_feed_structure(rss), Ok(()));
        let atom = r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/blog/"><title>t</title><entry xml:base="https://e.test/blog/2026/"><link href="../posts/1.html"/></entry></feed>"#;
        assert_eq!(check_feed_structure(atom), Ok(()));
        assert_eq!(check_feed_structure(r#"{"version":"1","items":[]}"#), Ok(()));
        assert_eq!(check_feed_structure("not a feed"), Ok(()));
    }

    #[test]
    fn deep_nesting_is_rejected() {
        let deep = format!("<feed>{}{}</feed>", "<x>".repeat(2_000), "</x>".repeat(2_000));
        assert_eq!(check_feed_structure(&deep), Err(FeedStructureViolation::TooDeep));
    }

    #[test]
    fn namespace_abuse_is_rejected() {
        let many = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom">{}{}</feed>"#,
            r#"<x xmlns:a="urn:a">"#.repeat(300),
            "</x>".repeat(300)
        );
        assert_eq!(check_feed_structure(&many), Err(FeedStructureViolation::TooManyNamespaces));
        // 重复的命名空间声明也会进 feed-rs 的绑定表。
        let repeated = format!(
            "<feed>{}{}</feed>",
            format!("<x{}>", r#" xmlns:x="urn:x""#.repeat(17)).repeat(16),
            "</x>".repeat(16)
        );
        assert_eq!(check_feed_structure(&repeated), Err(FeedStructureViolation::TooManyNamespaces));
        // 原始值按字节计：带前导零的字符引用反转义后很短，但 feed-rs 解码的是原始值。
        let long_raw = format!(
            r#"<rss version="2.0" xmlns:x="urn:&#{}65;"><channel><x:a/></channel></rss>"#,
            "0".repeat(300)
        );
        assert_eq!(check_feed_structure(&long_raw), Err(FeedStructureViolation::TooManyNamespaces));
    }

    #[test]
    fn duplicate_attributes_are_rejected() {
        let duplicates = format!("<feed a=\"\"{}></feed>", r#" a="""#.repeat(1_000));
        assert_eq!(check_feed_structure(&duplicates), Err(FeedStructureViolation::BadAttributes));
    }

    #[test]
    fn large_xml_base_is_rejected() {
        // 每个元素都要克隆一份基地址：基地址不长，但元素极多时总量超限。
        let base = format!("https://e.test/{}/", "a".repeat(200));
        let copies = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="{base}">{}</feed>"#,
            "<a/>".repeat(20_000)
        );
        assert_eq!(check_feed_structure(&copies), Err(FeedStructureViolation::XmlBaseTooLarge));
        // 很长的基地址配很多元素。
        let long = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/{}/">{}</feed>"#,
            "a".repeat(1 << 20),
            "<entry/>".repeat(100)
        );
        assert_eq!(check_feed_structure(&long), Err(FeedStructureViolation::XmlBaseTooLarge));
        // 重复的 xml:base：feed-rs 用第一个，不能拿后面的值重新计算。
        let duplicate = r#"<rss version="2.0" xml:base="https://e.test/"><channel xml:base="a/" xml:base="x:y"></channel></rss>"#;
        assert_eq!(check_feed_structure(duplicate), Err(FeedStructureViolation::XmlBaseTooLarge));
    }

    #[test]
    fn child_bases_do_not_hide_the_parent_base() {
        // RSS `image` 下的 `link` 用父元素 `image` 的基地址解析，子元素自己的 xml:base 不能把计费
        // 降下来。
        let image = format!(
            r#"<rss version="2.0"><channel><image xml:base="https://e.test/{}/"><url xml:base="x:y">https://e.test/logo.png</url>{}</image></channel></rss>"#,
            "a".repeat(1 << 20),
            r#"<link xml:base="x:y">p</link>"#.repeat(20_000)
        );
        assert_eq!(check_feed_structure(&image), Err(FeedStructureViolation::XmlBaseTooLarge));
    }

    #[test]
    fn root_relative_bases_replace_the_path() {
        // 以 `/` 开头的相对基地址替换路径，不随层数累加。
        let base = format!("/{}/", "p".repeat(90));
        let articles: String = (0..200)
            .map(|_| {
                format!(
                    "<entry><content type=\"xhtml\">{}{}{}</content></entry>",
                    format!(r#"<div xml:base="{base}">"#).repeat(12),
                    "<span>word</span>".repeat(50),
                    "</div>".repeat(12)
                )
            })
            .collect();
        let feed = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/">{articles}</feed>"#
        );
        assert_eq!(check_feed_structure(&feed), Ok(()));
    }

    #[test]
    fn ordinary_xml_base_chains_pass() {
        // 正常长度的相对基地址链，即使按原始字节累加也远在复制总量之内。
        let chain = format!(
            r#"<rss version="2.0" xml:base="https://e.test/{}/"><channel xml:base="/{}/"><item xml:base="/{}/"><link>p</link></item></channel></rss>"#,
            "a".repeat(90),
            "b".repeat(90),
            "c".repeat(90)
        );
        assert_eq!(check_feed_structure(&chain), Ok(()));
    }

    #[test]
    fn ascii_incompatible_encoding_declarations_are_rejected() {
        // 文档中途的 UTF-16 声明会让 feed-rs 按 UTF-16 解码属性名，字节层面看不见 `xml:base`。
        let utf16 = |text: &str| -> String {
            text.encode_utf16().flat_map(u16::to_le_bytes).map(char::from).collect()
        };
        let midway = format!(
            r#"<rss version="2.0"><channel><?xml version="1.0" encoding="UTF-16LE"?><{} {}="{}"></channel></rss>"#,
            utf16("item"),
            utf16("xml:base"),
            utf16("https://e.test/")
        );
        assert_eq!(
            check_feed_structure(&midway),
            Err(FeedStructureViolation::IncompatibleEncoding)
        );
        // 没有声明、靠开头字节 `00 3C 00 3F` 自动识别成 UTF-16BE 的也一样。
        let utf16be = |text: &str| -> String {
            text.encode_utf16().flat_map(u16::to_be_bytes).map(char::from).collect()
        };
        let detected = format!(
            "\0<{} {}=\"{}\">{}</{}>",
            utf16be("?:feed"),
            utf16be("xml:base"),
            utf16be("https://e.test/"),
            format!("<{}/>", utf16be("entry")).repeat(10),
            utf16be("?:feed")
        );
        assert_eq!(
            check_feed_structure(&detected),
            Err(FeedStructureViolation::IncompatibleEncoding)
        );
        let declared = r#"<?xml version="1.0" encoding="UTF-16"?><rss version="2.0"></rss>"#;
        assert_eq!(
            check_feed_structure(declared),
            Err(FeedStructureViolation::IncompatibleEncoding)
        );
        let latin1 = r#"<?xml version="1.0" encoding="ISO-8859-1"?><rss version="2.0"></rss>"#;
        assert_eq!(check_feed_structure(latin1), Ok(()));
    }
}
