use dioxus::prelude::*;

use crate::bootstrap::AppServices;

pub fn use_reader_shortcuts(
    entry_id: i64,
    is_read: Signal<bool>,
    is_starred: Signal<bool>,
    reload_tick: Signal<u64>,
    mut status: Signal<String>,
    mut status_tone: Signal<String>,
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
                                status.set(if is_read() {
                                    "已通过快捷键标记为未读。".to_string()
                                } else {
                                    "已通过快捷键标记为已读。".to_string()
                                });
                                status_tone.set("info".to_string());
                                reload_tick += 1;
                            }
                            Err(err) => {
                                status.set(format!("更新已读状态失败：{err}"));
                                status_tone.set("error".to_string());
                            }
                        },
                        Err(err) => {
                            status.set(format!("初始化应用失败：{err}"));
                            status_tone.set("error".to_string());
                        }
                    }
                });
            }
            "f" => {
                spawn(async move {
                    match AppServices::shared().await {
                        Ok(services) => match services.set_starred(entry_id, !is_starred()).await {
                            Ok(()) => {
                                status.set(if is_starred() {
                                    "已通过快捷键取消收藏。".to_string()
                                } else {
                                    "已通过快捷键收藏文章。".to_string()
                                });
                                status_tone.set("info".to_string());
                                reload_tick += 1;
                            }
                            Err(err) => {
                                status.set(format!("更新收藏状态失败：{err}"));
                                status_tone.set("error".to_string());
                            }
                        },
                        Err(err) => {
                            status.set(format!("初始化应用失败：{err}"));
                            status_tone.set("error".to_string());
                        }
                    }
                });
            }
            _ => {}
        }
    })
}
