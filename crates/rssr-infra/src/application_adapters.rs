#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
mod non_refresh;
#[cfg(not(target_arch = "wasm32"))]
mod refresh;

#[cfg(any(not(target_arch = "wasm32"), feature = "memory-store"))]
pub use non_refresh::InfraOpmlCodec;
#[cfg(not(target_arch = "wasm32"))]
pub use refresh::InfraFeedRefreshSource;
