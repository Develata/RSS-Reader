//! 文档开头 XML 声明里的 `encoding`，供解码前的编码探测使用（XML 1.0 §2.8、§4.3.3）。
//!
//! 声明到第一个 `?>` 为止，属性按顺序读取，取第一个名字恰为 `encoding` 的属性，与 feed-rs
//! 内部的 quick-xml 0.41 一致（`reader/state.rs` 的 `emit_question_mark`、`events/mod.rs` 的
//! `BytesDecl::encoding`）。没有长度窗口：声明内部的空白不受限制，整段扫描以 8 MiB 正文上限为界。

use std::ops::Range;

/// 文本开头（允许前导 XML 空白）那个声明里 `encoding` 属性值（不含引号）的字节区间。
/// 没有声明、声明里没有 `encoding`、或在读到它之前属性就已不合语法时返回 `None`。
pub(crate) fn leading_declared_encoding(text: &[u8]) -> Option<Range<usize>> {
    let start = skip_space(text, 0, text.len());
    let attributes = start + b"<?xml".len();
    if !text[start..].starts_with(b"<?xml") {
        return None;
    }
    let content_end = attributes + text[attributes..].windows(2).position(|pair| pair == b"?>")?;
    // `<?xml-stylesheet …?>` 是处理指令；`<?xml?>` 是没有属性的声明。
    if attributes < content_end && !is_xml_space(text[attributes]) {
        return None;
    }

    let mut at = attributes;
    loop {
        let name_start = skip_space(text, at, content_end);
        // 属性之间必须有空白；读到声明结尾说明没有 `encoding`。
        if name_start == content_end || name_start == at {
            return None;
        }
        let name_end = name_start
            + text[name_start..content_end]
                .iter()
                .position(|&b| b == b'=' || is_xml_space(b))
                .unwrap_or(content_end - name_start);
        let equals = skip_space(text, name_end, content_end);
        if equals == content_end || text[equals] != b'=' {
            return None;
        }
        let open = skip_space(text, equals + 1, content_end);
        let &quote = text[open..content_end].first().filter(|&&b| b == b'"' || b == b'\'')?;
        let value = open + 1;
        let length = text[value..content_end].iter().position(|&b| b == quote)?;
        if &text[name_start..name_end] == b"encoding" {
            return Some(value..value + length);
        }
        at = value + length + 1;
    }
}

/// XML 1.0 §2.3 `S`，也是 quick-xml 的空白定义；不同于 `u8::is_ascii_whitespace`，换页符不算。
pub(crate) fn is_xml_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn skip_space(text: &[u8], from: usize, end: usize) -> usize {
    from + text[from..end].iter().take_while(|&&b| is_xml_space(b)).count()
}

/// Elements a feed may nest at most. feed-rs walks a feed recursively and keeps, for every open
/// element, a copy of the base address in force; no real feed comes near this depth.
pub(crate) const MAX_XML_DEPTH: usize = 256;
/// `xml:base` attributes in force at once at most. feed-rs resolves each against the one above it
/// and keeps them all until their elements close, so nested relative bases grow the retained
/// addresses with their number squared (review C3-R5 R5-02).
pub(crate) const MAX_XML_BASES: usize = 16;

/// Refuses a feed feed-rs would read with more than [`MAX_XML_DEPTH`] nested elements, more
/// than [`MAX_XML_BASES`] `xml:base` attributes in force at once, or an `xml:base` longer than
/// [`rssr_application::MAX_URL_BYTES`]. Within these, the addresses feed-rs keeps stay under a
/// few megabytes.
///
/// Read with the quick-xml version and reader settings feed-rs uses, so the elements are the
/// ones it sees; a document this reader cannot read is refused here, as feed-rs would refuse it
/// (review C3-R5 R5-03). Linear: attributes are read without the duplicate check.
pub(crate) fn check_feed_structure(raw: &str) -> anyhow::Result<()> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(raw);
    let config = reader.config_mut();
    config.expand_empty_elements = true;
    config.trim_text(false);
    // For every open element: whether it sets `xml:base`.
    let mut open = Vec::new();
    let mut bases = 0_usize;
    loop {
        match reader.read_event()? {
            Event::Start(tag) => {
                anyhow::ensure!(
                    open.len() < MAX_XML_DEPTH,
                    "feed 的元素嵌套超过 {MAX_XML_DEPTH} 层"
                );
                let mut attributes = tag.attributes();
                attributes.with_checks(false);
                let mut sets_base = false;
                // feed-rs skips attributes it cannot read; so does this.
                for attribute in attributes.flatten() {
                    if attribute.key.as_ref() == b"xml:base" {
                        anyhow::ensure!(
                            attribute.value.len() <= rssr_application::MAX_URL_BYTES,
                            "feed 的 xml:base 超过 {} 字节",
                            rssr_application::MAX_URL_BYTES
                        );
                        sets_base = true;
                    }
                }
                if sets_base {
                    bases += 1;
                    anyhow::ensure!(
                        bases <= MAX_XML_BASES,
                        "feed 的 xml:base 嵌套超过 {MAX_XML_BASES} 层"
                    );
                }
                open.push(sets_base);
            }
            Event::End(_) => {
                if open.pop() == Some(true) {
                    bases -= 1;
                }
            }
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::leading_declared_encoding;

    fn found(text: &str) -> Option<&str> {
        leading_declared_encoding(text.as_bytes()).map(|range| &text[range])
    }

    #[test]
    fn finds_the_encoding_attribute_the_way_quick_xml_does() {
        let padded = format!(r#"<?xml version="1.0" encoding="GBK"{}?><rss/>"#, " ".repeat(1100));
        let indented = format!("{}<?xml version='1.0' encoding = 'Big5'?>", "\n".repeat(1100));
        for (text, expected) in [
            (r#"<?xml version="1.0" encoding="GBK"?>"#, "GBK"),
            (padded.as_str(), "GBK"),
            (indented.as_str(), "Big5"),
            (r#"<?xml version="1.0" encoding=" gbk "?>"#, " gbk "),
            (r#"<?xml version="1.0" encodingx="a" encoding="GBK"?>"#, "GBK"),
            (r#"<?xml version="1.0" standalone="yes" encoding="1bad"?>"#, "1bad"),
        ] {
            assert_eq!(found(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn missing_unreadable_or_absent_declarations_give_nothing() {
        for text in [
            r#"<?xml version="1.0"  ?><rss/>"#,
            "<?xml?><rss/>",
            r#"<?xml version=1.0 encoding="GBK"?>"#,
            r#"<?xml version="1.0"encoding="GBK"?>"#,
            r#"<?xml version "1.0" encoding="GBK"?>"#,
            r#"<?xml-stylesheet href="a.xsl" encoding="GBK"?><rss/>"#,
            r#"<!-- c --><?xml version="1.0" encoding="GBK"?><rss/>"#,
            r#"<rss encoding="GBK"/>"#,
            r#"<?xml version="1.0" encoding="GBK""#,
            r#"{"version": "https://jsonfeed.org/version/1.1"}"#,
        ] {
            assert_eq!(found(text), None, "{text}");
        }
    }
}
