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

/// 属性不规整的标签（见 [`check_xml`] 里的说明）按整个标签长度的平方计费，总和不超过输入长度
/// 的这个倍数加上 [`BUDGET_SLACK`]。
const IRREGULAR_TAG_COST_PER_BYTE: u64 = 16;

/// 基地址克隆与拼接的总量上限：总和不超过输入长度的这个倍数加上 [`BUDGET_SLACK`]。
///
/// 每个元素的计费是它自己和所有打开的祖先上全部 `xml:base` 原始值的字节数之和（属性不规整、
/// 又含 `xml:base` 字节的标签按整个标签长度计），从不因为绝对地址、`/` 开头的路径替换或值解码
/// 失败而重置。这样它只依赖“feed-rs 用的基地址只能由这些值逐层得出”，不复刻 feed-rs 与 `url`
/// 怎样解析、丢弃或替换它们：绝对地址的长度不超过自身，相对地址拼接后不超过上级加自身，丢弃时
/// 沿用上级，都被这个和覆盖（RSS `image` 下的子元素用父元素的基地址，父元素的和也不超过子元素
/// 的）。解码成 UTF-8、百分号编码与 IDNA 只让真实长度变长一个常数倍，所以 feed-rs 的真实复制量
/// 仍是输入长度的常数倍。
///
/// 这个和会多算：几层都写同一个绝对地址时按几倍计，Atom XHTML 正文里的元素 feed-rs 并不逐个
/// 克隆基地址也照样计。倍数取 64，让 feed、entry、content 三层各带一个约 200 字节的绝对地址、
/// 正文是几千个单元格的表格这类内容仍能通过。
const BASE_COPY_PER_BYTE: u64 = 64;
const BUDGET_SLACK: u64 = 1 << 20;

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
    /// 自身与祖先上全部 `xml:base` 原始值的字节数之和，见 [`BASE_COPY_PER_BYTE`]。
    base_bytes: usize,
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

