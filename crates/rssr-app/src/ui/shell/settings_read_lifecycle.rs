//! App-owned settings reads. The adapter supplies page ownership and the theme's stored
//! revision; this module owns neither settings values nor tasks, timers, or reactive state.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadTrigger {
    Explicit,
    Reread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReadMode {
    Start,
    ReadOnly,
}

#[derive(Debug)]
pub(super) struct SettingsRead {
    pub(super) mode: ReadMode,
    stored_revision: u64,
}

pub(super) struct SettingsReadLifecycle<Failure> {
    loading: bool,
    auto_refresh_started: bool,
    reread_wanted: bool,
    initial_failure: Option<Failure>,
    reread_failure: Option<(Failure, u64)>,
}

impl<Failure> Default for SettingsReadLifecycle<Failure> {
    fn default() -> Self {
        Self {
            loading: false,
            auto_refresh_started: false,
            reread_wanted: false,
            initial_failure: None,
            reread_failure: None,
        }
    }
}

impl<Failure> SettingsReadLifecycle<Failure> {
    pub(super) fn begin(
        &mut self,
        trigger: ReadTrigger,
        settings_page_open: bool,
        stored_revision: u64,
    ) -> Option<SettingsRead> {
        // The page owns automatic reads while open. Suppressed requests do not queue;
        // explicit retry/recovery still works even when the settings page is open.
        if trigger == ReadTrigger::Reread && settings_page_open {
            return None;
        }
        if self.loading {
            self.reread_wanted = true;
            return None;
        }
        self.loading = true;
        Some(SettingsRead {
            mode: if self.auto_refresh_started { ReadMode::ReadOnly } else { ReadMode::Start },
            stored_revision,
        })
    }

    /// An acknowledgement from the backend, independent of whether reading settings worked.
    pub(super) fn auto_refresh_started(&mut self) {
        self.auto_refresh_started = true;
    }

    /// A successful reply ends an earlier reread failure even when its settings are stale.
    /// Equal-value stores also advance the supplied revision and supersede this reply.
    pub(super) fn accept_loaded(&mut self, read: &SettingsRead, stored_revision: u64) -> bool {
        self.reread_failure = None;
        read.stored_revision == stored_revision
    }

    pub(super) fn failed(&mut self, failure: Failure, stored_revision: u64) {
        if stored_revision == 0 {
            self.initial_failure = Some(failure);
        } else {
            self.reread_failure = Some((failure, stored_revision));
        }
    }

    /// Called after the adapter has processed the ordered reply and stored accepted settings.
    /// A queued request is consumed once, then retried through `begin(Reread, ...)`: if the
    /// settings page opened meanwhile it is dropped, not postponed until the page closes.
    pub(super) fn finish(&mut self, _read: SettingsRead, stored_revision: u64) -> bool {
        self.loading = false;
        if stored_revision > 0 {
            self.initial_failure = None;
        }
        std::mem::take(&mut self.reread_wanted)
    }

    pub(super) fn is_loading(&self) -> bool {
        self.loading
    }

    pub(super) fn initial_failure(&self, has_stored_settings: bool) -> Option<&Failure> {
        self.initial_failure.as_ref().filter(|_| !has_stored_settings)
    }

    pub(super) fn needs_recovery(&self, has_stored_settings: bool) -> bool {
        has_stored_settings && self.initial_failure.is_some()
    }

    pub(super) fn reread_failure(&self, stored_revision: u64) -> Option<&Failure> {
        self.reread_failure
            .as_ref()
            .filter(|(_, applied)| *applied == stored_revision)
            .map(|(failure, _)| failure)
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadMode, ReadTrigger::*, SettingsReadLifecycle};

    #[test]
    fn backend_acknowledgement_not_settings_success_makes_later_reads_read_only() {
        for reached_backend in [false, true] {
            let mut lifecycle = SettingsReadLifecycle::default();
            let read = lifecycle.begin(Explicit, false, 0).unwrap();
            assert_eq!(read.mode, ReadMode::Start);
            if reached_backend {
                lifecycle.auto_refresh_started();
            }
            lifecycle.failed("offline", 0);
            assert!(!lifecycle.finish(read, 0));
            assert_eq!(lifecycle.initial_failure(false), Some(&"offline"));
            let retry = lifecycle.begin(Reread, false, 0).unwrap();
            assert_eq!(
                retry.mode,
                if reached_backend { ReadMode::ReadOnly } else { ReadMode::Start }
            );
            lifecycle.auto_refresh_started();
            assert!(lifecycle.accept_loaded(&retry, 0));
            assert!(!lifecycle.finish(retry, 1));
            assert_eq!(lifecycle.initial_failure(true), None);
            assert!(!lifecycle.needs_recovery(true));
            assert_eq!(lifecycle.begin(Explicit, true, 1).unwrap().mode, ReadMode::ReadOnly);
        }
    }

