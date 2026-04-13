use axum::{
    http::{HeaderValue, header},
    response::{IntoResponse, Redirect, Response},
};

use super::{
    config::AuthConfig,
    session::{gate_cookie_header, session_cookie_header},
};

pub(super) fn login_redirect(next: &str, error: Option<&str>) -> Response {
    let encoded_next = urlencoding::encode(next);
    let location = match error {
        Some(error) => format!("/login?error={error}&next={encoded_next}"),
        None => format!("/login?next={encoded_next}"),
    };
    Redirect::to(&location).into_response()
}

pub(super) fn successful_login_response(config: &AuthConfig, next: &str, token: &str) -> Response {
    let mut response = Redirect::to(next).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&session_cookie_header(token, config)).expect("valid session cookie"),
    );
    response.headers_mut().append(
        header::SET_COOKIE,
        HeaderValue::from_str(&gate_cookie_header(config)).expect("valid gate cookie"),
    );
    response
}

#[cfg(test)]
mod tests {
    use axum::http::header;

    use super::login_redirect;

    #[test]
    fn login_redirect_encodes_next_and_optional_error() {
        let response = login_redirect("/entries/1?filter=unread", Some("rate_limited"));
        assert_eq!(
            response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok()),
            Some("/login?error=rate_limited&next=%2Fentries%2F1%3Ffilter%3Dunread")
        );

        let response = login_redirect("/entries/1?filter=unread", None);
        assert_eq!(
            response.headers().get(header::LOCATION).and_then(|value| value.to_str().ok()),
            Some("/login?next=%2Fentries%2F1%3Ffilter%3Dunread")
        );
    }
}
