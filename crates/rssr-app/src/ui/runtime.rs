use crate::{
    bootstrap::AppServices,
    pages::entries_page::intent::EntriesPageIntent,
    pages::feeds_page::intent::{FeedsPageIntent, FeedsPageSnapshot},
    pages::reader_page::{
        intent::ReaderPageIntent,
        state::ReaderPageLoadedContent,
        support::{ReaderBody, format_reader_datetime_utc, select_reader_body},
    },
    pages::settings_page::intent::SettingsPageIntent,
    router::AppRoute,
    ui::{
        commands::UiCommand,
        snapshot::{AuthenticatedShellSnapshot, StartupRouteSnapshot, UiIntent},
    },
};
use anyhow::Context;
use rssr_domain::EntryQuery;
use rssr_domain::StartupView;

pub(crate) struct UiRuntimeOutcome {
    pub(crate) intents: Vec<UiIntent>,
}

impl UiRuntimeOutcome {
    fn single(intent: UiIntent) -> Self {
        Self { intents: vec![intent] }
    }
}

pub(crate) async fn execute_ui_command(command: UiCommand) -> UiRuntimeOutcome {
    match command {
        UiCommand::LoadAuthenticatedShell => match AppServices::shared().await {
            Ok(services) => {
                let settings = match services.load_settings().await {
                    Ok(settings) => settings,
                    Err(err) => return status_error(format!("读取设置失败：{err}")),
                };
                services.ensure_auto_refresh_started();
                UiRuntimeOutcome::single(UiIntent::AuthenticatedShellLoaded(
                    AuthenticatedShellSnapshot { settings },
                ))
            }
            Err(err) => status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::ResolveStartupRoute => match AppServices::shared().await {
            Ok(services) => {
                let settings = match services.load_settings().await {
                    Ok(settings) => settings,
                    Err(err) => return resolve_with_fallback(format!("读取设置失败：{err}")),
                };

                let route = match settings.startup_view {
                    StartupView::All => AppRoute::EntriesPage {},
                    StartupView::LastFeed => {
                        let last_feed_id = services.load_last_opened_feed_id().await.ok().flatten();
                        let feed_exists = match last_feed_id {
                            Some(feed_id) => services
                                .list_feeds()
                                .await
                                .map(|feeds| feeds.iter().any(|feed| feed.id == feed_id))
                                .unwrap_or(false),
                            None => false,
                        };

                        if let Some(feed_id) = last_feed_id.filter(|_| feed_exists) {
                            AppRoute::FeedEntriesPage { feed_id }
                        } else {
                            AppRoute::EntriesPage {}
                        }
                    }
                };

                UiRuntimeOutcome::single(UiIntent::StartupRouteResolved(StartupRouteSnapshot {
                    route,
                }))
            }
            Err(err) => resolve_with_fallback(format!("初始化应用失败：{err}")),
        },
        UiCommand::EntriesBootstrap { feed_id, load_preferences, load_feeds } => {
            match AppServices::shared().await {
                Ok(services) => {
                    let mut intents = Vec::new();

                    if let Some(feed_id) = feed_id {
                        let _ = services.remember_last_opened_feed_id(feed_id).await;
                    }

                    if load_preferences {
                        match services.load_settings().await {
                            Ok(settings) => intents.push(UiIntent::EntriesPage(
                                EntriesPageIntent::ApplyLoadedSettings(settings),
                            )),
                            Err(err) => {
                                return entries_status_error(format!("读取设置失败：{err}"));
                            }
                        }
                    }

                    if load_feeds {
                        match services.list_feeds().await {
                            Ok(feeds) => intents
                                .push(UiIntent::EntriesPage(EntriesPageIntent::SetFeeds(feeds))),
                            Err(err) => {
                                return entries_status_error(format!("读取订阅失败：{err}"));
                            }
                        }
                    }

                    UiRuntimeOutcome { intents }
                }
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        UiCommand::EntriesLoadEntries { query } => match AppServices::shared().await {
            Ok(services) => match services.list_entries(&query).await {
                Ok(entries) => UiRuntimeOutcome {
                    intents: vec![UiIntent::EntriesPage(EntriesPageIntent::SetEntries(entries))],
                },
                Err(err) => entries_status_error(format!("读取文章失败：{err}")),
            },
            Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::EntriesToggleRead { entry_id, entry_title, currently_read } => {
            match AppServices::shared().await {
                Ok(services) => match services.set_read(entry_id, !currently_read).await {
                    Ok(()) => entries_intents(vec![
                        EntriesPageIntent::SetStatus {
                            message: format!(
                                "已将《{}》{}。",
                                entry_title,
                                if currently_read { "标记为未读" } else { "标记为已读" }
                            ),
                            tone: "info".to_string(),
                        },
                        EntriesPageIntent::BumpReload,
                    ]),
                    Err(err) => entries_status_error(format!("更新已读状态失败：{err}")),
                },
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        UiCommand::EntriesToggleStarred { entry_id, entry_title, currently_starred } => {
            match AppServices::shared().await {
                Ok(services) => match services.set_starred(entry_id, !currently_starred).await {
                    Ok(()) => entries_intents(vec![
                        EntriesPageIntent::SetStatus {
                            message: format!(
                                "已{}《{}》。",
                                if currently_starred { "取消收藏" } else { "收藏" },
                                entry_title
                            ),
                            tone: "info".to_string(),
                        },
                        EntriesPageIntent::BumpReload,
                    ]),
                    Err(err) => entries_status_error(format!("更新收藏状态失败：{err}")),
                },
                Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
            }
        }
        UiCommand::EntriesSaveBrowsingPreferences {
            grouping_mode,
            show_archived,
            read_filter,
            starred_filter,
            selected_feed_urls,
        } => match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(mut settings) => {
                    let changed = settings.entry_grouping_mode != grouping_mode
                        || settings.show_archived_entries != show_archived
                        || settings.entry_read_filter != read_filter
                        || settings.entry_starred_filter != starred_filter
                        || settings.entry_filtered_feed_urls != selected_feed_urls;

                    if !changed {
                        return UiRuntimeOutcome { intents: Vec::new() };
                    }

                    settings.entry_grouping_mode = grouping_mode;
                    settings.show_archived_entries = show_archived;
                    settings.entry_read_filter = read_filter;
                    settings.entry_starred_filter = starred_filter;
                    settings.entry_filtered_feed_urls = selected_feed_urls;

                    match services.save_settings(&settings).await {
                        Ok(()) => UiRuntimeOutcome { intents: Vec::new() },
                        Err(err) => entries_status_error(format!("保存文章页偏好失败：{err}")),
                    }
                }
                Err(err) => entries_status_error(format!("读取文章页偏好失败：{err}")),
            },
            Err(err) => entries_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::ReaderLoadEntry { entry_id } => match AppServices::shared().await {
            Ok(services) => {
                let mut intents = vec![UiIntent::ReaderPage(ReaderPageIntent::BeginLoading)];
                match services.get_entry(entry_id).await {
                    Ok(Some(entry)) => {
                        let (body_html, body_text) = match select_reader_body(
                            entry.content_html,
                            entry.content_text,
                            entry.summary,
                        ) {
                            ReaderBody::Html(html) => (Some(html), String::new()),
                            ReaderBody::Text(text) => (None, text),
                        };

                        let content = ReaderPageLoadedContent {
                            title: entry.title,
                            body_text,
                            body_html,
                            source: entry
                                .url
                                .map(|url| url.to_string())
                                .unwrap_or_else(|| "无原文链接".to_string()),
                            published_at: format_reader_datetime_utc(entry.published_at)
                                .unwrap_or_else(|| "未知发布时间".to_string()),
                            is_read: entry.is_read,
                            is_starred: entry.is_starred,
                            navigation_state: services
                                .reader_navigation(entry_id)
                                .await
                                .unwrap_or_default(),
                        };
                        intents.push(UiIntent::ReaderPage(ReaderPageIntent::ApplyLoadedContent(
                            content,
                        )));
                    }
                    Ok(None) => intents.push(UiIntent::ReaderPage(ReaderPageIntent::SetError(
                        Some("文章不存在".to_string()),
                    ))),
                    Err(err) => intents.push(UiIntent::ReaderPage(ReaderPageIntent::SetError(
                        Some(err.to_string()),
                    ))),
                }
                UiRuntimeOutcome { intents }
            }
            Err(err) => reader_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::ReaderToggleRead { entry_id, currently_read, via_shortcut } => {
            match AppServices::shared().await {
                Ok(services) => match services.set_read(entry_id, !currently_read).await {
                    Ok(()) => reader_intents(vec![
                        ReaderPageIntent::SetStatus {
                            message: if via_shortcut {
                                if currently_read {
                                    "已通过快捷键标记为未读。".to_string()
                                } else {
                                    "已通过快捷键标记为已读。".to_string()
                                }
                            } else if currently_read {
                                "已将当前文章标记为未读。".to_string()
                            } else {
                                "已将当前文章标记为已读。".to_string()
                            },
                            tone: "info".to_string(),
                        },
                        ReaderPageIntent::BumpReload,
                    ]),
                    Err(err) => reader_status_error(format!("更新已读状态失败：{err}")),
                },
                Err(err) => reader_status_error(format!("初始化应用失败：{err}")),
            }
        }
        UiCommand::ReaderToggleStarred { entry_id, currently_starred, via_shortcut } => {
            match AppServices::shared().await {
                Ok(services) => match services.set_starred(entry_id, !currently_starred).await {
                    Ok(()) => reader_intents(vec![
                        ReaderPageIntent::SetStatus {
                            message: if via_shortcut {
                                if currently_starred {
                                    "已通过快捷键取消收藏。".to_string()
                                } else {
                                    "已通过快捷键收藏文章。".to_string()
                                }
                            } else if currently_starred {
                                "已取消收藏当前文章。".to_string()
                            } else {
                                "已收藏当前文章。".to_string()
                            },
                            tone: "info".to_string(),
                        },
                        ReaderPageIntent::BumpReload,
                    ]),
                    Err(err) => reader_status_error(format!("更新收藏状态失败：{err}")),
                },
                Err(err) => reader_status_error(format!("初始化应用失败：{err}")),
            }
        }
        UiCommand::FeedsLoadSnapshot => match AppServices::shared().await {
            Ok(services) => {
                let result: anyhow::Result<FeedsPageSnapshot> = async {
                    let feeds = services.list_feeds().await.context("读取订阅失败")?;
                    let entry_count = services
                        .list_entries(&EntryQuery::default())
                        .await
                        .context("读取文章统计失败")?
                        .len();
                    Ok(FeedsPageSnapshot { feed_count: feeds.len(), entry_count, feeds })
                }
                .await;
                feeds_intents(vec![FeedsPageIntent::SnapshotLoaded(
                    result.map_err(|err| err.to_string()),
                )])
            }
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::FeedsAddFeed { raw_url } => match AppServices::shared().await {
            Ok(services) => match services.add_subscription(&raw_url).await {
                Ok(()) => feeds_intents(vec![
                    FeedsPageIntent::FeedUrlChanged(String::new()),
                    FeedsPageIntent::SetStatus {
                        message: "订阅已保存并完成首次刷新。".to_string(),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) if err.to_string().contains("首次刷新订阅失败") => {
                    feeds_intents(vec![
                        FeedsPageIntent::FeedUrlChanged(String::new()),
                        FeedsPageIntent::SetStatus {
                            message: format!("订阅已保存，但首次刷新失败：{err}"),
                            tone: "error".to_string(),
                        },
                        FeedsPageIntent::BumpReload,
                    ])
                }
                Err(err) => feeds_status_error(format!("保存订阅失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::FeedsRefreshAll => match AppServices::shared().await {
            Ok(services) => match services.refresh_all().await {
                Ok(_) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: "刷新完成。".to_string(),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("刷新失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::FeedsRefreshFeed { feed_id, feed_title } => match AppServices::shared().await {
            Ok(services) => match services.refresh_feed(feed_id).await {
                Ok(_) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: format!("已刷新订阅：{feed_title}"),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("刷新订阅失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::FeedsRemoveFeed { feed_id, feed_title, confirmed } => {
            if !confirmed {
                return feeds_intents(vec![
                    FeedsPageIntent::PendingDeleteFeedSet(Some(feed_id)),
                    FeedsPageIntent::SetStatus {
                        message: format!("再次点击即可删除订阅：{feed_title}"),
                        tone: "info".to_string(),
                    },
                ]);
            }

            match AppServices::shared().await {
                Ok(services) => match services.remove_feed(feed_id).await {
                    Ok(()) => feeds_intents(vec![
                        FeedsPageIntent::PendingDeleteFeedSet(None),
                        FeedsPageIntent::SetStatus {
                            message: format!("已删除订阅：{feed_title}"),
                            tone: "info".to_string(),
                        },
                        FeedsPageIntent::BumpReload,
                    ]),
                    Err(err) => feeds_intents(vec![
                        FeedsPageIntent::PendingDeleteFeedSet(None),
                        FeedsPageIntent::SetStatus {
                            message: format!("删除订阅失败：{err}"),
                            tone: "error".to_string(),
                        },
                    ]),
                },
                Err(err) => feeds_intents(vec![
                    FeedsPageIntent::PendingDeleteFeedSet(None),
                    FeedsPageIntent::SetStatus {
                        message: format!("初始化应用失败：{err}"),
                        tone: "error".to_string(),
                    },
                ]),
            }
        }
        UiCommand::FeedsExportConfig => match AppServices::shared().await {
            Ok(services) => match services.export_config_json().await {
                Ok(raw) => feeds_intents(vec![
                    FeedsPageIntent::ConfigTextExported(raw),
                    FeedsPageIntent::SetStatus {
                        message: "已导出配置包 JSON。".to_string(),
                        tone: "info".to_string(),
                    },
                ]),
                Err(err) => feeds_status_error(format!("导出配置包失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::FeedsImportConfig { raw, confirmed } => {
            if !confirmed {
                return feeds_intents(vec![
                    FeedsPageIntent::PendingConfigImportSet(true),
                    FeedsPageIntent::SetStatus {
                        message:
                            "导入配置会按配置包覆盖当前订阅集合，并清理缺失订阅的本地文章；再次点击才会执行。"
                                .to_string(),
                        tone: "info".to_string(),
                    },
                ]);
            }

            match AppServices::shared().await {
                Ok(services) => match services.import_config_json(&raw).await {
                    Ok(()) => feeds_intents(vec![
                        FeedsPageIntent::PendingConfigImportSet(false),
                        FeedsPageIntent::SetStatus {
                            message: "配置包已导入。".to_string(),
                            tone: "info".to_string(),
                        },
                        FeedsPageIntent::BumpReload,
                    ]),
                    Err(err) => feeds_intents(vec![
                        FeedsPageIntent::PendingConfigImportSet(false),
                        FeedsPageIntent::SetStatus {
                            message: format!("导入配置包失败：{err}"),
                            tone: "error".to_string(),
                        },
                    ]),
                },
                Err(err) => feeds_intents(vec![
                    FeedsPageIntent::PendingConfigImportSet(false),
                    FeedsPageIntent::SetStatus {
                        message: format!("初始化应用失败：{err}"),
                        tone: "error".to_string(),
                    },
                ]),
            }
        }
        UiCommand::FeedsExportOpml => match AppServices::shared().await {
            Ok(services) => match services.export_opml().await {
                Ok(raw) => feeds_intents(vec![
                    FeedsPageIntent::OpmlTextExported(raw),
                    FeedsPageIntent::SetStatus {
                        message: "已导出 OPML。".to_string(),
                        tone: "info".to_string(),
                    },
                ]),
                Err(err) => feeds_status_error(format!("导出 OPML 失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::FeedsImportOpml { raw } => match AppServices::shared().await {
            Ok(services) => match services.import_opml(&raw).await {
                Ok(()) => feeds_intents(vec![
                    FeedsPageIntent::SetStatus {
                        message: "OPML 已导入。".to_string(),
                        tone: "info".to_string(),
                    },
                    FeedsPageIntent::BumpReload,
                ]),
                Err(err) => feeds_status_error(format!("导入 OPML 失败：{err}")),
            },
            Err(err) => feeds_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::SettingsLoad => match AppServices::shared().await {
            Ok(services) => match services.load_settings().await {
                Ok(settings) => {
                    settings_intents(vec![SettingsPageIntent::SettingsLoaded(settings)])
                }
                Err(err) => settings_status_error(format!("读取设置失败：{err}")),
            },
            Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
        },
        UiCommand::SettingsSaveAppearance { settings, success_message } => {
            match AppServices::shared().await {
                Ok(services) => match services.save_settings(&settings).await {
                    Ok(()) => settings_intents(vec![
                        SettingsPageIntent::SettingsLoaded(settings),
                        SettingsPageIntent::SetStatus {
                            message: success_message,
                            tone: "info".to_string(),
                        },
                    ]),
                    Err(err) => settings_status_error(format!("保存设置失败：{err}")),
                },
                Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
            }
        }
        UiCommand::SettingsPushConfig { endpoint, remote_path } => {
            match AppServices::shared().await {
                Ok(services) => match services.push_remote_config(&endpoint, &remote_path).await {
                    Ok(()) => settings_intents(vec![SettingsPageIntent::SetStatus {
                        message: "配置已上传到 WebDAV。".to_string(),
                        tone: "info".to_string(),
                    }]),
                    Err(err) => settings_status_error(format!("上传配置失败：{err}")),
                },
                Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
            }
        }
        UiCommand::SettingsPullConfig { endpoint, remote_path } => {
            match AppServices::shared().await {
                Ok(services) => match services.pull_remote_config(&endpoint, &remote_path).await {
                    Ok(true) => match services.load_settings().await {
                        Ok(settings) => settings_intents(vec![
                            SettingsPageIntent::SettingsLoaded(settings),
                            SettingsPageIntent::SetStatus {
                                message: "已从 WebDAV 下载并导入配置。".to_string(),
                                tone: "info".to_string(),
                            },
                        ]),
                        Err(err) => settings_status_error(format!("导入后读取设置失败：{err}")),
                    },
                    Ok(false) => settings_intents(vec![SettingsPageIntent::SetStatus {
                        message: "远端配置不存在。".to_string(),
                        tone: "info".to_string(),
                    }]),
                    Err(err) => settings_status_error(format!("下载配置失败：{err}")),
                },
                Err(err) => settings_status_error(format!("初始化应用失败：{err}")),
            }
        }
    }
}

fn status_error(message: impl Into<String>) -> UiRuntimeOutcome {
    UiRuntimeOutcome::single(UiIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    })
}

fn resolve_with_fallback(message: impl Into<String>) -> UiRuntimeOutcome {
    UiRuntimeOutcome {
        intents: vec![
            UiIntent::SetStatus { message: message.into(), tone: "error".to_string() },
            UiIntent::StartupRouteResolved(StartupRouteSnapshot {
                route: AppRoute::EntriesPage {},
            }),
        ],
    }
}

fn entries_intents(intents: Vec<EntriesPageIntent>) -> UiRuntimeOutcome {
    UiRuntimeOutcome { intents: intents.into_iter().map(UiIntent::EntriesPage).collect() }
}

fn entries_status_error(message: impl Into<String>) -> UiRuntimeOutcome {
    entries_intents(vec![EntriesPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}

fn reader_intents(intents: Vec<ReaderPageIntent>) -> UiRuntimeOutcome {
    UiRuntimeOutcome { intents: intents.into_iter().map(UiIntent::ReaderPage).collect() }
}

fn reader_status_error(message: impl Into<String>) -> UiRuntimeOutcome {
    reader_intents(vec![ReaderPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}

fn feeds_intents(intents: Vec<FeedsPageIntent>) -> UiRuntimeOutcome {
    UiRuntimeOutcome { intents: intents.into_iter().map(UiIntent::FeedsPage).collect() }
}

fn feeds_status_error(message: impl Into<String>) -> UiRuntimeOutcome {
    feeds_intents(vec![FeedsPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}

fn settings_intents(intents: Vec<SettingsPageIntent>) -> UiRuntimeOutcome {
    UiRuntimeOutcome { intents: intents.into_iter().map(UiIntent::SettingsPage).collect() }
}

fn settings_status_error(message: impl Into<String>) -> UiRuntimeOutcome {
    settings_intents(vec![SettingsPageIntent::SetStatus {
        message: message.into(),
        tone: "error".to_string(),
    }])
}
