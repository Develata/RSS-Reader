use reqwest::{
    StatusCode,
    header::{self, HeaderMap},
};

use super::feed_http::HttpMetadata;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FeedResponseStatus {
    NotModified,
    ReadBody,
}

pub(super) fn classify_feed_response_status(status: StatusCode) -> FeedResponseStatus {
    if status == StatusCode::NOT_MODIFIED {
        FeedResponseStatus::NotModified
    } else {
        FeedResponseStatus::ReadBody
    }
}

pub(super) fn http_metadata_from_headers(headers: &HeaderMap) -> HttpMetadata {
    HttpMetadata {
        etag: headers
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
        last_modified: headers
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use reqwest::{
        StatusCode,
        header::{HeaderMap, HeaderValue},
    };

    use super::{FeedResponseStatus, classify_feed_response_status, http_metadata_from_headers};

    #[test]
    fn classify_feed_response_status_separates_not_modified_from_body_reads() {
        assert_eq!(
            classify_feed_response_status(StatusCode::NOT_MODIFIED),
            FeedResponseStatus::NotModified
        );
        assert_eq!(classify_feed_response_status(StatusCode::OK), FeedResponseStatus::ReadBody);
        assert_eq!(
            classify_feed_response_status(StatusCode::FORBIDDEN),
            FeedResponseStatus::ReadBody
        );
    }

    #[test]
    fn http_metadata_from_headers_extracts_valid_cache_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("etag", HeaderValue::from_static(r#""feed-v1""#));
        headers.insert("last-modified", HeaderValue::from_static("Mon, 13 Apr 2026 00:00:00 GMT"));

        let metadata = http_metadata_from_headers(&headers);

        assert_eq!(metadata.etag.as_deref(), Some(r#""feed-v1""#));
        assert_eq!(metadata.last_modified.as_deref(), Some("Mon, 13 Apr 2026 00:00:00 GMT"));
    }
}
