// Rust 1.99 Clippy false-positive from async_trait macro expansion:
// https://github.com/rust-lang/rust-clippy/issues/17529
#![allow(clippy::double_must_use)]
pub mod application_adapters;
pub mod composition;
#[cfg(not(target_arch = "wasm32"))]
pub mod config_sync;
mod feed_body;
mod feed_normalization;
#[cfg(not(target_arch = "wasm32"))]
pub mod fetch;
pub mod local_backend;
mod remote_failure;
pub mod subscription_probe;
mod xml_declaration;
// 有意不加 target_arch 门控：正文 HTML 处理是纯字符串运算，Web 端同样需要。
pub mod html;
pub mod opml;
// 服务器的出站访问策略；浏览器不发起受策略约束的请求。
#[cfg(not(target_arch = "wasm32"))]
pub mod outbound;
#[cfg(not(target_arch = "wasm32"))]
pub mod parser;
#[cfg(not(target_arch = "wasm32"))]
pub mod store;
