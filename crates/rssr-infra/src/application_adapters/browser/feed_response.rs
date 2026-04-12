use reqwest::header;

pub(super) fn looks_like_proxy_login_or_spa_shell(response: &reqwest::Response) -> bool {
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();

    looks_like_proxy_login_or_spa_shell_parts(content_type, response.url().path())
}

pub(super) fn looks_like_html_response_body(raw: &str) -> bool {
    let trimmed = raw.trim_start_matches('\u{feff}').trim_start();
    let head = trimmed.chars().take(256).collect::<String>().to_ascii_lowercase();

    head.starts_with("<!doctype html")
        || head.starts_with("<html")
        || head.starts_with("<head")
        || head.starts_with("<body")
}

fn looks_like_proxy_login_or_spa_shell_parts(content_type: &str, path: &str) -> bool {
    let content_type = content_type.to_ascii_lowercase();

    content_type.starts_with("text/html")
        || content_type.starts_with("application/xhtml+xml")
        || path.starts_with("/login")
}

#[cfg(test)]
mod tests {
    use super::{looks_like_html_response_body, looks_like_proxy_login_or_spa_shell_parts};

    #[test]
    fn proxy_shell_detection_flags_html_and_login_routes() {
        assert!(looks_like_proxy_login_or_spa_shell_parts("text/html; charset=utf-8", "/"));
        assert!(looks_like_proxy_login_or_spa_shell_parts("application/xhtml+xml", "/"));
        assert!(looks_like_proxy_login_or_spa_shell_parts("application/xml", "/login"));
        assert!(!looks_like_proxy_login_or_spa_shell_parts("application/xml", "/feed-proxy"));
    }

    #[test]
    fn html_body_detection_handles_doctype_tags_and_bom() {
        assert!(looks_like_html_response_body("<!DOCTYPE html><html></html>"));
        assert!(looks_like_html_response_body("<html><body>login</body></html>"));
        assert!(looks_like_html_response_body("\u{feff}   <head><title>shell</title></head>"));
        assert!(!looks_like_html_response_body("<?xml version=\"1.0\"?><rss></rss>"));
    }
}
