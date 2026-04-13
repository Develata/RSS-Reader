use axum::{
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse},
};

use crate::auth::{AppState, issue_smoke_auth_cookie_headers};

const SMOKE_FEED_TITLE: &str = "Codex Smoke Feed";
const SMOKE_ENTRY_TITLE: &str = "Codex Smoke Entry";
const SMOKE_ENTRY_URL: &str = "https://example.com/posts/codex-smoke-entry";

pub(crate) fn smoke_helpers_enabled() -> bool {
    std::env::var("RSS_READER_WEB_ENABLE_SMOKE_HELPERS")
        .ok()
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "True"))
        .unwrap_or(false)
}

pub(crate) async fn browser_feed_smoke(State(state): State<AppState>) -> impl IntoResponse {
    let (session_cookie, gate_cookie) = match issue_smoke_auth_cookie_headers(&state.config) {
        Ok(headers) => headers,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("构造 smoke 登录态失败：{err}")),
            )
                .into_response();
        }
    };

    let html = format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>rssr-web browser feed smoke</title>
  <style>
    body {{
      margin: 0;
      font: 14px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
      background: #101418;
      color: #e9eef5;
      padding: 12px;
    }}
    h1 {{ margin: 0 0 8px; font-size: 16px; }}
    pre {{
      white-space: pre-wrap;
      background: #171d24;
      border: 1px solid #27313d;
      border-radius: 10px;
      padding: 10px;
      margin: 0 0 12px;
    }}
    iframe {{
      width: 1440px;
      height: 1200px;
      border: 1px solid #27313d;
      border-radius: 12px;
      background: white;
    }}
  </style>