fn check_xml(raw: &str) -> Result<(), FeedStructureViolation> {
    // 与 feed-rs 的 `ElementSource::new` 相同的分词配置；不做命名空间解析（那正是要防的路径）。
    let mut reader = Reader::from_reader(raw.as_bytes());
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);

    let budget =
        |per_byte: u64| BUDGET_SLACK.saturating_add(per_byte.saturating_mul(raw.len() as u64));
    let base_copy_budget = budget(BASE_COPY_PER_BYTE);
    let irregular_budget = budget(IRREGULAR_TAG_COST_PER_BYTE);
    let mut base_copies = 0_u64;
    let mut irregular_cost = 0_u64;
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
                let mut base_bytes = parent.map_or(0, |parent| parent.base_bytes);
                // 与 NsReader 一样不做重复检查地遍历（它会把重复的命名空间声明都压进绑定表）。
                // 没有错误、也没有重复键时，feed-rs 带重复检查的遍历每一步都通过检查，得到的
                // 属性与这里逐个相同。一旦出现错误或重复，两者出错后的恢复位置可能不同，不去
                // 对齐，按“不规整”整体计费。
                let mut seen = HashSet::new();
                let mut irregular = false;
                for attribute in start.attributes().with_checks(false) {
                    let Ok(attribute) = attribute else {
                        irregular = true;
                        continue;
                    };
                    irregular |= !seen.insert(attribute.key.as_ref().to_vec());
                    let key = attribute.key.as_ref();
                    if key == b"xmlns" || key.starts_with(b"xmlns:") {
                        bindings += 1;
                        if bindings > MAX_NAMESPACE_BINDINGS
                            || attribute.value.len() > MAX_NAMESPACE_VALUE_BYTES
                        {
                            return Err(FeedStructureViolation::TooManyNamespaces);
                        }
                    } else if key == b"xml:base" {
                        base_bytes = base_bytes.saturating_add(attribute.value.len());
                    }
                }
                if irregular {
                    // feed-rs 在这个标签上切出的属性名与出错次数都不超过标签长度，重复检查的
                    // 线性回查合计不超过长度的平方。
                    let tag = start.len() as u64;
                    irregular_cost = irregular_cost.saturating_add(tag.saturating_mul(tag));
                    if irregular_cost > irregular_budget {
                        return Err(FeedStructureViolation::BadAttributes);
                    }
                    // 它可能切出这里没看到的 `xml:base`：只要标签里有这几个字节，就按整个标签计。
                    if contains(start, b"xml:base") {
                        base_bytes = base_bytes.saturating_add(start.len());
                    }
                }
                base_copies = base_copies.saturating_add(base_bytes as u64);
                if base_copies > base_copy_budget {
                    return Err(FeedStructureViolation::XmlBaseTooLarge);
                }
                open.push(OpenElement { bindings, base_bytes });
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
    fn irregular_attributes_are_bounded() {
        let duplicates = format!("<feed a=\"\"{}></feed>", r#" a="""#.repeat(1_000));
        assert_eq!(check_feed_structure(&duplicates), Err(FeedStructureViolation::BadAttributes));
        // 不带检查的遍历只看到一次重复（第二个 `a` 的值很长）；feed-rs 带检查遍历时从重复处
        // 恢复到引号里面，再切出上千个重复的 `b`，每个都线性回查前面上千个属性。
        let recovery = format!(
            r#"<rss version="2.0"{} a="" a="{} "><channel/></rss>"#,
            (0..2_000).map(|i| format!(r#" x{i}="""#)).collect::<String>(),
            " b='0'".repeat(2_000)
        );
        assert_eq!(check_feed_structure(&recovery), Err(FeedStructureViolation::BadAttributes));
        // 少量重复或写错的属性仍然容忍。
        let sloppy = r#"<rss version="2.0" version="2.0"><channel><item><title>t</title><enclosure url="https://e.test/a.mp3" length=1 type="audio/mpeg"/></item></channel></rss>"#;
        assert_eq!(check_feed_structure(sloppy), Ok(()));
    }

    #[test]
    fn hidden_xml_base_in_irregular_tags_is_charged() {
        // 不带检查的遍历只看到两个 `a`；feed-rs 从重复处恢复后切出一个有效的 `xml:base`，
        // 下面每个元素都克隆它。
        let hidden = format!(
            r#"<rss version="2.0" a="" a=" xml:base='https://e.test/{}/' "><channel>{}</channel></rss>"#,
            "a".repeat(900),
            "<a/>".repeat(20_000)
        );
        assert_eq!(check_feed_structure(&hidden), Err(FeedStructureViolation::XmlBaseTooLarge));
    }

    #[test]
    fn large_xml_base_is_rejected() {
        // 每个元素都要克隆一份基地址：基地址不长，但元素极多时总量超限。
        let base = format!("https://e.test/{}/", "a".repeat(400));
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
    }

    /// 根元素上一个长基地址，`channel` 写成 `channel_tag`，其下 2 万个链接。子元素上的值无论
    /// 是绝对地址、`/` 开头、重复还是解码失败，都不能把计费降下来。
    fn long_base_under(channel_tag: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="Shift_JIS"?><rss version="2.0" xml:base="https://e.test/{}/">{channel_tag}<image xml:base="/">{}</image></channel></rss>"#,
            "a".repeat(1 << 16),
            "<link>p</link>".repeat(20_000)
        )
    }

    #[test]
    fn child_bases_never_lower_the_charge() {
        let cases = [
            // RSS `image` 下的 `link` 用父元素的基地址；子元素自己的绝对地址不能把计费降下来。
            r#"<channel xml:base="x:y">"#,
            // `//` 换了 authority，后面 `/` 开头的值保留的是新的 authority。
            r#"<channel xml:base="//b@e.test/">"#,
            // 第一个值没加引号被丢掉，feed-rs 也把第二个当作重复丢掉，继续用上级的基地址。
            r#"<channel xml:base=x xml:base="x:y">"#,
            // 按 Shift_JIS 解码失败的值被 feed-rs 丢掉，继续用上级的基地址。
            r#"<channel xml:base="x:Ж">"#,
        ];
        for channel in cases {
            assert_eq!(
                check_feed_structure(&long_base_under(channel)),
                Err(FeedStructureViolation::XmlBaseTooLarge),
                "{channel}"
            );
        }
        // 对照：同样的结构，根元素上的基地址很短。
        let short =
            long_base_under(r#"<channel xml:base="x:y">"#).replace(&"a".repeat(1 << 16), "a");
        assert_eq!(check_feed_structure(&short), Ok(()));
    }

    #[test]
    fn ordinary_xml_base_chains_pass() {
        // 正常长度的相对基地址链：即使逐层累加、从不重置，也远在复制总量之内。
        let chain = format!(
            r#"<rss version="2.0" xml:base="https://e.test/{}/"><channel xml:base="/{}/"><item xml:base="https://e.test/{}/"><link>p</link></item></channel></rss>"#,
            "a".repeat(90),
            "b".repeat(90),
            "c".repeat(90)
        );
        assert_eq!(check_feed_structure(&chain), Ok(()));
        // Atom 的 feed、entry、content 各带一个基地址，XHTML 正文由大量很小的元素组成。
        let articles: String = (0..2_000)
            .map(|i| {
                format!(
                    r#"<entry xml:base="https://e.test/2026/10/{i}/"><content type="xhtml" xml:base="/2026/10/{i}/assets/"><div>{}</div></content></entry>"#,
                    "<span>word</span><br/>".repeat(50),
                )
            })
            .collect();
        let feed = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="https://e.test/">{articles}</feed>"#
        );
        assert_eq!(check_feed_structure(&feed), Ok(()));
        // feed、entry、content 三层写同一个约 200 字节的绝对地址，正文是 4000 个单元格的表格。
        let base = format!("https://e.test/{}", "p".repeat(186));
        let rows = format!("<tr>{}</tr>", "<td>1</td>".repeat(20)).repeat(200);
        let table = format!(
            r#"<feed xmlns="http://www.w3.org/2005/Atom" xml:base="{base}"><id>f</id><title>t</title><updated>2026-10-21T00:00:00Z</updated><author><name>a</name></author><entry xml:base="{base}"><id>e</id><title>x</title><updated>2026-10-21T00:00:00Z</updated><content type="xhtml" xml:base="{base}"><div xmlns="http://www.w3.org/1999/xhtml"><table>{rows}</table></div></content></entry></feed>"#
        );
        assert_eq!(check_feed_structure(&table), Ok(()));
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
