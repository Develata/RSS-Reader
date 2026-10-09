//! Shared transport boundary for subscription discovery and feed refresh.

use std::ops::Range;

use anyhow::{Context, Result, ensure};
use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE};
use futures_util::StreamExt;
use reqwest::header;

pub(crate) const MAX_FEED_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// The XML declaration is a few dozen bytes; bounding the lookahead keeps the sniff O(1).
const XML_DECLARATION_LOOKAHEAD: usize = 1024;

/// Bound the decoded HTTP stream, including chunked and compressed responses. The header is
/// only an early rejection; neither a missing nor a compressed Content-Length bypasses the cap.
pub(crate) async fn read_feed_text(response: reqwest::Response) -> Result<String> {
    ensure!(
        response.content_length().is_none_or(|length| length <= MAX_FEED_RESPONSE_BYTES as u64),
        "订阅响应超过 8 MiB 上限。"
    );
    let charset = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|content_type| {
            content_type.split(';').skip(1).find_map(|part| {
                let (key, value) = part.trim().split_once('=')?;
                key.trim().eq_ignore_ascii_case("charset").then(|| value.trim().trim_matches('"'))
            })
        })
        .and_then(|label| Encoding::for_label(label.as_bytes()));
    let mut bytes = Vec::new();
    let stream = response.bytes_stream();
    futures_util::pin_mut!(stream);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取 feed 响应正文失败")?;
        ensure!(chunk.len() <= MAX_FEED_RESPONSE_BYTES - bytes.len(), "订阅响应超过 8 MiB 上限。");
        bytes.extend_from_slice(&chunk);
    }
    decode_feed_bytes(&bytes, charset)
}

/// Decodes a feed body exactly once, here, using the RFC 7303 §3.2 priority: a BOM is
/// authoritative, then the HTTP `charset` parameter, then the XML encoding declaration
/// (XML 1.0 §4.3.3), then UTF-8.
///
/// Postcondition: the text declares no encoding other than UTF-8. The XML parser honours the
/// declaration, so a stale `encoding="GBK"` would make it decode the decoded text a second time.
fn decode_feed_bytes(bytes: &[u8], charset: Option<&'static Encoding>) -> Result<String> {
    let encoding = charset.or_else(|| declared_encoding(bytes)).unwrap_or(UTF_8);
    // `decode` lets a BOM override `encoding` and strips it.
    let (body, _, _) = encoding.decode(bytes);
    let mut body = body.into_owned();
    if let Some(range) = declared_encoding_range(body.as_bytes())
        && !body[range.clone()].eq_ignore_ascii_case("UTF-8")
    {
        body.replace_range(range, "UTF-8");
    }
    // Character conversion can expand the body even when the transport bytes fit.
    ensure!(body.len() <= MAX_FEED_RESPONSE_BYTES, "订阅响应超过 8 MiB 上限。");
    Ok(body)
}

/// A declaration found by ASCII matching implies an ASCII-compatible stream without a BOM, so a
/// UTF-16 label there contradicts the bytes; like HTML's `<meta charset>`, read it as UTF-8.
/// Labels encoding_rs does not know fall back to UTF-8 as well.
fn declared_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    let encoding = Encoding::for_label(&bytes[declared_encoding_range(bytes)?])?;
    Some(if encoding == UTF_16LE || encoding == UTF_16BE { UTF_8 } else { encoding })
}

