//! Titles in lists are bounded (v0.2 phase C, review C3-R1 F04). Titles come from feeds and are
//! otherwise unbounded, and a list repeats a subscription's title on every entry: a feed with a
//! title of megabytes would make one list request copy it thousands of times. Every store cuts
//! the titles it puts in a list the same way, before copying them; the reader shows the whole
//! title.

/// Characters of a title a list shows at most.
pub const MAX_LISTED_TITLE_CHARS: usize = 512;
/// What ends a title that was cut.
pub const LISTED_TITLE_MARK: &str = "…";

/// `title` as a list shows it.
///
/// Postcondition: the result is `title` when it has at most [`MAX_LISTED_TITLE_CHARS`]
/// characters (Unicode scalar values); otherwise its first `MAX_LISTED_TITLE_CHARS - 1`
/// characters followed by [`LISTED_TITLE_MARK`], so exactly `MAX_LISTED_TITLE_CHARS`. Only the
/// kept prefix is copied. SQLite's `length` and `substr` count the same characters, so the
/// native store applies the same rule in SQL.
pub fn listed_title(title: &str) -> String {
    if title.char_indices().nth(MAX_LISTED_TITLE_CHARS).is_none() {
        return title.to_owned();
    }
    let end = title
        .char_indices()
        .nth(MAX_LISTED_TITLE_CHARS - 1)
        .map_or(title.len(), |(index, _)| index);
    format!("{}{LISTED_TITLE_MARK}", &title[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_is_cut_at_its_character_bound_and_marked() {
        let at_bound = "题".repeat(MAX_LISTED_TITLE_CHARS);
        assert_eq!(listed_title(&at_bound), at_bound);
        let over = format!("{at_bound}x");
        let cut = listed_title(&over);
        assert_eq!(cut.chars().count(), MAX_LISTED_TITLE_CHARS);
        assert!(cut.ends_with(LISTED_TITLE_MARK));
        assert_eq!(
            cut.trim_end_matches(LISTED_TITLE_MARK),
            "题".repeat(MAX_LISTED_TITLE_CHARS - 1)
        );
        assert_eq!(listed_title(""), "");
    }
}
