use dioxus::prelude::*;
use time::OffsetDateTime;

#[cfg(target_arch = "wasm32")]
pub(super) fn current_time_utc() -> OffsetDateTime {
    let millis = js_sys::Date::now();
    let seconds = (millis / 1_000.0).floor() as i64;
    let nanos = ((millis % 1_000.0) * 1_000_000.0).round() as i64;
    OffsetDateTime::from_unix_timestamp(seconds).expect("valid unix timestamp")
        + time::Duration::nanoseconds(nanos)
}

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn current_time_utc() -> OffsetDateTime {
    OffsetDateTime::now_utc()
}

/// 只在首次挂载与 `trigger` 变化时采样时钟；同一个 `trigger` 下的重复渲染拿到同一时刻。
///
/// 列表查询的归档分界取自这个时刻。渲染时直接读时钟会让 `EntryQuery` 每跨一秒就变化，
/// 每次重渲染都重发列表与计数查询（审计 A4）。
pub(super) fn use_sampled_clock<T>(trigger: T, sample: fn() -> OffsetDateTime) -> OffsetDateTime
where
    T: Clone + PartialEq + 'static,
{
    let sampled = use_memo(use_reactive!(|(trigger)| {
        let _ = &trigger;
        sample()
    }));
    sampled()
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        thread_local,
    };

    use dioxus::prelude::*;
    use time::OffsetDateTime;

    use super::use_sampled_clock;

    // VirtualDom 在测试线程上同步渲染，线程局部量即可隔离并行测试。
    thread_local! {
        static FAKE_NOW: Cell<i64> = const { Cell::new(1_000) };
        static SEEN: RefCell<Vec<i64>> = const { RefCell::new(Vec::new()) };
    }

    fn fake_now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(FAKE_NOW.get()).expect("timestamp")
    }

    #[derive(Clone, Copy)]
    struct Probe {
        trigger: Signal<u64>,
        unrelated: Signal<u64>,
    }

    fn harness() -> Element {
        let trigger = use_signal(|| 0);
        let unrelated = use_signal(|| 0);
        use_context_provider(|| Probe { trigger, unrelated });
        let _ = unrelated();
        let now = use_sampled_clock(trigger(), fake_now);
        SEEN.with_borrow_mut(|seen| seen.push(now.unix_timestamp()));
        rsx! {}
    }

    #[test]
    fn rerenders_reuse_the_sample_until_the_trigger_changes() {
        let mut dom = VirtualDom::new(harness);
        dom.rebuild_in_place();
        let Probe { mut trigger, mut unrelated } = dom.in_scope(ScopeId::APP, consume_context);

        FAKE_NOW.set(2_000);
        dom.in_scope(ScopeId::APP, || unrelated += 1);
        dom.render_immediate_to_vec();
        dom.in_scope(ScopeId::APP, || trigger += 1);
        dom.render_immediate_to_vec();

        assert_eq!(SEEN.take(), [1_000, 1_000, 2_000]);
    }
}
