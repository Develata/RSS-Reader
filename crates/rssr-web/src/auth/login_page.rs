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
    /* 与应用默认主题的 token 对齐（assets/styles/tokens.css）：
     * 登录页是用户进入阅读器前看到的第一屏，配色必须一致。 */
    :root {{
      color-scheme: light;
      --bg: #f4efe6;
      --panel: rgba(255, 250, 242, 0.92);
      --line: rgba(62, 42, 27, 0.14);
      --ink: #1f1a17;
      --muted: #6f6257;
      --accent: #b04a2f;
      --danger: #8d2d1f;
      --shadow: 0 18px 48px rgba(63, 38, 18, 0.08);
      --input-bg: rgba(255, 255, 255, 0.82);
      --focus-ring-color: rgba(176, 74, 47, 0.32);
      --focus-ring-shadow: 0 0 0 4px rgba(176, 74, 47, 0.08);
      --button-shadow: 0 1px 2px rgba(63, 38, 18, 0.18);
      --page-glow: rgba(255, 255, 255, 0.7);
      --page-top: #f2e9dc;
      font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }}
    @media (prefers-color-scheme: dark) {{
      :root {{
        color-scheme: dark;
        --bg: #141414;
        --panel: rgba(28, 28, 28, 0.92);
        --line: rgba(255, 255, 255, 0.08);
        --ink: #f3efe8;
        --muted: #b8aba0;
        --accent: #c05a36;
        --danger: #ffb3a3;
        --shadow: 0 18px 48px rgba(0, 0, 0, 0.4);
        --input-bg: rgba(0, 0, 0, 0.28);
        --focus-ring-color: rgba(210, 120, 83, 0.5);
        --focus-ring-shadow: 0 0 0 4px rgba(210, 120, 83, 0.16);
        --button-shadow: 0 1px 2px rgba(0, 0, 0, 0.45);
        --page-glow: rgba(255, 255, 255, 0.05);
        --page-top: #171717;
      }}
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      background:
        radial-gradient(circle at top, var(--page-glow), transparent 42%),
        linear-gradient(180deg, var(--page-top), var(--bg));
      color: var(--ink);
    }}
    .login-shell {{
      width: min(420px, calc(100vw - 32px));
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 22px;
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
      border-radius: 16px;
      padding: 14px 16px;
      font: inherit;
      color: var(--ink);
      background: var(--input-bg);
    }}
    input:focus {{
      outline: none;
      border-color: var(--focus-ring-color);
      box-shadow: var(--focus-ring-shadow);
    }}
    button {{
      margin-top: 6px;
      border: none;
      border-radius: 12px;
      min-height: 44px;
      padding: 11px 16px;
      font: inherit;
      font-weight: 600;
      background: var(--accent);
      color: #ffffff;
      cursor: pointer;
      box-shadow: var(--button-shadow);
      transition: filter 120ms ease, transform 120ms ease;
    }}
    button:hover {{ filter: brightness(0.96); }}
    button:active {{ filter: brightness(0.9); transform: scale(0.98); }}
    @media (prefers-reduced-motion: reduce) {{
      button {{ transition: none; }}
      button:active {{ transform: none; }}
    }}
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
