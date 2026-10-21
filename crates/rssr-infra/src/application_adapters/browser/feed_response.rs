pub(super) fn looks_like_html_response_body(raw: &str) -> bool {
    let trimmed = raw.trim_start_matches('\u{feff}').trim_start();
    let head = trimmed.chars().take(256).collect::<String>().to_ascii_lowercase();

    head.starts_with("<!doctype html")
        || head.starts_with("<html")
        || head.starts_with("<head")
        || head.starts_with("<body")
}

#[cfg(test)]
mod tests {
    use super::looks_like_html_response_body;

    #[test]
    fn html_body_detection_handles_doctype_tags_and_bom() {
        assert!(looks_like_html_response_body("<!DOCTYPE html><html></html>"));
        assert!(looks_like_html_response_body("<html><body>login</body></html>"));
        assert!(looks_like_html_response_body("\u{feff}   <head><title>shell</title></head>"));
        assert!(!looks_like_html_response_body("<?xml version=\"1.0\"?><rss></rss>"));
    }
}
