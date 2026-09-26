//! Shared transport boundary for subscription discovery and feed refresh.

use anyhow::{Context, Result, ensure};
use futures_util::StreamExt;
use reqwest::header;

pub(crate) const MAX_FEED_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

/// Bound the decoded HTTP stream, including chunked and compressed responses. The header is
/// only an early rejection; neither a missing nor a compressed Content-Length bypasses the cap.
pub(crate) async fn read_feed_text(response: reqwest::Response) -> Result<String> {
    ensure!(
        response.content_length().is_none_or(|length| length <= MAX_FEED_RESPONSE_BYTES as u64),
        "订阅响应超过 8 MiB 上限。"
    );
    let encoding = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|content_type| {
            content_type.split(';').skip(1).find_map(|part| {
                let (key, value) = part.trim().split_once('=')?;
                key.trim().eq_ignore_ascii_case("charset").then(|| value.trim().trim_matches('"'))
            })
        })
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()))
        .unwrap_or(encoding_rs::UTF_8);
    let mut bytes = Vec::new();
    let stream = response.bytes_stream();
    futures_util::pin_mut!(stream);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("读取 feed 响应正文失败")?;
        ensure!(chunk.len() <= MAX_FEED_RESPONSE_BYTES - bytes.len(), "订阅响应超过 8 MiB 上限。");
        bytes.extend_from_slice(&chunk);
    }
    let (body, _, _) = encoding.decode(&bytes);
    // Character conversion can expand the body even when the transport bytes fit.
    ensure!(body.len() <= MAX_FEED_RESPONSE_BYTES, "订阅响应超过 8 MiB 上限。");
    Ok(body.into_owned())
}
