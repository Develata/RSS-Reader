// Rust 1.99 Clippy false-positive from async_trait macro expansion:
// https://github.com/rust-lang/rust-clippy/issues/17529
#![allow(clippy::double_must_use)]
pub mod application_adapters;
// On wasm, composing the use cases, fetching and parsing feeds and probing subscriptions serve only
// the library kept in the browser (the Pages demo, `browser-store`); the formal Web page asks the
// server (`http-backend`).
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
pub mod composition;
#[cfg(not(target_arch = "wasm32"))]
pub mod config_sync;
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
mod feed_body;
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
mod feed_normalization;
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
mod feed_structure;
#[cfg(not(target_arch = "wasm32"))]
pub mod fetch;
#[cfg(feature = "http-backend")]
pub mod http_backend;
pub mod local_backend;
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
mod remote_failure;
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
pub mod subscription_probe;
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
mod xml_declaration;
// 有意不加 target_arch 门控：正文 HTML 处理是纯字符串运算，Web 端同样需要。
pub mod html;
#[cfg(any(not(target_arch = "wasm32"), feature = "browser-store"))]
pub mod opml;
// 服务器的出站访问策略；浏览器不发起受策略约束的请求。
#[cfg(not(target_arch = "wasm32"))]
pub mod outbound;
#[cfg(not(target_arch = "wasm32"))]
pub mod parser;
#[cfg(not(target_arch = "wasm32"))]
pub mod store;
