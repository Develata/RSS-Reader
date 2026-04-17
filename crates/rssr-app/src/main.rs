#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod bootstrap;
mod components;
mod hooks;
mod pages;
mod router;
mod status;
mod theme;
mod ui;
mod web_auth;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
use std::{borrow::Cow, sync::OnceLock};

#[cfg(not(target_arch = "wasm32"))]
use tracing_subscriber::EnvFilter;

#[cfg(target_arch = "wasm32")]
fn init_tracing() {
    let mut builder = tracing_wasm::WASMLayerConfigBuilder::new();
    builder.set_max_level(tracing::Level::INFO);
    tracing_wasm::set_as_global_default_with_config(builder.build());
}

#[cfg(not(target_arch = "wasm32"))]
fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,dioxus_desktop::edits=off")),
        )
        .init();
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn main() {
    use dioxus::desktop::{
        Config, LogicalSize, WindowBuilder,
        tao::window::Icon,
        wry::{RequestAsyncResponder, http::Request as HttpRequest},
    };
    use dioxus::prelude::LaunchBuilder;
    use image::ImageFormat;

    init_tracing();

    let window_icon = load_window_icon();
    let window = WindowBuilder::new()
        .with_title("RSS-Reader")
        .with_window_icon(window_icon)
        .with_inner_size(LogicalSize::new(1280.0, 900.0))
        .with_visible(true)
        .with_focused(true)
        .with_decorations(true)
        .with_resizable(true)
        .with_minimizable(true)
        .with_maximizable(true)
        .with_closable(true);

    let config =
        Config::new().with_window(window).with_menu(None).with_asynchronous_custom_protocol(
            crate::pages::reader_page::support::DESKTOP_IMAGE_PROXY_SCHEME,
            |_webview_id, request: HttpRequest<Vec<u8>>, responder: RequestAsyncResponder| {
                tokio::spawn(async move {
                    responder.respond(desktop_image_proxy_response(request).await);
                });
            },
        );
    LaunchBuilder::new().with_cfg(config).launch(app::App);

    fn load_window_icon() -> Option<Icon> {
        let image = image::load_from_memory_with_format(
            include_bytes!("../../../icons/icon.png"),
            ImageFormat::Png,
        )
        .ok()?
        .into_rgba8();
        let (width, height) = image.dimensions();
        Icon::from_rgba(image.into_raw(), width, height).ok()
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
const DESKTOP_IMAGE_PROXY_USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) ",
    "AppleWebKit/537.36 (KHTML, like Gecko) ",
    "Chrome/135.0.0.0 Safari/537.36 RSS-Reader/1.0"
);

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn desktop_image_proxy_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
async fn desktop_image_proxy_response(
    request: dioxus::desktop::wry::http::Request<Vec<u8>>,
) -> dioxus::desktop::wry::http::Response<Cow<'static, [u8]>> {
    let request_uri = request.uri().to_string();
    let parsed = url::Url::parse(&request_uri)
        .or_else(|_| {
            url::Url::parse(&format!(
                "{}:{}",
                crate::pages::reader_page::support::DESKTOP_IMAGE_PROXY_SCHEME,
                request_uri
            ))
        })
        .ok();
    let Some(parsed) = parsed else {
        return simple_proxy_response(400, "invalid proxy request uri");
    };

    let mut target = None;
    let mut referer = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "target" => target = Some(value.into_owned()),
            "referer" => referer = Some(value.into_owned()),
            _ => {}
        }
    }

    let Some(target) = target else {
        return simple_proxy_response(400, "missing target");
    };

    let Ok(target_url) = url::Url::parse(&target) else {
        return simple_proxy_response(400, "invalid target");
    };
    if !matches!(target_url.scheme(), "http" | "https") {
        return simple_proxy_response(400, "unsupported target scheme");
    }

    let mut request = desktop_image_proxy_client().get(target_url.clone()).header(
        reqwest::header::ACCEPT,
        "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
    );
    request = request
        .header(reqwest::header::USER_AGENT, DESKTOP_IMAGE_PROXY_USER_AGENT)
        .header(reqwest::header::ACCEPT_LANGUAGE, "zh-CN,zh;q=0.9,en;q=0.8");
    if let Some(referer) = referer {
        if let Ok(referer_url) = url::Url::parse(&referer) {
            request = request.header(reqwest::header::REFERER, referer_url.as_str());
        }
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(target = %target_url, error = %error, "桌面图片代理抓取失败");
            return simple_proxy_response(502, "proxy fetch failed");
        }
    };

    let status = response.status();
    if !status.is_success() {
        tracing::warn!(target = %target_url, status = status.as_u16(), "桌面图片代理返回非成功状态");
        return simple_proxy_response(502, "upstream image status error");
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();
    if !content_type.to_ascii_lowercase().starts_with("image/") {
        tracing::warn!(
            target = %target_url,
            content_type = %content_type,
            "桌面图片代理拿到的不是图片内容"
        );
        return simple_proxy_response(415, "upstream response is not an image");
    }

    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(target = %target_url, error = %error, "桌面图片代理读取响应失败");
            return simple_proxy_response(502, "proxy read failed");
        }
    };

    dioxus::desktop::wry::http::Response::builder()
        .status(200)
        .header("Content-Type", content_type)
        .header("Cache-Control", "no-store")
        .body(Cow::Owned(bytes.to_vec()))
        .unwrap_or_else(|_| simple_proxy_response(500, "proxy response build failed"))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn simple_proxy_response(
    status: u16,
    message: &'static str,
) -> dioxus::desktop::wry::http::Response<Cow<'static, [u8]>> {
    dioxus::desktop::wry::http::Response::builder()
        .status(status)
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(Cow::Borrowed(message.as_bytes()))
        .expect("valid proxy response")
}

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "android")))]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    #[tokio::test]
    async fn desktop_image_proxy_forwards_browser_like_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind proxy test server");
        let addr = listener.local_addr().expect("listener addr");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buf = [0_u8; 8192];
            let bytes_read = stream.read(&mut buf).expect("read request");
            let request = String::from_utf8_lossy(&buf[..bytes_read]).to_string();
            let response = concat!(
                "HTTP/1.1 200 OK\r\n",
                "Connection: close\r\n",
                "Content-Type: image/png\r\n",
                "Content-Length: 4\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).expect("write headers");
            stream.write_all(&[1_u8, 2_u8, 3_u8, 4_u8]).expect("write body");
            stream.flush().expect("flush response");
            request
        });

        let encode = |value: &str| -> String {
            url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
        };
        let uri = format!(
            "{}://fetch?target={}&referer={}",
            crate::pages::reader_page::support::DESKTOP_IMAGE_PROXY_SCHEME,
            encode(&format!("http://{addr}/image.png")),
            encode("https://blogs.nvidia.com/blog/example-post/")
        );
        let request = dioxus::desktop::wry::http::Request::builder()
            .uri(uri)
            .body(Vec::new())
            .expect("build request");

        let response = super::desktop_image_proxy_response(request).await;
        let upstream_request = server.join().expect("join server");
        let upstream_request_lower = upstream_request.to_ascii_lowercase();

        assert_eq!(response.status(), 200);
        assert!(upstream_request.contains("GET /image.png HTTP/1.1"));
        assert!(
            upstream_request_lower.contains(
                "accept: image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8"
            )
        );
        assert!(upstream_request_lower.contains("user-agent: mozilla/5.0"));
        assert!(upstream_request_lower.contains("accept-language: zh-cn,zh;q=0.9,en;q=0.8"));
        assert!(
            upstream_request_lower.contains("referer: https://blogs.nvidia.com/blog/example-post/")
        );
    }
}

#[cfg(target_os = "android")]
fn main() {
    use dioxus::mobile::{Config, WindowCloseBehaviour};
    use dioxus::prelude::LaunchBuilder;

    init_tracing();
    let config = Config::new().with_close_behaviour(WindowCloseBehaviour::WindowHides);
    LaunchBuilder::new().with_cfg(config).launch(app::App);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    init_tracing();
    dioxus::launch(app::App);
}
