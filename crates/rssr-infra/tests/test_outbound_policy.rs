#![cfg(not(target_arch = "wasm32"))]

use std::time::Duration;

use rssr_infra::{
    config_sync::webdav::WebDavConfigSync, fetch::BodyAssetLocalizer, outbound::OutboundPolicy,
};
use tokio::{io::AsyncWriteExt, net::TcpListener};

#[tokio::test]
async fn public_only_image_localization_never_contacts_a_loopback_host() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let _ = socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 4\r\n\r\n\x01\x02\x03\x04")
            .await;
    });
    let html = format!(r#"<p><img src="http://{addr}/hero.png" alt="hero"></p>"#);

    let localized = BodyAssetLocalizer::for_reader_entry()
        .with_outbound(OutboundPolicy::PublicOnly)
        .localize_html_images(&html, None)
        .await
        .expect("localization degrades instead of failing");

    assert!(!localized.contains("data:image"), "loopback image must not be fetched: {localized}");
    assert!(
        tokio::time::timeout(Duration::from_millis(300), server).await.is_err(),
        "the loopback image server must not receive a connection"
    );
}

#[test]
fn public_only_webdav_rejects_a_private_endpoint() {
    let error = WebDavConfigSync::with_outbound(
        "http://192.168.1.10/dav/",
        "rss-reader.json",
        OutboundPolicy::PublicOnly,
    )
    .expect_err("private WebDAV endpoint must be rejected");
    assert!(error.to_string().contains("内网或本地地址"), "unexpected error: {error:#}");
    assert!(
        WebDavConfigSync::new("http://192.168.1.10/dav/", "rss-reader.json").is_ok(),
        "desktop default stays unrestricted"
    );
}
