use dioxus::prelude::*;
use rssr_domain::FeedSummary;

use super::{bindings::FeedsPageBindings, queries::load_feeds_page_snapshot};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct FeedsPageSession {
    feed_url: Signal<String>,
    config_text: Signal<String>,
    opml_text: Signal<String>,
    pending_config_import: Signal<bool>,
    pending_delete_feed: Signal<Option<i64>>,
    feeds: Signal<Vec<FeedSummary>>,
    feed_count: Signal<usize>,
    entry_count: Signal<usize>,
    status: Signal<String>,
    status_tone: Signal<String>,
    bindings: FeedsPageBindings,
}

impl FeedsPageSession {
    pub(crate) fn new(reload_tick: Signal<u64>) -> Self {
        let feed_url = use_signal(String::new);
        let config_text = use_signal(String::new);
        let opml_text = use_signal(String::new);
        let pending_config_import = use_signal(|| false);
        let pending_delete_feed = use_signal(|| None::<i64>);
        let feeds = use_signal(Vec::<FeedSummary>::new);
        let feed_count = use_signal(|| 0_usize);
        let entry_count = use_signal(|| 0_usize);
        let status = use_signal(String::new);
        let status_tone = use_signal(|| "info".to_string());
        let bindings = FeedsPageBindings::new(
            feed_url,
            config_text,
            opml_text,
            pending_config_import,
            pending_delete_feed,
            reload_tick,
            feeds,
            feed_count,
            entry_count,
            status,
            status_tone,
        );

        Self {
            feed_url,
            config_text,
            opml_text,
            pending_config_import,
            pending_delete_feed,
            feeds,
            feed_count,
            entry_count,
            status,
            status_tone,
            bindings,
        }
    }

    pub(crate) fn bindings(self) -> FeedsPageBindings {
        self.bindings
    }

    pub(crate) fn feed_url(self) -> Signal<String> {
        self.feed_url
    }

    pub(crate) fn config_text(self) -> Signal<String> {
        self.config_text
    }

    pub(crate) fn opml_text(self) -> Signal<String> {
        self.opml_text
    }

    pub(crate) fn pending_config_import(self) -> Signal<bool> {
        self.pending_config_import
    }

    pub(crate) fn pending_delete_feed(self) -> Signal<Option<i64>> {
        self.pending_delete_feed
    }

    pub(crate) fn feeds(self) -> Signal<Vec<FeedSummary>> {
        self.feeds
    }

    pub(crate) fn feed_count(self) -> usize {
        (self.feed_count)()
    }

    pub(crate) fn entry_count(self) -> usize {
        (self.entry_count)()
    }

    pub(crate) fn status(self) -> String {
        (self.status)()
    }

    pub(crate) fn status_tone(self) -> String {
        (self.status_tone)()
    }

    pub(crate) async fn load_snapshot(self) {
        match load_feeds_page_snapshot().await {
            Ok(snapshot) => self.bindings.apply_snapshot(snapshot),
            Err(err) => self.bindings.set_status_error(err.to_string()),
        }
    }
}
