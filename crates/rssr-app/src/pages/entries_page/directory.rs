//! Dioxus binding for the renderer-independent directory controller.
use std::{cell::Cell, rc::Rc, sync::Arc};

use dioxus::prelude::*;

use super::{
    intent::EntriesPageIntent, presenter::EntriesPagePresenter, session::EntriesPageSession,
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
pub(crate) struct DirectoryModel(
    pub(super) Arc<EntriesPagePresenter>,
    pub(super) Arc<Vec<Arc<rssr_domain::EntrySummary>>>,
);

impl PartialEq for DirectoryModel {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) && Arc::ptr_eq(&self.1, &other.1)
    }
}

impl std::ops::Deref for DirectoryModel {
    type Target = EntriesPagePresenter;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DirectoryModel {
    fn article_cards(&self, anchor: &str) -> &[super::groups::EntryCardRef] {
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
        Arc::ptr_eq(&left.0, &right.0)
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
        let before =
            DirectoryResetIdentity(DirectoryModel(Arc::new(presenter.clone()), Arc::default()));
        let mut after = presenter.clone();
        after.source_filter_options.push((1, "Feed".into(), "https://feed.test".into(), 11));
        assert!(
            before
                == DirectoryResetIdentity(DirectoryModel(Arc::new(after.clone()), Arc::default()))
        );
        after.current_page += 1;
        assert!(before != DirectoryResetIdentity(DirectoryModel(Arc::new(after), Arc::default())));
        let mut after = presenter;
        after.group_nav_items.push(EntryGroupNavItem {
            anchor_id: "new-group".into(),
            title: "New group".into(),
            subtitle: "1".into(),
            target_page: 1,
            is_active: false,
        });
        assert!(before != DirectoryResetIdentity(DirectoryModel(Arc::new(after), Arc::default())));
    }
}