</head>
<body data-smoke="rssr-web-browser-feed-smoke" data-result="pending">
  <h1>rssr-web browser feed smoke</h1>
  <pre id="log">booting...</pre>
  <pre id="summary"></pre>
  <iframe id="app" src="/feeds"></iframe>
  <script>
    const feedUrl = `${{window.location.origin}}/__codex/feed-fixture.xml`;
    const expectedFeedTitle = {SMOKE_FEED_TITLE:?};
    const expectedEntryTitle = {SMOKE_ENTRY_TITLE:?};
    const timeoutMs = 60000;
    const logEl = document.getElementById("log");
    const summaryEl = document.getElementById("summary");
    const frame = document.getElementById("app");
    const logs = [];

    function log(message) {{
      logs.push(message);
      logEl.textContent = logs.join("\\n");
    }}

    function setResult(status, extra) {{
      document.body.dataset.result = status;
      summaryEl.textContent = JSON.stringify({{
        status,
        feedUrl,
        expectedFeedTitle,
        expectedEntryTitle,
        ...extra,
        logs,
      }}, null, 2);
    }}

    function sleep(ms) {{
      return new Promise(resolve => setTimeout(resolve, ms));
    }}

    async function waitFor(check, label, timeout = timeoutMs) {{
      const start = Date.now();
      for (;;) {{
        const result = check();
        if (result) return result;
        if (Date.now() - start > timeout) {{
          throw new Error(`timeout waiting for ${{label}}`);
        }}
        await sleep(250);
      }}
    }}

    function setInputValue(input, value) {{
      input.focus();
      input.value = value;
      input.dispatchEvent(new Event("input", {{ bubbles: true, composed: true }}));
      input.dispatchEvent(new Event("change", {{ bubbles: true, composed: true }}));
    }}

    async function main() {{
      log("waiting for /feeds iframe");
      const feedsDoc = await waitFor(
        () => frame.contentDocument?.querySelector('[data-page="feeds"]') ? frame.contentDocument : null,
        "feeds page"
      );

      const feedInput = await waitFor(
        () => feedsDoc.querySelector('[data-field="feed-url-input"]'),
        "feed input"
      );
      setInputValue(feedInput, feedUrl);
      log(`filled feed url: ${{feedUrl}}`);

      const addButton = await waitFor(
        () => feedsDoc.querySelector('[data-action="add-feed"]'),
        "add-feed button"
      );
      addButton.click();
      log("clicked add-feed");

      const feedCard = await waitFor(
        () => Array.from(feedsDoc.querySelectorAll('li[data-layout="feed-card"]')).find(card =>
          card.textContent.includes(expectedFeedTitle) || card.textContent.includes(feedUrl)
        ),
        "new feed card",
        90000
      );
      log("feed card appeared");

      const refreshButton = await waitFor(
        () => feedCard.querySelector('[data-action="refresh-feed"]'),
        "refresh-feed button"
      );
      const refreshStateBefore = feedCard.dataset.refreshState || "unknown";
      const lastFetchedAtBefore = BigInt(feedCard.dataset.lastFetchedAt || "0");
      refreshButton.click();
      log("clicked refresh-feed");

      const refreshedCard = await waitFor(
        () => {{
          const refreshStateAfter = feedCard.dataset.refreshState || "unknown";
          const lastFetchedAtAfter = BigInt(feedCard.dataset.lastFetchedAt || "0");
          if (refreshStateAfter === "failed") {{
            throw new Error(`refresh failed: ${{feedCard.dataset.fetchError || "unknown error"}}`);
          }}
          if (refreshStateAfter !== refreshStateBefore && refreshStateAfter !== "never") {{
            return feedCard;
          }}
          if (lastFetchedAtAfter > lastFetchedAtBefore && refreshStateAfter !== "never") {{
            return feedCard;
          }}
          return null;
        }},
        "successful refresh result",
        90000
      );
      log(`refresh finished: state=${{refreshedCard.dataset.refreshState}} entries=${{refreshedCard.dataset.entryCount}} lastFetchedAt=${{refreshedCard.dataset.lastFetchedAt}}`);

      const feedEntriesLink = await waitFor(
        () => refreshedCard.querySelector('[data-nav="feed-entries"]'),
        "feed-entries link"
      );
      feedEntriesLink.click();
      log("clicked feed-entries");

      const entriesDoc = await waitFor(
        () => frame.contentDocument?.querySelector('[data-page="entries"][data-entry-scope="feed"]')
          ? frame.contentDocument
          : null,
        "feed entries page",
        90000
      );

      await waitFor(
        () => entriesDoc.body.textContent.includes(expectedEntryTitle),
        "expected entry title",
        90000
      );
      log("entry page loaded");

      setResult("pass", {{
        finalPath: frame.contentWindow.location.pathname,
        refreshState: refreshedCard.dataset.refreshState,
        entryCount: refreshedCard.dataset.entryCount,
        fetchError: refreshedCard.dataset.fetchError,
        feedCardText: refreshedCard.textContent.replace(/\\s+/g, " ").trim(),
      }});
    }}

    main().catch(error => {{
      log(`error: ${{String(error)}}`);
      setResult("fail", {{ error: String(error) }});
    }});
  </script>
</body>
</html>"#
    );

    let mut response = (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
        Html(html),
    )
        .into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie).expect("valid session cookie"),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&gate_cookie).expect("valid gate cookie"),
    );
    response
}

pub(crate) async fn feed_fixture() -> impl IntoResponse {
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>{}</title>
    <link>https://example.com/</link>
    <description>Codex smoke feed fixture.</description>
    <item>
      <guid>{}</guid>
      <title>{}</title>
      <link>{}</link>
      <pubDate>Fri, 10 Apr 2026 12:00:00 GMT</pubDate>
      <description><![CDATA[<p>Codex smoke feed entry body.</p>]]></description>
    </item>
  </channel>
</rss>"#,
        SMOKE_FEED_TITLE, SMOKE_ENTRY_URL, SMOKE_ENTRY_TITLE, SMOKE_ENTRY_URL
    );

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, HeaderValue::from_static("application/rss+xml; charset=utf-8"))],
        xml,
    )
}
