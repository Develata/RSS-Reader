// Rust 1.99 Clippy false-positive from async_trait macro expansion:
// https://github.com/rust-lang/rust-clippy/issues/17529
#![allow(clippy::double_must_use)]
pub mod application_adapters;
// The Pages demo uses the shared repositories in memory. Formal Web uses only HttpBackend;
// native hosts also provide file storage and outbound network adapters.
#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
pub mod composition;
#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
pub mod config_sync;
#[cfg(not(target_arch = "wasm32"))]
mod feed_body;
#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
mod feed_normalization;
#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
mod feed_structure;
#[cfg(not(target_arch = "wasm32"))]
pub mod fetch;
#[cfg(feature = "http-backend")]
pub mod http_backend;
pub mod local_backend;
#[cfg(not(target_arch = "wasm32"))]
mod remote_failure;
#[cfg(not(target_arch = "wasm32"))]
pub mod subscription_probe;
#[cfg(not(target_arch = "wasm32"))]
mod xml_declaration;
// 有意不加 target_arch 门控：正文 HTML 处理是纯字符串运算，Web 端同样需要。
pub mod html;
#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
pub mod opml;
// 服务器的出站访问策略；浏览器不发起受策略约束的请求。
#[cfg(not(target_arch = "wasm32"))]
pub mod outbound;
#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
pub mod parser;
pub mod platform_clock;
#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
pub mod store;
