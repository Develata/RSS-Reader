//! Page-local result ownership and a single gate for settings mutations.
//! No settings values, tasks or platform capabilities are retained here.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SettingsWrite {
    Save,
    Download,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct EditingRevision {
    pub draft: u64,
    pub preset: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LoadRequest {
    revision: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct WriteRequest {
    revision: u64,
    kind: SettingsWrite,
    pub editing: EditingRevision,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct SettingsRequestLifecycle {
    revision: u64,
    editing: EditingRevision,
    pending: Option<SettingsWrite>,
}

impl SettingsRequestLifecycle {
    pub fn editing(&self) -> EditingRevision {
        self.editing
    }

    pub fn edit_draft(&mut self) {
        self.editing.draft = self.editing.draft.wrapping_add(1);
    }

    pub fn edit_preset(&mut self) {
        self.editing.preset = self.editing.preset.wrapping_add(1);
    }

    pub fn pending(&self) -> Option<SettingsWrite> {
        self.pending
    }

    pub fn begin_load(&mut self) -> Option<LoadRequest> {
        if self.pending.is_some() {
            return None;
        }
        self.revision = self.revision.wrapping_add(1);
        Some(LoadRequest { revision: self.revision })
    }

    pub fn complete_load(&mut self, request: LoadRequest) -> bool {
        if self.pending.is_some() || self.revision != request.revision {
            return false;
        }
        self.revision = self.revision.wrapping_add(1);
        true
    }

    pub fn begin_write(&mut self, kind: SettingsWrite) -> Option<WriteRequest> {
        if self.pending.is_some() {
            return None;
        }
        // A write supersedes older initial reads even if the write later fails.
        self.revision = self.revision.wrapping_add(1);
        self.pending = Some(kind);
        Some(WriteRequest { revision: self.revision, kind, editing: self.editing })
    }

    pub fn complete_write(&mut self, request: WriteRequest) -> bool {
        if self.revision != request.revision || self.pending != Some(request.kind) {
            return false;
        }
        self.pending = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_load_is_consumed_once_and_edits_do_not_hide_persisted_results() {
        let mut state = SettingsRequestLifecycle::default();
        let old = state.begin_load().unwrap();
        let new = state.begin_load().unwrap();
        state.edit_draft();
        state.edit_preset();
        assert!(!state.complete_load(old));
        assert!(state.complete_load(new));
        assert!(!state.complete_load(new));
        assert_eq!(state.editing(), EditingRevision { draft: 1, preset: 1 });
    }

    #[test]
    fn mutations_share_one_gate_and_permanently_supersede_old_loads() {
        for first in [SettingsWrite::Save, SettingsWrite::Download] {
            let mut state = SettingsRequestLifecycle::default();
            let load = state.begin_load().unwrap();
            let write = state.begin_write(first).unwrap();
            assert!(state.begin_load().is_none());
            assert!(state.begin_write(SettingsWrite::Save).is_none());
            assert!(state.begin_write(SettingsWrite::Download).is_none());
            assert!(!state.complete_load(load));
            assert!(state.complete_write(write));
            assert!(!state.complete_load(load));
            let next = state.begin_write(first).unwrap();
            assert!(!state.complete_write(write));
            assert_eq!(state.pending(), Some(first));
            assert!(state.complete_write(next));
        }
    }
}
