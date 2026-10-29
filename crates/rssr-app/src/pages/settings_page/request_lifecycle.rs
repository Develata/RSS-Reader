//! Read/write ordering for one Settings page. The session owns commands and effects.
use rssr_domain::UserSettings;

/// A read's identity and the app's stored-settings revision when it was issued.
#[derive(Clone, Copy)]
pub(super) struct SettingsLoadRequest {
    written: u64,
    read: u64,
    applied: u64,
}

/// An admitted read; the session compares `applied` with the live app revision
/// before projecting it, since another page may have stored newer settings.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LoadedSettings {
    pub(super) settings: UserSettings,
    pub(super) applied: u64,
}

#[derive(Clone, Default, PartialEq)]
pub(super) struct SettingsRequestLifecycle {
    in_flight: bool,
    written: u64,
    reads: u64,
    held_load: Option<(LoadedSettings, u64)>,
}

impl SettingsRequestLifecycle {
    pub(super) fn begin_load(&mut self, applied: u64) -> SettingsLoadRequest {
        self.reads = self.reads.wrapping_add(1);
        SettingsLoadRequest { written: self.written, read: self.reads, applied }
    }

    /// Gate the entire reply, including errors/statuses, before publishing effects.
    pub(super) fn accepts(&self, request: SettingsLoadRequest) -> bool {
        self.written == request.written && self.reads == request.read
    }

    pub(super) fn receive_load(
        &mut self,
        request: SettingsLoadRequest,
        settings: UserSettings,
    ) -> Option<LoadedSettings> {
        if !self.accepts(request) {
            return None;
        }
        let loaded = LoadedSettings { settings, applied: request.applied };
        if self.in_flight {
            self.held_load = Some((loaded, request.read));
            None
        } else {
            Some(loaded)
        }
    }

    /// Saves and pulls share this admission gate. Rejection changes nothing.
    pub(super) fn begin_write(&mut self) -> bool {
        if self.in_flight {
            return false;
        }
        self.in_flight = true;
        true
    }

    /// Only a write carrying stored settings invalidates older reads.
    pub(super) fn settings_stored(&mut self) {
        self.written += 1;
        self.held_load = None;
    }

    /// A failed/no-op write releases its held read only if no later read began.
    /// A successful write already retired the held read via settings_stored.
    pub(super) fn end_write(&mut self) -> Option<LoadedSettings> {
        self.in_flight = false;
        self.held_load.take().and_then(|(loaded, read)| (read == self.reads).then_some(loaded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> UserSettings {
        UserSettings { entries_page_size: 80, ..Default::default() }
    }

    #[test]
    fn a_new_read_retires_old_replies_and_held_results_even_if_it_fails() {
        let mut lifecycle = SettingsRequestLifecycle::default();
        let old = lifecycle.begin_load(3);
        assert!(lifecycle.begin_write());
        assert!(lifecycle.receive_load(old, settings()).is_none());
        let latest = lifecycle.begin_load(3);
        assert!(!lifecycle.accepts(old), "old statuses are stale as well");
        assert!(lifecycle.accepts(latest));
        // The latest read fails: there is no replacement SettingsLoaded.
        assert!(lifecycle.end_write().is_none(), "the old held result cannot revive");
        assert!(lifecycle.receive_load(old, settings()).is_none());
    }

    #[test]
    fn a_successful_write_retires_reads_answered_before_or_after_its_receipt() {
        for answer_after_write in [false, true] {
            let mut lifecycle = SettingsRequestLifecycle::default();
            let read = lifecycle.begin_load(4);
            assert!(lifecycle.begin_write());
            if !answer_after_write {
                assert!(lifecycle.receive_load(read, settings()).is_none());
            }
            lifecycle.settings_stored();
            assert!(lifecycle.end_write().is_none());
            assert!(!lifecycle.accepts(read));
            if answer_after_write {
                assert!(lifecycle.receive_load(read, settings()).is_none());
            }
            let next = lifecycle.begin_load(5);
            assert_eq!(
                lifecycle.receive_load(next, settings()),
                Some(LoadedSettings { settings: settings(), applied: 5 })
            );
        }
    }

    #[test]
    fn a_failed_write_releases_current_held_read_once_with_its_original_app_revision() {
        let mut lifecycle = SettingsRequestLifecycle::default();
        let read = lifecycle.begin_load(7);
        assert!(lifecycle.begin_write());
        assert!(lifecycle.receive_load(read, settings()).is_none());
        assert!(!lifecycle.begin_write(), "rejection must not discard the held result");
        assert_eq!(
            lifecycle.end_write(),
            Some(LoadedSettings { settings: settings(), applied: 7 })
        );
        assert!(lifecycle.end_write().is_none());
        assert!(lifecycle.begin_write(), "the gate is reusable");
    }
}
