#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use rssr_infra::{
    fetch::{FetchClient, FetchRequest, FetchResult},
    subscription_probe::MAX_SUBSCRIPTION_BYTES,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

async fn server(responses: Vec<Vec<u8>>, keep_open: bool) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/feed.xml", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        for response in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            let received = socket.read(&mut request).await.unwrap();
            assert!(received > 0, "expected a request before sending the fixture");
            // An early size rejection may close the connection before the server finishes.
            let _ = socket.write_all(&response).await;
            if keep_open {
                std::future::pending::<()>().await;
            }
        }
    });
    (url, task)
}

fn response(headers: &str, body: &[u8]) -> Vec<u8> {
    let mut result = format!("HTTP/1.1 200 OK\r\nConnection: close\r\n{headers}\r\n").into_bytes();
    result.extend_from_slice(body);
    result
}

async fn fetch(raw: Vec<u8>, keep_open: bool, deadline: Duration) -> anyhow::Result<FetchResult> {
    let (url, server) = server(vec![raw], keep_open).await;
    let result = tokio::time::timeout(
        deadline,
        FetchClient::new().fetch(&FetchRequest { url, etag: None, last_modified: None }),
    )
    .await;
    server.abort();
    result.expect("fetch must finish before the test deadline")
}

#[tokio::test]
async fn feed_accepts_exact_limit_with_or_without_content_length() {
    let body = vec![b'x'; MAX_SUBSCRIPTION_BYTES];
    for headers in [String::new(), format!("Content-Length: {}\r\n", body.len())] {
        let result = fetch(response(&headers, &body), false, Duration::from_secs(5)).await.unwrap();
        assert!(
            matches!(result, FetchResult::Fetched { body, .. } if body.len() == MAX_SUBSCRIPTION_BYTES)
        );
    }
}

