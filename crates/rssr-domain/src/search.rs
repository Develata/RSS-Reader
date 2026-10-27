//! 标题搜索的匹配规则。原生端（SQLite）与 Web 端（浏览器内存）都用这里的折叠，同一个搜索词
//! 在两端匹配到同样的文章（审计 D6）。

/// 搜索用的大小写折叠：Unicode 小写（`str::to_lowercase`）。不做完整的大小写折叠，例如 `ß`
/// 不等于 `ss`。
pub fn fold_for_search(text: &str) -> String {
    text.to_lowercase()
}

/// 标题是否包含搜索词，按字面子串匹配。`folded_search` 必须是 [`fold_for_search`] 的结果。
pub fn title_contains(title: &str, folded_search: &str) -> bool {
    fold_for_search(title).contains(folded_search)
}

#[cfg(test)]
mod tests {
    use super::{fold_for_search, title_contains};

    #[test]
    fn titles_match_regardless_of_case_in_any_script() {
        for (title, search) in [
            ("Roche Scales NVIDIA AI Factories", "SCA"),
            ("ÉCOLE de Paris", "école"),
            ("Привет, МИР", "мир"),
            ("ΣΟΦΙΑ", "σοφια"),
            ("中文标题", "标题"),
        ] {
            assert!(title_contains(title, &fold_for_search(search)), "{title} / {search}");
        }
        assert!(!title_contains("Roche Scales NVIDIA AI Factories", &fold_for_search("xyz")));
        assert!(title_contains("100% pure", &fold_for_search("0% p")), "searches are literal");
    }
}
