use serde::{Deserialize, Serialize};

use crate::entry::{ReadFilter, StarredFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryGroupingPreference {
    #[default]
    Time,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EntriesWorkspaceState {
    pub grouping_mode: EntryGroupingPreference,
    pub show_archived: bool,
    pub read_filter: ReadFilter,
    pub starred_filter: StarredFilter,
    pub selected_feed_urls: Vec<String>,
}

impl Default for EntriesWorkspaceState {
    fn default() -> Self {
        Self {
            grouping_mode: EntryGroupingPreference::Time,
            show_archived: false,
            read_filter: ReadFilter::All,
            starred_filter: StarredFilter::All,
            selected_feed_urls: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppStateSnapshot {
    pub last_opened_feed_id: Option<i64>,
    pub entries_workspace: EntriesWorkspaceState,
}

impl AppStateSnapshot {
    pub fn replace_feed_url(&mut self, old_url: &str, new_url: &str) {
        for url in &mut self.entries_workspace.selected_feed_urls {
            if url == old_url {
                *url = new_url.to_string();
            }
        }
        self.entries_workspace.selected_feed_urls.sort();
        self.entries_workspace.selected_feed_urls.dedup();
    }
}
