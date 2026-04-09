use crate::ui::AppShellState;
use time::OffsetDateTime;

use super::{
    presenter::EntriesPagePresenter, session::EntriesPageSession, state::EntriesPageState,
};

pub(crate) struct EntriesPageFacade {
    pub(crate) ui: AppShellState,
    pub(crate) session: EntriesPageSession,
    pub(crate) snapshot: EntriesPageState,
    pub(crate) presenter: EntriesPagePresenter,
}

impl EntriesPageFacade {
    pub(crate) fn new(
        ui: AppShellState,
        session: EntriesPageSession,
        snapshot: EntriesPageState,
        now: OffsetDateTime,
    ) -> Self {
        let presenter = session.presenter(now);
        Self { ui, session, snapshot, presenter }
    }
}
