//! Device-local time capability. Resolve the offset at the displayed instant, never at now.
use time::{OffsetDateTime, UtcOffset};

pub(crate) fn at(timestamp: OffsetDateTime) -> OffsetDateTime {
    timestamp.to_offset(offset_at(timestamp).unwrap_or(UtcOffset::UTC))
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn offset_at(timestamp: OffsetDateTime) -> Option<UtcOffset> {
    use chrono::{Local, TimeZone};
    // Chrono reads the OS timezone database with a thread-local cache, without libc's
    // single-thread restriction or modifying TZ. If the OS zone is unavailable it uses UTC.
    // Zero offsets (including that fallback) are explicitly displayed as UTC.
    let local = Local.timestamp_opt(timestamp.unix_timestamp(), 0).single()?;
    UtcOffset::from_whole_seconds(local.offset().local_minus_utc()).ok()
}

#[cfg(target_arch = "wasm32")]
fn offset_at(timestamp: OffsetDateTime) -> Option<UtcOffset> {
    // Direct synchronous JS binding: no per-card eval task or asynchronous host round trip.
    let date = js_sys::Date::new(&js_sys::Number::from(timestamp.unix_timestamp() as f64 * 1000.0));
    let seconds = -date.get_timezone_offset() * 60.0;
    if !seconds.is_finite() || seconds.abs() > 86_399.0 {
        return None;
    }
    UtcOffset::from_whole_seconds(seconds as i32).ok()
}

#[cfg(target_os = "android")]
fn offset_at(timestamp: OffsetDateTime) -> Option<UtcOffset> {
    use jni::{JavaVM, objects::JValue};
    let context = ndk_context::android_context();
    // The VM is owned by the Android host and lives for the process lifetime.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let result = env.with_local_frame(4, |env| -> jni::errors::Result<i32> {
        let zone = env
            .call_static_method("java/util/TimeZone", "getDefault", "()Ljava/util/TimeZone;", &[])?
            .l()?;
        env.call_method(
            zone,
            "getOffset",
            "(J)I",
            &[JValue::Long(timestamp.unix_timestamp() * 1000)],
        )?
        .i()
    });
    match result {
        Ok(milliseconds) => UtcOffset::from_whole_seconds(milliseconds / 1000).ok(),
        Err(error) => {
            let _ = env.exception_clear();
            tracing::warn!(%error, "设备时区不可用，使用 UTC");
            None
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), not(target_os = "android")))]
mod tests {
    use super::*;

    #[test]
    fn native_timezone_uses_instant_rules_across_threads() {
        const CHILD: &str = "RSSR_TIMEZONE_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            // DST starts and ends at these UTC instants in New York in 2026.
            for (raw, seconds) in [
                ("2026-03-08T06:59:59Z", -18000),
                ("2026-03-08T07:00:00Z", -14400),
                ("2026-11-01T05:59:59Z", -14400),
                ("2026-11-01T06:00:00Z", -18000),
            ] {
                let timestamp =
                    OffsetDateTime::parse(raw, &time::format_description::well_known::Rfc3339)
                        .unwrap();
                let handles: Vec<_> =
                    (0..4).map(|_| std::thread::spawn(move || at(timestamp))).collect();
                for handle in handles {
                    assert_eq!(handle.join().unwrap().offset().whole_seconds(), seconds);
                }
            }
            return;
        }
        // Only child-process environment is changed; never mutate the running process TZ.
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["native_timezone_uses_instant_rules_across_threads", "--nocapture"])
            .env("TZ", "America/New_York")
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
    }
}
