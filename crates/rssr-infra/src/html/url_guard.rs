//! 正文图片归一化前的 URL 点段预检；避免路径回退反复扫描长 authority。

/// 包含编码后的点；普通图片地址通常远低于此限制。
pub(super) const MAX_URL_PATH_DOTS: usize = 64;

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

/// `raw` 在第一个 `?` 或 `#` 之前的点数，`%2e` 按一个点计（`url` 把它当作点段处理）。
/// 与 URL 解析器一样先忽略 TAB、LF、CR：解析器会删掉它们，`%2\ne` 在它眼里就是 `%2e`。
/// 这里数的是所有点（包括主机名与文件名里的），比真正的点段多，只会多拒绝、不会漏。
pub(super) fn url_path_dots(raw: &str) -> usize {
    let mut dots = 0_usize;
    // 已经读到的 `%2e` 前缀长度：0 无，1 读到 `%`，2 读到 `%2`。
    let mut encoded = 0_u8;
    for byte in raw.bytes().filter(|byte| !matches!(byte, b'\t' | b'\n' | b'\r')) {
        if matches!(byte, b'?' | b'#') {
            break;
        }
        encoded = match (encoded, byte) {
            (1, b'2') => 2,
            (2, b'e' | b'E') => {
                dots += 1;
                0
            }
            (_, b'%') => 1,
            _ => {
                if byte == b'.' {
                    dots += 1;
                }
                0
            }
        };
    }
    dots
}
