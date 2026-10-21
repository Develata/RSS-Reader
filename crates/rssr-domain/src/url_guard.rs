//! 解析不受信 URL 之前的廉价预检。
//!
//! `url` 2.5 处理每个 `..` 段时，会从已序列化的结果末尾反向找 `/`；路径已在根目录时这次查找
//! 要扫过整个 authority。一个带长用户名、成千上万个 `../` 的地址就能让一次解析平方级
//! （本机实测：规模翻倍，耗时约十倍）。feed、订阅地址、配置导入、Web 代理都会把远端给的字符串
//! 交给 `url`，这里提供统一的上限：点数与长度。

use url::Url;

/// 可解析地址里 `?`、`#` 之前最多含多少个点（含 `%2e`）。每个 `..` 段至少两个点，
/// 所以一次解析的反向扫描总量不超过这个数乘以地址长度。正常地址只有个位数的点。
pub const MAX_URL_PATH_DOTS: usize = 64;

/// 订阅、feed 链接等地址的最大字节数。远超浏览器与服务器的实际上限，只为挡住
/// 把整篇正文塞进一个地址、再被反复拼接复制的输入。
pub const MAX_UNTRUSTED_URL_BYTES: usize = 8 * 1024;

/// `raw` 在第一个 `?` 或 `#` 之前的点数，`%2e` 按一个点计（`url` 把它当作点段处理）。
/// 与 URL 解析器一样先忽略 TAB、LF、CR：解析器会删掉它们，`%2\ne` 在它眼里就是 `%2e`。
/// 这里数的是所有点（包括主机名与文件名里的），比真正的点段多，只会多拒绝、不会漏。
pub fn url_path_dots(raw: &str) -> usize {
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

/// 不受信的地址是否可以交给 `url` 解析：长度与点数都在上限之内。
pub fn untrusted_url_is_parseable(raw: &str) -> bool {
    raw.len() <= MAX_UNTRUSTED_URL_BYTES && url_path_dots(raw) <= MAX_URL_PATH_DOTS
}

/// 先预检再解析；超限按无法解析处理。
pub fn parse_untrusted_url(raw: &str) -> Option<Url> {
    untrusted_url_is_parseable(raw).then(|| Url::parse(raw).ok()).flatten()
}

/// 先预检再相对 `base` 解析。`base` 也计入长度上限：`join` 每次都会复制一遍它。
pub fn join_untrusted_url(base: &Url, raw: &str) -> Option<Url> {
    let fits = base.as_str().len() <= MAX_UNTRUSTED_URL_BYTES && untrusted_url_is_parseable(raw);
    fits.then(|| base.join(raw).ok()).flatten()
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::{MAX_UNTRUSTED_URL_BYTES, join_untrusted_url, parse_untrusted_url, url_path_dots};

    #[test]
    fn counts_dots_like_the_url_parser_sees_them() {
        assert_eq!(url_path_dots("https://e.test/a/../b.png?x=1.2.3"), 4);
        assert_eq!(url_path_dots("%2e%2E/.%2e/"), 4);
        assert_eq!(url_path_dots("%2\ne%2\r\te/"), 2);
        assert_eq!(url_path_dots("/a#b.c"), 0);
        assert_eq!(url_path_dots("%%2e"), 1);
    }

    #[test]
    fn dot_segments_parse_in_linear_time() {
        // 依赖 vendor/url 的补丁：原版 `url` 2.5.8 在根路径上处理每个 `..` 都反扫整个
        // authority，这个输入在 debug 下要几十秒（release 约 4 s）；打过补丁不到 1 s。
        // 补丁丢失时这个测试会明显变慢。
        let n = 1 << 18;
        let raw = format!("https://{}@e.test/{}", "a".repeat(n), "../".repeat(n));
        let url = Url::parse(&raw).unwrap();
        assert_eq!(url.path(), "/");
        assert_eq!(url.username().len(), n);
    }

    #[test]
    fn ordinary_urls_parse_and_abusive_ones_do_not() {
        assert!(parse_untrusted_url("https://example.com/feed.xml").is_some());
        let base = Url::parse("https://example.com/a/b/").unwrap();
        assert!(join_untrusted_url(&base, "../../c.xml").is_some());

        let dots = format!("https://{}@e.test/{}", "a".repeat(1 << 12), "../".repeat(1 << 12));
        assert!(parse_untrusted_url(&dots).is_none());
        let long = format!("https://e.test/{}", "a".repeat(MAX_UNTRUSTED_URL_BYTES));
        assert!(parse_untrusted_url(&long).is_none());
        let long_base = Url::parse(&long).unwrap();
        assert!(join_untrusted_url(&long_base, "x").is_none());
    }
}
