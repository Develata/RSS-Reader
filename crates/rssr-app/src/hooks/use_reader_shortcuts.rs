use dioxus::prelude::*;

use crate::{
    bootstrap::AppServices,
    status::{set_status_error, set_status_info},
};

pub fn use_reader_shortcuts(
    entry_id: i64,
    is_read: Signal<bool>,
    is_starred: Signal<bool>,
    reload_tick: Signal<u64>,
    status: Signal<String>,
    status_tone: Signal<String>,
) -> Callback<KeyboardEvent> {
    use_callback(move |event: KeyboardEvent| {
        let key = event.key().to_string().to_lowercase();
        let mut reload_tick = reload_tick;

        match key.as_str() {
            "m" => {
                spawn(async move {
                    match AppServices::shared().await {
                        Ok(services) => match services.set_read(entry_id, !is_read()).await {
                            Ok(()) => {
                                set_status_info(
                                    status,
                                    status_tone,
                                    if is_read() {
                                        "已通过快捷键标记为未读。"
                                    } else {
                                        "已通过快捷键标记为已读。"
                                    },
                                );
                                reload_tick += 1;
                            }
                            Err(err) => {
                                set_status_error(
                                    status,
                                    status_tone,
                                    format!("更新已读状态失败：{err}"),
                                );
                            }
                        },
                        Err(err) => {
                            set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
                        }
                    }
                });
            }
            "f" => {
                spawn(async move {
                    match AppServices::shared().await {
                        Ok(services) => match services.set_starred(entry_id, !is_starred()).await {
                            Ok(()) => {
                                set_status_info(
                                    status,
                                    status_tone,
                                    if is_starred() {
                                        "已通过快捷键取消收藏。"
                                    } else {
                                        "已通过快捷键收藏文章。"
                                    },
                                );
                                reload_tick += 1;
                            }
                            Err(err) => {
                                set_status_error(
                                    status,
                                    status_tone,
                                    format!("更新收藏状态失败：{err}"),
                                );
                            }
                        },
                        Err(err) => {
                            set_status_error(status, status_tone, format!("初始化应用失败：{err}"));
                        }
                    }
                });
            }
            _ => {}
        }
    })
}
