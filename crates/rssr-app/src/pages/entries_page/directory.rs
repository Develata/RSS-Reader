//! Directory model and Dioxus binding for the renderer-independent controller.
//! See `directory/README.md` for ownership, DOM contracts, and validation entrypoints.
use std::{cell::Cell, rc::Rc, sync::Arc};

use dioxus::prelude::*;

use super::{
    groups::EntryCardRef, intent::EntriesPageIntent, presenter::EntriesPagePresenter,
    session::EntriesPageSession,
};
use crate::ui::use_reactive_side_effect;

mod controller;
mod transport;
mod view;
mod virtual_articles;
mod window;
pub(super) use controller::DirectoryState;
pub(super) use view::DirectoryRail;

// Cheap view-prop identity. Presenter metadata can change without changing the
// directory; resetting interaction state requires the narrower identity below.
#[derive(Clone)]
pub(crate) struct DirectoryModel {
    presenter: Arc<EntriesPagePresenter>,
    entries: Arc<Vec<Arc<rssr_domain::EntrySummary>>>,
}

struct ResolvedDirectoryArticle<'a> {
    entry: &'a Arc<rssr_domain::EntrySummary>,
    page: u32,
}

impl PartialEq for DirectoryModel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.presenter, &other.presenter) && Arc::ptr_eq(&self.entries, &other.entries)
    }
}

impl std::ops::Deref for DirectoryModel {
    type Target = EntriesPagePresenter;
    fn deref(&self) -> &Self::Target {
        &self.presenter
    }
}

impl DirectoryModel {
    pub(super) fn new(
        presenter: Arc<EntriesPagePresenter>,
        entries: Arc<Vec<Arc<rssr_domain::EntrySummary>>>,
    ) -> Self {
        Self { presenter, entries }
    }

    /// Both small and windowed branches resolve against the same full snapshot.
    /// A stale index must never render or navigate to a different article.
    fn resolve_article(&self, card: &EntryCardRef) -> Option<ResolvedDirectoryArticle<'_>> {
        let entry = self.entries.get(card.index).filter(|entry| entry.id == card.id)?;
        Some(ResolvedDirectoryArticle {
            entry,
            page: (card.index / self.page_size.max(1)) as u32 + 1,
        })
    }

    fn article_cards(&self, anchor: &str) -> &[EntryCardRef] {
        self.directory_months
            .iter()
            .flat_map(|month| &month.dates)
            .find(|date| date.anchor_id == anchor)
            .map(|date| date.entry_cards.as_slice())
            .or_else(|| {
                self.directory_sources
                    .iter()
                    .flat_map(|source| &source.months)
                    .find(|month| month.anchor_id == anchor)
                    .map(|month| month.entry_cards.as_slice())
            })
            .unwrap_or_default()
    }
}

#[derive(Clone)]
struct DirectoryResetIdentity(DirectoryModel);

impl PartialEq for DirectoryResetIdentity {
    fn eq(&self, other: &Self) -> bool {
        let left = &self.0;
        let right = &other.0;
        Arc::ptr_eq(&left.presenter, &right.presenter)
            || (left.current_page == right.current_page
                && left.page_size == right.page_size
                && left.page_start == right.page_start
                && left.page_end == right.page_end
                && left.active_group_anchor == right.active_group_anchor
                && left.active_directory_anchor == right.active_directory_anchor
                && left.group_nav_items == right.group_nav_items
                && left.directory_months == right.directory_months
                && left.directory_sources == right.directory_sources)
    }
}

#[derive(Clone, Copy)]
pub(super) struct DirectoryStore(pub(super) Signal<DirectoryState>);

