pub mod application_adapters;
pub mod composition;
#[cfg(not(target_arch = "wasm32"))]
pub mod config_sync;
#[cfg(not(target_arch = "wasm32"))]
pub mod db;
mod feed_normalization;
#[cfg(not(target_arch = "wasm32"))]
pub mod fetch;
// 有意不加 target_arch 门控：正文 HTML 处理是纯字符串运算，Web 端同样需要。
pub mod html;
pub mod opml;
#[cfg(not(target_arch = "wasm32"))]
pub mod parser;