/// Byte range of the `EncName` in a leading XML declaration (XML 1.0 §2.8 `XMLDecl`, §4.3.3).
/// Leading XML whitespace is tolerated because the parser tolerates it too, so the declaration
/// sniffed here is the one the parser would honour. The range only ever covers ASCII bytes, so
/// it also slices decoded text on character boundaries.
fn declared_encoding_range(text: &[u8]) -> Option<Range<usize>> {
    let head = &text[..text.len().min(XML_DECLARATION_LOOKAHEAD)];
    let skip_space = |at: usize| at + head[at..].iter().take_while(|&&b| is_xml_space(b)).count();
    let start = skip_space(0);
    if !head[start..].starts_with(b"<?xml") {
        return None;
    }
    let attributes = start + b"<?xml".len();
    // `<?xml-stylesheet …?>` is a processing instruction, not the declaration.
    if !head.get(attributes).is_some_and(|&b| is_xml_space(b)) {
        return None;
    }
    let end = attributes + head[attributes..].windows(2).position(|pair| pair == b"?>")?;
    let key = (attributes..end)
        .find(|&at| head[at..end].starts_with(b"encoding") && is_xml_space(head[at - 1]))?;
    let equals = skip_space(key + b"encoding".len());
    if head.get(equals) != Some(&b'=') {
        return None;
    }
    let open = skip_space(equals + 1);
    let quote = *head.get(open).filter(|&&b| b == b'"' || b == b'\'')?;
    let value = open + 1;
    let close = value + head[value..end].iter().position(|&b| b == quote)?;
    // EncName ::= [A-Za-z] ([A-Za-z0-9._] | '-')*
    let name = &head[value..close];
    let is_enc_name = name.first().is_some_and(u8::is_ascii_alphabetic)
        && name.iter().all(|&b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    is_enc_name.then_some(value..close)
}

/// XML 1.0 §2.3 `S`; unlike `u8::is_ascii_whitespace`, form feed is not XML whitespace.
fn is_xml_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

#[cfg(test)]
mod tests {
    use encoding_rs::{GBK, WINDOWS_1252};

    use super::*;

    const GBK_FEED: &str =
        r#"<?xml version="1.0" encoding="GBK" standalone="yes"?><rss><title>中文</title></rss>"#;

    fn decode(bytes: &[u8], charset: Option<&'static Encoding>) -> String {
        decode_feed_bytes(bytes, charset).expect("decode")
    }

    #[test]
    fn declaration_decides_without_bom_or_charset_and_is_rewritten() {
        let (bytes, _, _) = GBK.encode(GBK_FEED);
        assert_eq!(
            decode(&bytes, None),
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><rss><title>中文</title></rss>"#
        );
    }

    #[test]
    fn charset_outranks_the_declaration_and_bom_outranks_both() {
        let xml = r#"<?xml version="1.0" encoding='GBK'?><t>café</t>"#;
        let (latin1, _, _) = WINDOWS_1252.encode(xml);
        assert_eq!(
            decode(&latin1, Some(WINDOWS_1252)),
            r#"<?xml version="1.0" encoding='UTF-8'?><t>café</t>"#
        );
        let with_bom = [b"\xef\xbb\xbf".as_slice(), xml.as_bytes()].concat();
        assert_eq!(
            decode(&with_bom, Some(WINDOWS_1252)),
            r#"<?xml version="1.0" encoding='UTF-8'?><t>café</t>"#
        );
    }

    #[test]
    fn utf16_bom_text_is_decoded_and_its_declaration_rewritten() {
        let xml = r#"<?xml version="1.0" encoding="UTF-16"?><t>中文</t>"#;
        let bytes =
            [&[0xff, 0xfe][..], &xml.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()]
                .concat();
        assert_eq!(decode(&bytes, None), r#"<?xml version="1.0" encoding="UTF-8"?><t>中文</t>"#);
    }

    #[test]
    fn contradictory_or_unknown_declarations_fall_back_to_utf8() {
        for label in ["UTF-16", "x-no-such-encoding"] {
            let xml = format!(r#"<?xml version="1.0" encoding="{label}"?><t>中文</t>"#);
            assert_eq!(
                decode(xml.as_bytes(), None),
                r#"<?xml version="1.0" encoding="UTF-8"?><t>中文</t>"#,
                "{label}"
            );
        }
    }

    #[test]
    fn declaration_sniff_matches_only_the_xml_declaration() {
        let cases: [(&[u8], Option<&str>); 7] = [
            (b"<?xml version='1.0' encoding = 'Big5'?>", Some("Big5")),
            (b"\r\n <?xml version=\"1.0\" encoding=\"gbk\"?>", Some("gbk")),
            (b"<?xml-stylesheet href=\"a.xsl\" encoding=\"GBK\"?>", None),
            (b"<?xml version=\"1.0\"?><t encoding=\"GBK\"/>", None),
            (b"<?xml version=\"1.0\" myencoding=\"GBK\"?>", None),
            (b"<?xml version=\"1.0\" encoding=\"1bad\"?>", None),
            (b"<rss encoding=\"GBK\"/>", None),
        ];
        for (text, expected) in cases {
            let found = declared_encoding_range(text).map(|range| &text[range]);
            assert_eq!(found, expected.map(str::as_bytes), "{}", String::from_utf8_lossy(text));
        }
    }

    #[test]
    fn unchanged_utf8_feed_is_returned_verbatim() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><t>中文</t>"#;
        assert_eq!(decode(xml.as_bytes(), None), xml);
        assert_eq!(decode(b"<rss/>", None), "<rss/>");
    }
}
