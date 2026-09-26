#![cfg(not(target_arch = "wasm32"))]
#[path = "support/discovery_cases.rs"]
mod cases;
use rssr_application::{
    AddSubscriptionInput, AddSubscriptionLifecycleInput, PrepareSubscriptionOutcome,
};
use rssr_domain::EntryQuery;
use rssr_infra::{
    composition::compose_native_sqlite_use_cases,
    db::{migrate, sqlite_native::NativeSqliteBackend, storage_backend::StorageBackend},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[test]
fn parser_contract() {
    cases::assert_discovery_cases();
}

#[tokio::test]
async fn discovery_http_lifecycle_redirect_duplicates_guesses_and_limits() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let hits = Arc::new(Mutex::new(HashMap::<String, usize>::new()));
    let counts = hits.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut socket, _) = listener.accept().await.unwrap();
            let counts = counts.clone();
            tokio::spawn(async move {
                let mut request = [0u8; 4096];
                let n = socket.read(&mut request).await.unwrap();
                let text = String::from_utf8_lossy(&request[..n]);
                let path = text.split_whitespace().nth(1).unwrap().to_string();
                *counts.lock().unwrap().entry(path.clone()).or_default() += 1;
                let (status,headers,body)=match path.as_str() {
                "/" => ("302 Found","Location: /moved/page\r\n",String::new()),
                "/moved/page" => ("200 OK","Content-Type: text/html\r\n","<head><base href='../feeds/'><link rel=alternate type=application/rss+xml href='news.xml'></head>".into()),
                "/multi" => ("200 OK","Content-Type: text/html\r\n","<head><link rel=alternate type=application/rss+xml href='/one'><link rel=alternate type=application/atom+xml href='/two'></head>".into()),
                "/guess" => ("200 OK","Content-Type: text/html\r\n","<html><head></head><body>home</body></html>".into()),
                "/large" => ("200 OK","Content-Type: text/html\r\n","x".repeat(rssr_infra::subscription_probe::MAX_SUBSCRIPTION_BYTES+1)),
                "/feeds/news.xml"|"/rss.xml" => ("200 OK","Content-Type: application/rss+xml\r\n",r#"<rss version="2.0"><channel><title>Test</title><description>Test</description><item><guid>1</guid><title>First</title><description>body</description></item></channel></rss>"#.into()),
                _ => ("404 Not Found","",String::new()),
            };
                let response = format!(
                    "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = socket.write_all(response.as_bytes()).await;
            });
        }
    });
    let backend = NativeSqliteBackend::new("sqlite::memory:");
    let pool = backend.connect().await.unwrap();
    migrate(&pool).await.unwrap();
    let app = compose_native_sqlite_use_cases(pool.clone(), pool).use_cases;
    let input = |url: String, refresh| AddSubscriptionLifecycleInput {
        subscription: AddSubscriptionInput { url, title: None, folder: None },
        refresh_after_add: refresh,
    };
    let outcome = app
        .subscription_workflow
        .add_subscription_lifecycle(input(format!("{origin}/"), true))
        .await
        .unwrap();
    assert_eq!(outcome.feed.url.as_str(), format!("{origin}/feeds/news.xml"));
    assert_eq!(outcome.feed.site_url.unwrap().as_str(), format!("{origin}/moved/page"));
    assert_eq!(
        app.entries_list_service.list_entries(&EntryQuery::default()).await.unwrap().entries.len(),
        1
    );
    assert_eq!(hits.lock().unwrap().get("/feeds/news.xml"), Some(&1));
    assert!(
        app.subscription_workflow
            .add_subscription_lifecycle(input(format!("{origin}/feeds/news.xml"), true))
            .await
            .unwrap_err()
            .to_string()
            .contains("已订阅")
    );
    assert_eq!(hits.lock().unwrap().get("/feeds/news.xml"), Some(&1));
    assert!(
        matches!(app.subscription_workflow.prepare_subscription(&format!("{origin}/multi")).await.unwrap(),PrepareSubscriptionOutcome::NeedsSelection { candidates, .. } if candidates.len()==2)
    );
    let no_refresh = app
        .subscription_workflow
        .add_subscription_lifecycle(input(format!("{origin}/guess"), false))
        .await
        .unwrap();
    assert_eq!(no_refresh.first_refresh, None);
    assert_eq!(
        app.entries_list_service.list_entries(&EntryQuery::default()).await.unwrap().entries.len(),
        1
    );
    for path in ["/feed", "/rss.xml", "/atom.xml", "/index.xml"] {
        assert_eq!(hits.lock().unwrap().get(path), Some(&1));
    }
    assert!(
        app.subscription_workflow
            .prepare_subscription(&format!("{origin}/large"))
            .await
            .unwrap_err()
            .to_string()
            .contains("8 MiB")
    );
    server.abort();
}
