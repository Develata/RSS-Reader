//! Shared transport boundary for subscription discovery and feed refresh.

use anyhow::{Context, Result, ensure};
use encoding_rs::{Encoding, UTF_8, UTF_16BE, UTF_16LE};
use futures_util::StreamExt;
use reqwest::header;

use crate::xml_declaration::{DeclaredEncoding, leading_declaration};

pub(crate) const MAX_FEED_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

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

/// Decodes a feed body once, using the RFC 7303 §3.2 priority: a BOM is authoritative, then the
/// HTTP `charset` parameter, then the XML encoding declaration (XML 1.0 §4.3.3), then UTF-8.
///
/// The declaration is left in the text. The feed parser re-reads the text as bytes and would
/// honour it, so `parse_feed_xml` pins the parser to UTF-8 at that boundary (audit B2).
fn decode_feed_bytes(bytes: &[u8], charset: Option<&'static Encoding>) -> Result<String> {
    let encoding = charset.or_else(|| declared_encoding(bytes)).unwrap_or(UTF_8);
    // `decode` lets a BOM override `encoding` and strips it.
    let (body, _, _) = encoding.decode(bytes);
    // Character conversion can expand the body even when the transport bytes fit.
    ensure!(body.len() <= MAX_FEED_RESPONSE_BYTES, "订阅响应超过 8 MiB 上限。");
    Ok(body.into_owned())
}

/// A declaration found by ASCII matching implies an ASCII-compatible stream without a BOM, so a
/// UTF-16 label there contradicts the bytes; like HTML's `<meta charset>`, read it as UTF-8.
/// Labels encoding_rs does not know fall back to UTF-8 as well.
fn declared_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    let DeclaredEncoding::Value(label) = leading_declaration(bytes)? else {
        return None;
    };
    let encoding = Encoding::for_label(&bytes[label])?;
    Some(if encoding == UTF_16LE || encoding == UTF_16BE { UTF_8 } else { encoding })
}

#[cfg(test)]
mod tests {
    use encoding_rs::{GBK, WINDOWS_1252};

    use super::*;

    fn decode(bytes: &[u8], charset: Option<&'static Encoding>) -> String {
        decode_feed_bytes(bytes, charset).expect("decode")
    }

    #[test]
    fn declaration_decides_without_bom_or_charset() {
        let xml = r#"<?xml version="1.0" encoding="GBK" standalone="yes"?><rss><title>中文</title></rss>"#;
        let (bytes, _, _) = GBK.encode(xml);
        assert_eq!(decode(&bytes, None), xml);
        // The declaration may be padded beyond any fixed lookahead (XML 1.0 §2.8 `S?`).
        let padded =
            format!(r#"<?xml version="1.0" encoding="GBK"{}?><t>中文</t>"#, " ".repeat(1100));
        let (bytes, _, _) = GBK.encode(&padded);
        assert_eq!(decode(&bytes, None), padded);
    }

    #[test]
    fn charset_outranks_the_declaration_and_bom_outranks_both() {
        let xml = r#"<?xml version="1.0" encoding='GBK'?><t>café</t>"#;
        let (latin1, _, _) = WINDOWS_1252.encode(xml);
        assert_eq!(decode(&latin1, Some(WINDOWS_1252)), xml);
        let with_bom = [b"\xef\xbb\xbf".as_slice(), xml.as_bytes()].concat();
        assert_eq!(decode(&with_bom, Some(WINDOWS_1252)), xml);
    }

    #[test]
    fn utf16_bom_text_is_decoded() {
        let xml = r#"<?xml version="1.0" encoding="UTF-16"?><t>中文</t>"#;
        let bytes =
            [&[0xff, 0xfe][..], &xml.encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()]
                .concat();
        assert_eq!(decode(&bytes, None), xml);
    }

    #[test]
    fn contradictory_or_unknown_declarations_fall_back_to_utf8() {
        for label in ["UTF-16", "x-no-such-encoding", "1bad"] {
            let xml = format!(r#"<?xml version="1.0" encoding="{label}"?><t>中文</t>"#);
            assert_eq!(decode(xml.as_bytes(), None), xml, "{label}");
        }
    }

    #[test]
    fn plain_utf8_feed_is_returned_verbatim() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?><t>中文</t>"#;
        assert_eq!(decode(xml.as_bytes(), None), xml);
        assert_eq!(decode(b"<rss/>", None), "<rss/>");
    }
}
