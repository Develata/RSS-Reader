use dioxus::prelude::*;

/// 本地门禁通过之后进入阅读器。
///
/// Web 端整页重载而不是就地切状态：解锁会换掉 `localStorage` 里可读的数据集，
/// 重载是让所有已挂载的组件都从新数据重新起一遍最省事、也最不容易漏的做法。
/// 重载失败（例如被浏览器策略拦下）才回落到就地切换。
#[cfg(target_arch = "wasm32")]
pub(crate) fn complete_web_auth_transition(on_authenticated: EventHandler<()>) {
    if let Some(window) = web_sys::window()
        && window.location().reload().is_ok()
    {
        return;
    }

    on_authenticated.call(());
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn complete_web_auth_transition(on_authenticated: EventHandler<()>) {
    on_authenticated.call(());
}
