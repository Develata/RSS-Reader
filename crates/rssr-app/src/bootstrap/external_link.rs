//! Native Dioxus intercepts external navigation and opens the system handler.
//! Web uses a separate browsing context; the caller must set noopener noreferrer.
#[cfg(target_arch = "wasm32")]
pub(crate) const TARGET: &str = "_blank";
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const TARGET: &str = "_self";