#[tokio::test]
async fn feed_rejects_oversized_header_without_waiting_for_a_body() {
    let raw = response(&format!("Content-Length: {}\r\n", MAX_SUBSCRIPTION_BYTES + 1), &[]);
    let error = fetch(raw, true, Duration::from_secs(2)).await.unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_rejects_chunked_body_over_the_limit() {
    let mut chunks = Vec::new();
    for size in [MAX_SUBSCRIPTION_BYTES, 1] {
        chunks.extend_from_slice(format!("{size:x}\r\n").as_bytes());
        chunks.extend(std::iter::repeat_n(b'x', size));
        chunks.extend_from_slice(b"\r\n");
    }
    // No terminal chunk: crossing the cap must fail before waiting for end of response.
    let error =
        fetch(response("Transfer-Encoding: chunked\r\n", &chunks), true, Duration::from_secs(5))
            .await
            .unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_bounds_decompressed_bytes_not_the_compressed_content_length() {
    // Python: gzip.compress(b'x' * (8 * 1024 * 1024 + 1), mtime=0).
    let compressed = include_bytes!("fixtures/feed-response-oversized.gz");
    let headers = format!("Content-Encoding: gzip\r\nContent-Length: {}\r\n", compressed.len());
    let error =
        fetch(response(&headers, compressed), false, Duration::from_secs(5)).await.unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_preserves_charset_and_bom_decoding_and_bounds_converted_text() {
    for (headers, bytes, expected) in [
        ("Content-Type: application/xml; charset=windows-1252\r\n", b"caf\xe9".as_slice(), "café"),
        ("Content-Type: application/xml\r\n", b"\xef\xbb\xbfcaf\xc3\xa9".as_slice(), "café"),
    ] {
        let result = fetch(response(headers, bytes), false, Duration::from_secs(5)).await.unwrap();
        assert!(matches!(result, FetchResult::Fetched { body, .. } if body == expected));
    }
    let error = fetch(
        response(
            "Content-Type: application/xml; charset=windows-1252\r\n",
            &vec![0xe9; MAX_SUBSCRIPTION_BYTES / 2 + 1],
        ),
        false,
        Duration::from_secs(5),
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("8 MiB"), "{error:#}");
}

#[tokio::test]
async fn feed_body_stall_still_obeys_the_30_second_request_timeout() {
    let error = fetch(response("Content-Length: 100\r\n", b"x"), true, Duration::from_secs(35))
        .await
        .unwrap_err();
    assert!(
        error.chain().any(|error| error
            .downcast_ref::<reqwest::Error>()
            .is_some_and(reqwest::Error::is_timeout)),
        "{error:#}"
    );
}

/// What feed-rs keeps per open element is bounded before it reads a feed: nested relative
/// `xml:base` attributes, deep nesting and a long `xml:base` are refused; a document with a
/// comment inside its DOCTYPE is still checked (review C3-R5 R5-02, R5-03). A tag of many
/// attributes is read in linear time (quick-xml 0.41), in a feed and in an OPML document.
#[test]
fn feed_structure_is_bounded_before_feed_rs_reads_it() {
    let parse = |feed: &str| rssr_infra::parser::FeedParser::new().parse(feed);
    let started = std::time::Instant::now();
    let rss = |inner: &str| {
        format!(
            "<rss version=\"2.0\" xml:base=\"https://example.com/\"><channel><title>t</title>{inner}<item><title>i</title></item></channel></rss>"
        )
    };

    let nested = |levels: usize| {
        format!("{}{}", "<x xml:base=\"a/\">".repeat(levels), "</x>".repeat(levels))
    };
    assert!(parse(&rss(&nested(50_000))).is_err(), "50 000 nested bases");
    assert!(parse(&rss(&nested(15))).is_ok(), "15 nested bases and the feed's own");
    assert!(parse(&rss(&nested(16))).is_err(), "17 bases in force at once");

    let deep = format!("{}{}", "<x>".repeat(300), "</x>".repeat(300));
    assert!(parse(&rss(&deep)).is_err(), "300 nested elements");
    let deep = format!("{}{}", "<x>".repeat(200), "</x>".repeat(200));
    assert!(parse(&rss(&deep)).is_ok(), "200 nested elements");

    let long_base = format!("<x xml:base=\"{}/\"></x>", "b".repeat(3000));
    assert!(parse(&rss(&long_base)).is_err(), "an xml:base over 2 KiB");

    let doctype = format!("<!DOCTYPE rss [<!-- << -->]>{}", rss(&nested(50_000)));
    assert!(parse(&doctype).is_err(), "a comment in the DOCTYPE does not hide the feed");

    let attributes = (0..100_000).map(|index| format!(" a{index}=\"x\"")).collect::<String>();
    let wide = format!(
        "<rss version=\"2.0\"><channel><title>t</title><item{attributes}><title>i</title></item></channel></rss>"
    );
    assert!(parse(&wide).is_ok(), "a tag of many attributes");
    let opml = format!(
        "<opml version=\"2.0\"><head/><body><outline{attributes} xmlUrl=\"https://example.com/f.xml\"/></body></opml>"
    );
    assert_eq!(rssr_infra::opml::OpmlCodec::new().decode(&opml).unwrap().len(), 1);
    assert!(started.elapsed() < std::time::Duration::from_secs(10), "{:?}", started.elapsed());
}

/// An OPML export with a long title imports again (review C3-R5 R5-06).
#[test]
fn a_long_title_survives_an_opml_round_trip() {
    let title = "长".repeat(3000);
    let codec = rssr_infra::opml::OpmlCodec::new();
    let exported = codec
        .encode(&[rssr_domain::OpmlFeed {
            url: "https://example.com/feed.xml".into(),
            name: Some(title.clone()),
            source_title: Some(title.clone()),
            folder: None,
        }])
        .unwrap();
    assert!(exported.len() > 16 * 1024);
    let imported = codec.decode(&exported).unwrap();
    assert_eq!(imported[0].title.as_deref(), Some(title.as_str()));
}
