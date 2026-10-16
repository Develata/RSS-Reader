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

/// The byte order mark is removed once, where the body is decoded: a page after one is read; a
/// second is text, which ends the head (review C3-R6 R6-04).
#[tokio::test]
async fn discovery_reads_the_page_after_one_byte_order_mark() {
    use rssr_application::{SubscriptionProbeOutcome, SubscriptionProbePort};
    let page =
        b"<html><head><link rel=alternate type=application/rss+xml href='/f.xml'></head></html>";
    for (marks, expected) in [(0, 1), (1, 1), (2, 0)] {
        let body = [b"\xef\xbb\xbf".repeat(marks), page.to_vec()].concat();
        let (url, server) =
            server(vec![response("Content-Type: text/html\r\n", &body)], false).await;
        let outcome = rssr_infra::subscription_probe::HttpSubscriptionProbe::with_feed_client()
            .probe(&url.parse().unwrap())
            .await
            .unwrap();
        server.abort();
        let SubscriptionProbeOutcome::Html { candidates, .. } = outcome else {
            panic!("an HTML page");
        };
        assert_eq!(candidates.len(), expected, "{marks} byte order marks");
    }
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

    // A tag of 256 attributes is read; one of 257 is not, in a feed or an OPML file (review C3-R6
    // R6-01). A repeated name is looked up by scanning the earlier ones: 100 000 distinct names
    // followed by 100 000 repeats would take 10^10 comparisons.
    let attributes = |count: usize| (0..count).map(|i| format!(" a{i}=\"x\"")).collect::<String>();
    let item = |attributes: &str| {
        format!(
            "<rss version=\"2.0\"><channel><title>t</title><item{attributes}><title>i</title></item></channel></rss>"
        )
    };
    assert!(parse(&item(&attributes(256))).is_ok(), "256 attributes");
    assert!(parse(&item(&attributes(257))).is_err(), "257 attributes");
    let repeated = format!("{}{}", attributes(100_000), " a99999=\"x\"".repeat(100_000));
    assert!(parse(&item(&repeated)).is_err(), "repeated attributes");
    let opml = |attributes: &str| {
        format!(
            "<opml version=\"2.0\"><head/><body><outline{attributes} xmlUrl=\"https://example.com/f.xml\"/></body></opml>"
        )
    };
    let codec = rssr_infra::opml::OpmlCodec::new();
    assert_eq!(codec.decode(&opml(&attributes(255))).unwrap().len(), 1);
    assert!(codec.decode(&opml(&attributes(256))).is_err(), "257 attributes in OPML");

    // After a repeated name quick-xml resumes reading at the first space inside the value, so
    // `a="0" a=" xml:base='…'"` hides a base in a value. The check reads attributes as feed-rs
    // does, and counts and measures the hidden base (review C3-R6 R6-02).
    let hidden = |base: &str, levels: usize| {
        let open = format!("<x a=\"0\" a=\" xml:base='{base}'\">");
        format!("{}{}", open.repeat(levels), "</x>".repeat(levels))
    };
    assert!(parse(&rss(&hidden("a/", 1))).is_ok(), "one hidden base");
    assert!(parse(&rss(&hidden("a/", 15))).is_ok(), "16 bases in force, 15 of them hidden");
    assert!(parse(&rss(&hidden("a/", 16))).is_err(), "17 bases in force, 16 of them hidden");
    assert!(parse(&rss(&hidden(&"雪".repeat(1000), 1))).is_err(), "a hidden base over 2 KiB");
    // A backstop: without the bounds these take hours.
    assert!(started.elapsed() < std::time::Duration::from_secs(60), "{:?}", started.elapsed());
}

