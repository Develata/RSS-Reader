//! Adapted from feat/reader-mathml 73f8d6e; supplements, never replaces, the v0.2 model budget.
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

use quick_xml::events::Event;
use quick_xml::reader::Reader;

/// 元素最大嵌套深度。正常 feed（含 Atom 的 XHTML 内容）只有几十层；quick-xml 的深度计数是
/// `u16`，这里远在溢出之前截住。
const MAX_XML_DEPTH: usize = super::MAX_XML_DEPTH;

/// 同时在作用域内的命名空间绑定数上限（含 NsReader 预置的 `xml`、`xmlns` 两个）：feed-rs 解析
/// 每个元素名都要扫一遍这些绑定。
const MAX_NAMESPACE_BINDINGS: usize = 256;

/// 命名空间声明原始值（未反转义）的最大字节数：feed-rs 对每个带前缀的元素都会重新解码它。
const MAX_NAMESPACE_VALUE_BYTES: usize = 256;

/// 每个标签的属性判重代价，总和不超过输入长度的这个倍数加上 [`BUDGET_SLACK`]。
///
/// 规整的标签（见 [`check_xml`] 里的说明）有 n 个属性、属性名共 K 字节时按 n·(n + K) 计：
/// quick-xml 判重用的哈希表种子固定，攻击者可以挑出哈希低位相同的属性名，让每次插入都扫过
/// 同一串满桶（合计不超过 n²）；完整哈希相同时还要与前面每个属性名逐字节比较（每次不超过
/// K）。两项都与哈希种子无关。不规整的标签按整个标签长度的平方计。
const ATTRIBUTE_COST_PER_BYTE: u64 = 16;

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
}

/// XML work preflight, before the parent's namespace/model scan. The caller has already
/// selected XML. Do not use an independent 8 KiB sniff: v0.2 prepends a UTF-8 declaration.
pub(super) fn check_feed_structure(raw: &str) -> Result<(), FeedStructureViolation> {
    check_xml(raw)
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
    let mut reader = Reader::from_str(raw);
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_markup_names_in_closing_tags = true;
    config.trim_text(false);

    let budget =
        |per_byte: u64| BUDGET_SLACK.saturating_add(per_byte.saturating_mul(raw.len() as u64));
    let base_copy_budget = budget(BASE_COPY_PER_BYTE);
    let attribute_budget = budget(ATTRIBUTE_COST_PER_BYTE);
    let mut base_copies = 0_u64;
    let mut attribute_cost = 0_u64;
    let mut open: Vec<OpenElement> = Vec::new();
    let mut buf = Vec::new();

    loop {
        let position = reader.buffer_position();
        let read = reader.read_event_into(&mut buf);
        // The response is already UTF-8. Reader::from_str fixes that encoding, as does the
        // declaration prepended for feed-rs by utf8_parser_input. Leftover XML declarations
        // cannot reinterpret attribute names; the parent's NsReader uses the same UTF-8 input.
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
                let mut bindings = parent.map_or(2, |parent| parent.bindings);
                let mut base_bytes = parent.map_or(0, |parent| parent.base_bytes);
                // 与 NsReader 一样不做重复检查地遍历（它会把重复的命名空间声明都压进绑定表）。
                // 没有错误、也没有重复键时，feed-rs 带重复检查的遍历每一步都通过检查，得到的
                // 属性与这里逐个相同。一旦出现错误或重复，两者出错后的恢复位置可能不同，不去
                // 对齐，按“不规整”整体计费。
                let mut seen = HashSet::new();
                let mut irregular = false;
                let mut attributes = 0_u64;
                let mut key_bytes = 0_u64;
                for attribute in start.attributes().with_checks(false) {
                    let Ok(attribute) = attribute else {
                        irregular = true;
                        continue;
                    };
                    attributes += 1;
                    key_bytes += attribute.key.as_ref().len() as u64;
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
                // 不规整时，feed-rs 在这个标签上切出的属性名与出错次数都不超过标签长度，判重的
                // 哈希探测与线性回查合计不超过长度的平方。
                let cost = if irregular {
                    (start.len() as u64).saturating_pow(2)
                } else {
                    attributes.saturating_mul(attributes.saturating_add(key_bytes))
                };
                attribute_cost = attribute_cost.saturating_add(cost);
                if attribute_cost > attribute_budget {
                    return Err(FeedStructureViolation::BadAttributes);
                }
                // 不规整时 feed-rs 可能切出这里没看到的 `xml:base`：只要标签里有这几个字节，就按
                // 整个标签计。
                if irregular && contains(start, b"xml:base") {
                    base_bytes = base_bytes.saturating_add(start.len());
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
        // NsReader 预置 `xml`、`xmlns` 两个绑定，同一元素上最多再声明 254 个。
        let flat = |count: usize| {
            format!(
                r#"<rss version="2.0"{}><channel/></rss>"#,
                (0..count).map(|i| format!(r#" xmlns:p{i}="urn:x""#)).collect::<String>()
            )
        };
        assert_eq!(check_feed_structure(&flat(254)), Ok(()));
        assert_eq!(
            check_feed_structure(&flat(255)),
            Err(FeedStructureViolation::TooManyNamespaces)
        );
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
        // 规整的标签按属性个数计：判重哈希表的种子固定，挑出哈希低位相同的属性名就能让插入
        // 退化成平方级（这里只测计费，名字本身没有挑）。
        let many = format!(
            r#"<rss version="2.0"{}><channel/></rss>"#,
            (0..2_000).map(|i| format!(r#" x{i}="""#)).collect::<String>()
        );
        assert_eq!(check_feed_structure(&many), Err(FeedStructureViolation::BadAttributes));
        // 完整哈希相同时逐字节比较属性名：属性不多但名字很长也要计。
        let long_names = format!(
            r#"<rss version="2.0"{}><channel/></rss>"#,
            (0..100).map(|i| format!(r#" {}{i}="""#, "k".repeat(3_000))).collect::<String>()
        );
        assert_eq!(check_feed_structure(&long_names), Err(FeedStructureViolation::BadAttributes));
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
            // 非 ASCII 基地址也不能降低计费；v0.2 的输入已固定为 UTF-8。
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
    fn declarations_cannot_reinterpret_already_decoded_utf8() {
        for encoding in ["UTF-16", "UTF-16LE", "UTF-16BE", "GBK", "Shift_JIS"] {
            let xml = format!(
                r#"<?xml version="1.0" encoding="{encoding}"?><rss version="2.0"><channel><title>数学</title></channel></rss>"#
            );
            assert_eq!(check_feed_structure(&xml), Ok(()), "{encoding}");
        }
    }
}
