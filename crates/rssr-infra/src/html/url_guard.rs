//! 正文图片归一化前的 URL 点段预检；避免路径回退反复扫描长 authority。

/// 包含编码后的点；普通图片地址通常远低于此限制。
pub(super) use rssr_domain::{MAX_URL_PATH_DOTS, url_path_dots};

/// 在 ammonia 自己解析 URL 前检查解码后的属性；它的 scheme 检查早于自定义属性回调。
/// srcset 按浏览器候选分别计数，不能把一组正常图片地址的点相加。
pub(super) fn attributes_are_bounded(attrs: &[html5ever::Attribute]) -> bool {
    attrs.iter().all(|attr| match attr.name.local.as_ref() {
        "srcset" => super::live_display::srcset_candidates(&attr.value)
            .all(|candidate| bounded_attribute_url(candidate.url)),
        "href" | "src" | "action" | "formaction" | "ping" | "poster" | "cite" | "data" => {
            bounded_attribute_url(&attr.value)
        }
        _ => true,
    })
}

fn bounded_attribute_url(raw: &str) -> bool {
    // Localized SVG data URLs can contain many decimal points. `data:image/` starts an
    // opaque path, so it never runs the hierarchical `..` path-shortening algorithm.
    raw.trim_ascii_start()
        .get(..11)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:image/"))
        || url_path_dots(raw) <= MAX_URL_PATH_DOTS
}