    #[test]
    fn many_requests_in_flight_make_one_follow_up_using_the_new_stored_revision() {
        let mut lifecycle = SettingsReadLifecycle::<&str>::default();
        let read = lifecycle.begin(Explicit, false, 0).unwrap();
        for trigger in [Reread, Explicit, Reread, Explicit] {
            assert!(lifecycle.begin(trigger, false, 0).is_none());
        }
        assert!(lifecycle.is_loading());
        lifecycle.auto_refresh_started();
        assert!(lifecycle.accept_loaded(&read, 0));
        assert!(lifecycle.finish(read, 1));
        let follow_up = lifecycle.begin(Reread, false, 1).unwrap();
        assert_eq!(follow_up.mode, ReadMode::ReadOnly);
        assert!(lifecycle.accept_loaded(&follow_up, 1));
        assert!(!lifecycle.finish(follow_up, 2));
        assert!(!lifecycle.is_loading());
    }

    #[test]
    fn open_settings_page_suppresses_automatic_requests_without_queuing_them() {
        let mut lifecycle = SettingsReadLifecycle::<&str>::default();
        assert!(lifecycle.begin(Reread, true, 0).is_none());
        assert!(!lifecycle.is_loading());
        let explicit = lifecycle.begin(Explicit, true, 0).unwrap();
        assert!(lifecycle.begin(Reread, true, 0).is_none());
        assert!(!lifecycle.finish(explicit, 0));
    }

    #[test]
    fn opening_settings_consumes_a_queued_retry_without_starting_another_read() {
        let mut lifecycle = SettingsReadLifecycle::<&str>::default();
        let read = lifecycle.begin(Explicit, false, 1).unwrap();
        assert!(lifecycle.begin(Explicit, true, 1).is_none());
        assert!(lifecycle.finish(read, 1));
        assert!(lifecycle.begin(Reread, true, 1).is_none());
        assert!(!lifecycle.is_loading());
        // A later explicit request still works, with no unconsumed follow-up behind it.
        let retry = lifecycle.begin(Explicit, true, 1).unwrap();
        assert!(!lifecycle.finish(retry, 1));
    }

    #[test]
    fn a_newer_store_rejects_the_reply_but_success_ends_the_previous_reread_failure() {
        let mut lifecycle = SettingsReadLifecycle::default();
        let failed = lifecycle.begin(Explicit, false, 1).unwrap();
        lifecycle.failed("offline", 1);
        lifecycle.finish(failed, 1);
        let read = lifecycle.begin(Explicit, false, 1).unwrap();
        assert_eq!(lifecycle.reread_failure(1), Some(&"offline"));
        assert!(!lifecycle.accept_loaded(&read, 2));
        lifecycle.finish(read, 2);
        assert_eq!(lifecycle.reread_failure(1), None);
        assert_eq!(lifecycle.reread_failure(2), None);
    }

    #[test]
    fn failure_belongs_to_the_revision_when_it_arrives_not_when_the_read_started() {
        let mut lifecycle = SettingsReadLifecycle::default();
        let read = lifecycle.begin(Explicit, false, 0).unwrap();
        lifecycle.failed("offline", 2);
        lifecycle.finish(read, 2);
        assert_eq!(lifecycle.initial_failure(false), None);
        assert_eq!(lifecycle.reread_failure(0), None);
        assert_eq!(lifecycle.reread_failure(2), Some(&"offline"));
        assert_eq!(lifecycle.reread_failure(3), None);
    }

    #[test]
    fn a_page_store_ends_the_initial_failure_even_if_recovery_read_fails() {
        let mut lifecycle = SettingsReadLifecycle::default();
        let start = lifecycle.begin(Explicit, false, 0).unwrap();
        lifecycle.failed("start", 0);
        lifecycle.finish(start, 0);
        assert_eq!(lifecycle.initial_failure(false), Some(&"start"));
        assert!(!lifecycle.needs_recovery(false));
        assert_eq!(
            lifecycle.initial_failure(true),
            None,
            "page store hides it before recovery ends"
        );
        assert!(lifecycle.needs_recovery(true));
        let recovery = lifecycle.begin(Explicit, true, 1).unwrap();
        lifecycle.failed("recovery", 1);
        lifecycle.finish(recovery, 1);
        assert_eq!(lifecycle.initial_failure(true), None);
        assert!(!lifecycle.needs_recovery(true));
        assert_eq!(lifecycle.reread_failure(1), Some(&"recovery"));
    }
}
