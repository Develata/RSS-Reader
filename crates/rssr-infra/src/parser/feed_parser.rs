pub use crate::feed_normalization::{ParsedEntry, ParsedFeed};

use crate::feed_normalization::parse_feed_xml;

#[derive(Debug, Clone, Default)]
pub struct FeedParser;

impl FeedParser {
    pub fn new() -> Self {
        Self
    }

    pub fn parse(&self, raw: &str) -> anyhow::Result<ParsedFeed> {
        parse_feed_xml(raw, true)
    }
}
