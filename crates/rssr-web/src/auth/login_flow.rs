use axum::{
    http::{HeaderValue, header},
    response::{IntoResponse, Redirect, Response},
};

use super::session::{
    gate_cookie_header, logout_cookie_header, logout_gate_cookie_header, session_cookie_header,
};

pub(super) fn login_redirect(next: &str, error: Option<&str>) -> Response {
    let encoded_next = urlencoding::encode(next);
    let location = match error {
        Some(error) => format!("/login?error={error}&next={encoded_next}"),
        None => format!("/login?next={encoded_next}"),
    };
    Redirect::to(&location).into_response()
}

pub(super) fn successful_login_response(
    secure: bool,
    next: &str,
    token: &str,
    device_cookie: Option<String>,
) -> Response {
    let mut response = Redirect::to(next).into_response();
    if let Some(device_cookie) = device_cookie.and_then(|value| HeaderValue::from_str(&value).ok())
    {
        response.headers_mut().append(header::SET_COOKIE, device_cookie);
    }
    append_cookie(&mut response, session_cookie_header(token, secure));
    append_cookie(&mut response, gate_cookie_header(secure));
    response
}

/// 退出登录后回到登录页，清掉会话与门禁 cookie（已知设备标记保留）。
pub(super) fn logged_out_response(secure: bool) -> Response {
    let mut response = Redirect::to("/login").into_response();
    append_cookie(&mut response, logout_cookie_header(secure));
    append_cookie(&mut response, logout_gate_cookie_header(secure));
    response
}

/// Cookie 头只由本模块按固定格式拼出（令牌是 base64url），一定是合法的头部值。
fn append_cookie(response: &mut Response, cookie: String) {
    let value = HeaderValue::try_from(cookie).expect("cookies are built from header-safe parts");
    response.headers_mut().append(header::SET_COOKIE, value);
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
