//! What the user reads about WebDAV automatic synchronization: the settings page's last result
//! and the shell's notice after the start (`docs/design/webdav-auto-sync.md`).

use rssr_application::{RemoteSyncOutcome, RemoteSyncReport, RemoteSyncTrigger};

/// What a synchronization did, as one sentence.
pub(crate) fn outcome_text(outcome: &RemoteSyncOutcome) -> String {
    match outcome {
        RemoteSyncOutcome::UpToDate => "两边配置一致，无需传输。".to_string(),
        RemoteSyncOutcome::Pushed { exported_feed_count } => {
            format!("已上传本机配置：{exported_feed_count} 个订阅。")
        }
        RemoteSyncOutcome::Pulled { imported_feed_count, removed_feed_count } => {
            if *removed_feed_count == 0 {
                format!("已下载远端配置：{imported_feed_count} 个订阅。")
            } else {
                format!(
                    "已下载远端配置：{imported_feed_count} 个订阅，移除 {removed_feed_count} 个本机订阅。"
                )
            }
        }
        RemoteSyncOutcome::NeedsChoice => {
            "首次同步时本机与远端配置不同，请手动上传或下载一次。".to_string()
        }
        RemoteSyncOutcome::Conflict => {
            "本机和远端都改过配置，未自动同步；请手动选择上传或下载。".to_string()
        }
        RemoteSyncOutcome::Failed { message } => format!("同步失败：{message}"),
    }
}

/// Whether the user has to act (choose a direction) or should know it failed.
pub(crate) fn needs_attention(outcome: &RemoteSyncOutcome) -> bool {
    matches!(
        outcome,
        RemoteSyncOutcome::NeedsChoice
            | RemoteSyncOutcome::Conflict
            | RemoteSyncOutcome::Failed { .. }
    )
}

fn trigger_text(trigger: RemoteSyncTrigger) -> &'static str {
    match trigger {
        RemoteSyncTrigger::Startup => "启动后",
        RemoteSyncTrigger::AfterChange => "修改配置后",
        RemoteSyncTrigger::Manual => "手动",
    }
}

/// The settings page's line about the last synchronization.
pub(crate) fn report_text(report: &RemoteSyncReport) -> String {
    let at = crate::datetime::format_datetime(Some(report.at)).unwrap_or_default();
    format!("最近一次（{}，{at}）：{}", trigger_text(report.trigger), outcome_text(&report.outcome))
}

/// The shell's notice after the synchronization at start: only a download or what needs the
/// user is worth interrupting for.
pub(crate) fn startup_notice(report: &RemoteSyncReport) -> Option<(String, &'static str)> {
    match &report.outcome {
        RemoteSyncOutcome::Pulled { .. } => {
            Some((format!("WebDAV 自动同步：{}", outcome_text(&report.outcome)), "info"))
        }
        outcome if needs_attention(outcome) => {
            Some((format!("WebDAV 自动同步：{}", outcome_text(outcome)), "error"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(outcome: RemoteSyncOutcome) -> RemoteSyncReport {
        RemoteSyncReport {
            at: time::OffsetDateTime::UNIX_EPOCH,
            trigger: RemoteSyncTrigger::Startup,
            outcome,
        }
    }

    #[test]
    fn the_start_interrupts_only_for_a_download_or_what_needs_the_user() {
        assert_eq!(startup_notice(&report(RemoteSyncOutcome::UpToDate)), None);
        assert_eq!(
            startup_notice(&report(RemoteSyncOutcome::Pushed { exported_feed_count: 2 })),
            None
        );
        let pulled = RemoteSyncOutcome::Pulled { imported_feed_count: 3, removed_feed_count: 1 };
        assert_eq!(startup_notice(&report(pulled)).map(|(_, tone)| tone), Some("info"));
        for outcome in [
            RemoteSyncOutcome::NeedsChoice,
            RemoteSyncOutcome::Conflict,
            RemoteSyncOutcome::Failed { message: "无法访问远端".into() },
        ] {
            assert_eq!(startup_notice(&report(outcome)).map(|(_, tone)| tone), Some("error"));
        }
    }

    #[test]
    fn a_report_names_its_trigger_and_result() {
        let text = report_text(&report(RemoteSyncOutcome::Failed { message: "离线".into() }));
        assert!(text.starts_with("最近一次（启动后，"), "{text}");
        assert!(text.ends_with("同步失败：离线"), "{text}");
    }
}
