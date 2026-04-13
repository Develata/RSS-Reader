use super::config::AuthConfig;

const APP_NAME: &str = "RSS-Reader";
const WEB_LOGIN_MARKUP: &str = include_str!("../../../../assets/branding/rssr-mark.svg");

pub(crate) fn render_login_page(
    config: &AuthConfig,
    next: &str,
    error_code: Option<&str>,
) -> String {
    let error_message = login_error_message(error_code);
    let secure_note = secure_cookie_note(config);

    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{} 登录</title>
  <style>
    :root {{
      color-scheme: light;
      --bg: #f5f1ea;
      --panel: rgba(255,255,255,0.92);
      --line: rgba(77, 55, 35, 0.12);
      --ink: #231b14;
      --muted: #6b6258;
      --accent: #6d4c35;
      --accent-strong: #533827;
      --danger: #9b3d2e;
      --shadow: 0 20px 48px rgba(32, 24, 18, 0.08);
      font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      background:
        radial-gradient(circle at top, rgba(255,255,255,0.7), transparent 42%),
        linear-gradient(180deg, #efe4d5, var(--bg));
      color: var(--ink);
    }}
    .login-shell {{
      width: min(420px, calc(100vw - 32px));
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 20px;
      box-shadow: var(--shadow);
      padding: 28px;
    }}
    .brand {{
      display: flex;
      align-items: center;
      gap: 12px;
      margin-bottom: 14px;
    }}
    .brand-mark {{
      width: 56px;
      height: 56px;
      flex: 0 0 auto;
    }}
    .brand-mark svg {{
      display: block;
      width: 100%;
      height: 100%;
    }}
    .brand-name {{
      margin: 0;
      font-size: 1.15rem;
      font-weight: 800;
      letter-spacing: 0.02em;
    }}
    h1 {{ margin: 0 0 8px; font-size: 1.8rem; }}
    p {{ margin: 0 0 16px; color: var(--muted); line-height: 1.6; }}
    form {{ display: grid; gap: 14px; margin-top: 18px; }}
    label {{ display: grid; gap: 6px; font-weight: 600; }}
    input {{
      width: 100%;
      border: 1px solid var(--line);
      border-radius: 12px;
      padding: 12px 14px;
      font: inherit;
      background: rgba(255,255,255,0.95);
    }}
    input:focus {{
      outline: none;
      border-color: rgba(109, 76, 53, 0.45);
      box-shadow: 0 0 0 3px rgba(109, 76, 53, 0.12);
    }}
    button {{
      margin-top: 6px;
      border: none;
      border-radius: 12px;
      padding: 12px 16px;
      font: inherit;
      font-weight: 700;
      background: var(--accent);
      color: white;
      cursor: pointer;
    }}
    button:hover {{ background: var(--accent-strong); }}
    .error {{ min-height: 22px; color: var(--danger); font-weight: 600; }}
    .note {{ margin-top: 14px; font-size: 0.92rem; }}
  </style>
</head>
<body>
  <main class="login-shell">
    <div class="brand">
      <div class="brand-mark">{}</div>
      <p class="brand-name">{}</p>
    </div>
    <h1>登录 {}</h1>
    <p>这个 Web 部署启用了登录保护。输入部署者提供的用户名和密码后，才能进入阅读器。</p>
    <div class="error">{}</div>
    <form method="post" action="/login" autocomplete="on">
      <input type="hidden" name="next" value="{}">
      <label for="login-username">用户名
        <input id="login-username" name="username" type="text" autocomplete="username" required>
      </label>
      <label for="login-password">密码
        <input id="login-password" name="password" type="password" autocomplete="current-password" required>
      </label>
      <button type="submit">登录</button>
    </form>
    <p class="note">{}</p>
  </main>
</body>
</html>"#,
        APP_NAME,
        WEB_LOGIN_MARKUP,
        APP_NAME,
        APP_NAME,
        error_message,
        html_escape(next),
        secure_note
    )
}

pub(crate) fn render_login_failure(error: impl AsRef<str>) -> String {
    format!("登录失败：{}", html_escape(error))
}

pub(crate) fn html_escape(raw: impl AsRef<str>) -> String {
    raw.as_ref()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn login_error_message(error_code: Option<&str>) -> &'static str {
    match error_code {
        Some("invalid_credentials") => "用户名或密码错误。",
        Some("session_expired") => "登录已过期，请重新登录。",
        Some("rate_limited") => "登录尝试过于频繁，请稍后再试。",
        _ => "",
    }
}

fn secure_cookie_note(config: &AuthConfig) -> &'static str {
    if config.secure_cookie {
        "当前会话 cookie 已启用 Secure。"
    } else {
        "当前会话 cookie 未启用 Secure；生产环境请通过 HTTPS 反向代理并开启 RSS_READER_WEB_SECURE_COOKIE=true。"
    }
}

#[cfg(test)]
mod tests {
    use super::html_escape;

    #[test]
    fn html_escape_escapes_attribute_sensitive_characters() {
        assert_eq!(
            html_escape(r#"/entries?x=<script>&quote=""#),
            "/entries?x=&lt;script&gt;&amp;quote=&quot;"
        );
    }
}
