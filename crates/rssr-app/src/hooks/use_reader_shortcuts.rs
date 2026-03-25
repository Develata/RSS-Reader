use dioxus::prelude::*;

use crate::bootstrap::AppServices;

pub fn use_reader_shortcuts(
    entry_id: i64,
    is_read: Signal<bool>,
    is_starred: Signal<bool>,
    reload_tick: Signal<u64>,
) -> Callback<KeyboardEvent> {
    use_callback(move |event: KeyboardEvent| {
        let key = event.key().to_string().to_lowercase();
        let mut reload_tick = reload_tick;

        match key.as_str() {
            "m" => {
                spawn(async move {
                    if let Ok(services) = AppServices::shared().await {
                        let _ = services.set_read(entry_id, !is_read()).await;
                        reload_tick += 1;
                    }
                });
            }
            "f" => {
                spawn(async move {
                    if let Ok(services) = AppServices::shared().await {
                        let _ = services.set_starred(entry_id, !is_starred()).await;
                        reload_tick += 1;
                    }
                });
            }
            _ => {}
        }
    })
}