/// What feed-rs builds is estimated before it reads the feed (review C3-R6 R6-03): large real
/// feeds stay within the estimate; floods of small entries, or of addresses resolved against a
/// long base, do not.
#[test]
fn feed_model_is_bounded_before_feed_rs_builds_it() {
    let parse = |feed: &str| rssr_infra::parser::FeedParser::new().parse(feed);
    const LIMIT: usize = 8 * 1024 * 1024 - 4096;

    // A podcast at the response limit: about 3 000 episodes with full descriptions.
    let mut items = String::new();
    while items.len() < LIMIT {
        let i = items.len();
        let description = "An episode description &lt;p&gt;with markup&lt;/p&gt;. ".repeat(36);
        items.push_str(&format!(
            "<item><title>Episode {i}</title><link>https://example.com/episodes/{i}</link><guid isPermaLink=\"false\">urn:episode:{i}</guid><pubDate>Mon, 01 Jan 2024 00:00:00 +0000</pubDate><description>{description}</description><enclosure url=\"https://cdn.example.com/{i}.mp3\" length=\"12345678\" type=\"audio/mpeg\"/><itunes:duration>01:02:03</itunes:duration><itunes:episode>{i}</itunes:episode><itunes:explicit>false</itunes:explicit><itunes:image href=\"https://cdn.example.com/{i}.jpg\"/><itunes:author>Host</itunes:author><itunes:summary>Summary</itunes:summary><dc:creator>Host</dc:creator><category>Society</category><category>Culture</category><media:content url=\"https://cdn.example.com/{i}.mp3\" type=\"audio/mpeg\" medium=\"audio\"/></item>"
        ));
    }
    let podcast = format!(
        "<rss version=\"2.0\" xmlns:media=\"http://search.yahoo.com/mrss/\" xmlns:itunes=\"http://www.itunes.com/dtds/podcast-1.0.dtd\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><channel><title>t</title>{items}</channel></rss>"
    );
    assert!(parse(&podcast).unwrap().entries.len() > 2500, "a large podcast");

    // A full-text Atom feed at the limit, with an `xml:base` and inline XHTML: the elements of the
    // XHTML are kept as text.
    let mut entries = String::new();
    while entries.len() < LIMIT {
        let i = entries.len();
        let paragraphs =
            "<p>Text with <a href=\"/posts/x\">a link</a> and <em>emphasis</em>.</p>".repeat(20);
        entries.push_str(&format!(
            "<entry><title>Post {i}</title><link rel=\"alternate\" href=\"/posts/{i}\"/><id>tag:example.com,2024:{i}</id><updated>2024-01-01T00:00:00Z</updated><author><name>A</name></author><category term=\"x\"/><content type=\"xhtml\"><div xmlns=\"http://www.w3.org/1999/xhtml\">{paragraphs}</div></content></entry>"
        ));
    }
    let atom = |base: &str, inner: &str| {
        format!(
            "<feed xmlns=\"http://www.w3.org/2005/Atom\" xml:base=\"{base}\"><title>t</title><id>i</id><updated>2024-01-01T00:00:00Z</updated>{inner}</feed>"
        )
    };
    let full_text = atom("https://example.com/blog/wp-atom.php", &entries);
    assert!(parse(&full_text).unwrap().entries.len() > 4000, "a full-text Atom feed");

    // A JSON Feed at the limit.
    let mut items = Vec::new();
    let mut size = 0;
    while size < LIMIT {
        let i = items.len();
        let item = format!(
            "{{\"id\":\"{i}\",\"url\":\"https://example.com/{i}\",\"title\":\"Post {i}\",\"content_html\":\"{}\",\"date_published\":\"2024-01-01T00:00:00Z\",\"authors\":[{{\"name\":\"A\"}}],\"tags\":[\"a\",\"b\"]}}",
            "<p>Some text.</p>".repeat(120)
        );
        size += item.len() + 1;
        items.push(item);
    }
    let json = |items: &str| {
        format!(
            "{{\"version\":\"https://jsonfeed.org/version/1.1\",\"title\":\"t\",\"items\":[{items}]}}"
        )
    };
    assert!(parse(&json(&items.join(","))).unwrap().entries.len() > 3000, "a large JSON Feed");

    // Each flood below is refused by one charge alone; the others would let it through.
    let started = std::time::Instant::now();
    // Links resolved against a base of 2 031 bytes, 6 051 once percent-encoded: 20 000 take about
    // 120 MB (500 000, in 7.5 MB, about 3 GB).
    let base = format!("https://example.com/{}/", "雪".repeat(670));
    assert!(parse(&atom(&base, &"<link href=\"\"/>".repeat(20_000))).is_err(), "links");
    // 200 000 empty items (1.4 MB): about 190 MB of entries.
    let empty = format!(
        "<rss version=\"2.0\"><channel><title>t</title>{}</channel></rss>",
        "<item/>".repeat(200_000)
    );
    assert!(parse(&empty).is_err(), "empty items");
    // 50 000 JSON items: about 100 MB; 300 000 tags of one item (1.2 MB): about 35 MB.
    let ids = (0..50_000).map(|i| format!("{{\"id\":\"{i}\"}}")).collect::<Vec<_>>();
    assert!(parse(&json(&ids.join(","))).is_err(), "JSON items");
    let tags = vec!["\"a\""; 300_000].join(",");
    assert!(parse(&json(&format!("{{\"id\":\"x\",\"tags\":[{tags}]}}"))).is_err(), "JSON tags");
    // `media:content` is not an Atom text construct: what is inside it is charged.
    let thumbnails = "<media:thumbnail url=\"\"/>".repeat(300_000);
    let media = format!(
        "<rss version=\"2.0\" xmlns:media=\"http://search.yahoo.com/mrss/\"><channel><title>t</title><item><media:content url=\"\">{thumbnails}</media:content></item></channel></rss>"
    );
    assert!(parse(&media).is_err(), "media thumbnails");
    // Outside an Atom text construct elements are charged even where feed-rs would skip them.
    let content = "<x/>".repeat(300_000);
    let outside = format!("<entry><id>x</id>{content}</entry>");
    assert!(parse(&atom("https://example.com/", &outside)).is_err(), "elements outside it");
    // A backstop: without the bounds these take hours.
    assert!(started.elapsed() < std::time::Duration::from_secs(60), "{:?}", started.elapsed());
    // Inside one they are text.
    let xhtml = format!("<entry><id>x</id><content type=\"xhtml\">{content}</content></entry>");
    assert!(parse(&atom("https://example.com/", &xhtml)).is_ok(), "elements in content");
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
