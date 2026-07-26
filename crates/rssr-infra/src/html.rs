//! 正文 HTML 处理。
//!
//! 这个模块**不能**被 `#[cfg(not(target_arch = "wasm32"))]` 门控。里面全是纯字符串处理，
//! 不含任何 I/O，Web 端同样需要它：这些代码原本埋在 `fetch::client::image_html` 下，而整个
//! `fetch` 模块是原生专用的，导致 Web 阅读页拿不到懒加载图片地址的归一化，同一篇文章在
//! 桌面端图片正常、在 Web 端裂图。
//!
//! 阅读页（`rssr-app`）曾经自带一份功能重叠的标签解析器 + HTML 实体解码，与这里的实现并行
//! 演进。正文的解析、归一化与消毒属于基础设施职责，页面层只负责显示，所以统一放在这里。

mod live_display;
mod reader;

pub use live_display::normalize_html_for_live_display;
pub use reader::{looks_like_html_fragment, sanitize_reader_html};

// 正文图片本地化只在原生端做，Web 端没有对应能力。
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use live_display::{LocalizableImageDocument, normalize_image_content_type};