#[component]
pub(super) fn DirectoryBridge(
    model: DirectoryModel,
    context_key: String,
    session: EntriesPageSession,
) -> Element {
    let DirectoryStore(mut state) = use_context();
    use_reactive_side_effect((context_key, DirectoryResetIdentity(model)), move |(_, identity)| {
        let model = identity.0;
        state
            .write()
            .reset(model.active_group_anchor.clone(), model.active_directory_anchor.clone());
    });
    let bridge = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));
    let cleanup = bridge.clone();
    use_drop(move || {
        if let Some(eval) = cleanup.get() {
            let _ = eval.send(serde_json::Value::Null);
        }
    });
    use_future(move || {
        let bridge = bridge.clone();
        async move {
            let mut eval = document::eval(include_str!("directory/adapter.js"));
            bridge.set(Some(eval));
            // Within an epoch this copy is the only writer. The reset effect is
            // the only other writer and always advances the epoch. Publish only
            // render changes, not every observation/sequence acknowledgement.
            let mut current = state.peek().clone();
            while let Ok(fact) = transport::receive_fact(&mut eval).await {
                if current.epoch() != state.peek().epoch() {
                    current = state.peek().clone();
                }
                let reply = current.receive(&fact);
                if let Some(page) = reply.set_page {
                    session.dispatch(EntriesPageIntent::SetCurrentPage(page));
                }
                if reply.publish {
                    state.set(current.clone());
                }
                if eval.send(reply.command).is_err() {
                    break;
                }
            }
            // A closed transport must also release the adapter's listeners and
            // pending work. Ordinary malformed facts are handled inside recv.
            let _ = eval.send(serde_json::Value::Null);
            bridge.set(None);
        }
    });
    let epoch = state.read().epoch();
    rsx! { span { hidden: true, "data-directory-controller": "true", "data-directory-epoch": "{epoch}" } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_identity_ignores_feed_summary_but_tracks_page_and_directory_structure() {
        use crate::pages::entries_page::{
            groups::EntryGroupNavItem, presenter::EntriesPresenterInput, state::EntriesPageState,
        };
        let presenter = EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(
            &EntriesPageState::new(),
            None,
        ));
        let before = DirectoryResetIdentity(DirectoryModel::new(
            Arc::new(presenter.clone()),
            Arc::default(),
        ));
        let mut after = presenter.clone();
        after.source_filter_options.push((1, "Feed".into(), "https://feed.test".into(), 11));
        assert!(
            before
                == DirectoryResetIdentity(DirectoryModel::new(
                    Arc::new(after.clone()),
                    Arc::default()
                ))
        );
        after.current_page += 1;
        assert!(
            before != DirectoryResetIdentity(DirectoryModel::new(Arc::new(after), Arc::default()))
        );
        let mut after = presenter;
        after.group_nav_items.push(EntryGroupNavItem {
            anchor_id: "new-group".into(),
            title: "New group".into(),
            subtitle: "1".into(),
            target_page: 1,
            is_active: false,
        });
        assert!(
            before != DirectoryResetIdentity(DirectoryModel::new(Arc::new(after), Arc::default()))
        );
    }

    fn article_model(page_size: usize) -> DirectoryModel {
        use crate::pages::entries_page::{
            presenter::EntriesPresenterInput, state::EntriesPageState,
        };
        let mut presenter = EntriesPagePresenter::from_input(&EntriesPresenterInput::from_state(
            &EntriesPageState::new(),
            None,
        ));
        presenter.page_size = page_size;
        let entries = (1..=5)
            .map(|id| {
                Arc::new(rssr_domain::EntrySummary {
                    id,
                    feed_id: 1,
                    title: format!("Article {id}"),
                    feed_title: "Feed".into(),
                    published_at: None,
                    is_read: false,
                    is_starred: false,
                })
            })
            .collect();
        DirectoryModel::new(Arc::new(presenter), Arc::new(entries))
    }

    #[test]
    fn article_resolution_rejects_stale_and_out_of_bounds_references() {
        let model = article_model(2);
        assert!(model.resolve_article(&EntryCardRef { index: 0, id: 99 }).is_none());
        assert!(model.resolve_article(&EntryCardRef { index: 5, id: 6 }).is_none());
        let resolved = model.resolve_article(&EntryCardRef { index: 4, id: 5 }).unwrap();
        assert!(Arc::ptr_eq(resolved.entry, &model.entries[4]));
    }

    #[test]
    fn article_resolution_uses_global_index_for_page_boundaries() {
        let model = article_model(2);
        for (index, page) in [(0, 1), (1, 1), (2, 2), (4, 3)] {
            let resolved =
                model.resolve_article(&EntryCardRef { index, id: index as i64 + 1 }).unwrap();
            assert_eq!(resolved.page, page);
        }
        let zero_page_size = article_model(0);
        assert_eq!(
            zero_page_size.resolve_article(&EntryCardRef { index: 4, id: 5 }).unwrap().page,
            5
        );
    }
}
