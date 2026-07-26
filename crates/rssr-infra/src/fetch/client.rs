mod body_asset_localizer;
mod feed_http;
mod feed_response;

pub use body_asset_localizer::BodyAssetLocalizer;
pub use feed_http::{FetchClient, FetchRequest, FetchResult, HttpMetadata};

// 正文 HTML 归一化本身与抓取无关，实现已移到不受平台门控的 `crate::html`；
// 这里保留旧路径的再导出，避免调用方被迫跟着改。
pub use crate::html::normalize_html_for_live_display;
