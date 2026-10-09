//! 文档开头的 XML 声明，按 feed-rs 内部的 quick-xml 读取它的方式定位。
//!
//! quick-xml 0.41 在遇到的**第一个** `<?xml …?>` 声明事件上切换解码器：声明到第一个 `?>`
//! 为止，取第一个名字恰为 `encoding` 的属性，交给 `Encoding::for_label`（会去掉两端空白）。
//! 见 `reader/state.rs` 的 `emit_question_mark` 与 `events/mod.rs` 的 `BytesDecl::encoder`。
//! 这里的查找没有长度窗口：声明内部的空白不受限制，整段扫描仍以 8 MiB 正文上限为界。

use std::ops::Range;

/// 开头那个 XML 声明里的 `encoding` 属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DeclaredEncoding {
    /// 属性值（不含引号）的字节区间。两端都是 ASCII 引号，可以直接切片已解码的 UTF-8 文本。
    Value(Range<usize>),
    /// 声明里没有 `encoding`；`insert_at` 紧跟最后一个属性，插入 ` encoding="…"` 不破坏语法。
    Missing { insert_at: usize },
    /// 在找到 `encoding` 之前属性列表就已不合语法，quick-xml 不会据此切换编码。
    Unreadable,
}

/// 文本开头（允许前导 XML 空白）的 XML 声明；没有声明时返回 `None`。
pub(crate) fn leading_declaration(text: &[u8]) -> Option<DeclaredEncoding> {
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
        if name_start == content_end {
            return Some(DeclaredEncoding::Missing { insert_at: at });
        }
        if name_start == at {
            // 属性之间必须有空白。
            return Some(DeclaredEncoding::Unreadable);
        }
        let name_end = name_start
            + text[name_start..content_end]
                .iter()
                .position(|&b| b == b'=' || is_xml_space(b))
                .unwrap_or(content_end - name_start);
        let equals = skip_space(text, name_end, content_end);
        if equals == content_end || text[equals] != b'=' {
            return Some(DeclaredEncoding::Unreadable);
        }
        let open = skip_space(text, equals + 1, content_end);
        let Some(&quote) = text[open..content_end].first().filter(|&&b| b == b'"' || b == b'\'')
        else {
            return Some(DeclaredEncoding::Unreadable);
        };
        let value = open + 1;
        let Some(length) = text[value..content_end].iter().position(|&b| b == quote) else {
            return Some(DeclaredEncoding::Unreadable);
        };
        if &text[name_start..name_end] == b"encoding" {
            return Some(DeclaredEncoding::Value(value..value + length));
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

#[cfg(test)]
mod tests {
    use super::{DeclaredEncoding, leading_declaration};

    fn encoding_value(text: &str) -> Option<&str> {
        match leading_declaration(text.as_bytes())? {
            DeclaredEncoding::Value(range) => Some(&text[range]),
            other => panic!("expected an encoding value in {text:?}, got {other:?}"),
        }
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
            assert_eq!(encoding_value(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn distinguishes_missing_unreadable_and_absent_declarations() {
        assert_eq!(
            leading_declaration(br#"<?xml version="1.0"  ?><rss/>"#),
            Some(DeclaredEncoding::Missing { insert_at: 19 })
        );
        assert_eq!(
            leading_declaration(b"<?xml?><rss/>"),
            Some(DeclaredEncoding::Missing { insert_at: 5 })
        );
        for unreadable in [
            r#"<?xml version=1.0 encoding="GBK"?>"#,
            r#"<?xml version="1.0"encoding="GBK"?>"#,
            r#"<?xml version "1.0" encoding="GBK"?>"#,
        ] {
            assert_eq!(
                leading_declaration(unreadable.as_bytes()),
                Some(DeclaredEncoding::Unreadable),
                "{unreadable}"
            );
        }
        for absent in [
            r#"<?xml-stylesheet href="a.xsl" encoding="GBK"?><rss/>"#,
            r#"<!-- c --><?xml version="1.0" encoding="GBK"?><rss/>"#,
            r#"<rss encoding="GBK"/>"#,
            r#"<?xml version="1.0" encoding="GBK""#,
            r#"{"version": "https://jsonfeed.org/version/1.1"}"#,
        ] {
            assert_eq!(leading_declaration(absent.as_bytes()), None, "{absent}");
        }
    }
}
