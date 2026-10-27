//! Android Activity input transport. Route and overlay policy stays in the Rust page hook.
use tokio::sync::mpsc::{Sender, error::TrySendError};

#[derive(Default)]
struct BackRegistry {
    generation: u64,
    active: Option<(u64, Sender<()>)>,
}

impl BackRegistry {
    fn register(&mut self, sender: Sender<()>) -> u64 {
        self.generation += 1;
        self.active = Some((self.generation, sender));
        self.generation
    }

    fn unregister(&mut self, generation: u64) {
        if self.active.as_ref().is_some_and(|(active, _)| *active == generation) {
            self.active = None;
        }
    }

    fn dispatch(&self) -> bool {
        self.active.as_ref().is_some_and(|(_, sender)| {
            // Coalesce a burst while the UI is busy; never turn a full queue into exit.
            matches!(sender.try_send(()), Ok(()) | Err(TrySendError::Full(())))
        })
    }
}

#[cfg(target_os = "android")]
static BACK: std::sync::Mutex<BackRegistry> =
    std::sync::Mutex::new(BackRegistry { generation: 0, active: None });

#[cfg(target_os = "android")]
pub(super) fn register(sender: Sender<()>) -> u64 {
    BACK.lock().unwrap_or_else(|error| error.into_inner()).register(sender)
}

#[cfg(target_os = "android")]
pub(super) fn unregister(generation: u64) {
    BACK.lock().unwrap_or_else(|error| error.into_inner()).unregister(generation);
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_dioxus_main_MainActivity_dispatchSystemBack(
    _env: jni::JNIEnv,
    _activity: jni::objects::JObject,
) -> jni::sys::jboolean {
    let handled = std::panic::catch_unwind(|| {
        BACK.lock().unwrap_or_else(|error| error.into_inner()).dispatch()
    });
    match handled {
        Ok(value) => u8::from(value),
        Err(_) => {
            tracing::error!("Android 返回输入桥接发生 panic");
            jni::sys::JNI_TRUE
        }
    }
}

#[cfg(target_os = "android")]
pub(super) fn background_task() {
    use jni::{JavaVM, objects::JObject};
    let result = (|| -> anyhow::Result<()> {
        let context = ndk_context::android_context();
        // Both references are owned by the Android host, not this call.
        let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }?;
        let mut env = vm.attach_current_thread()?;
        let activity = unsafe { JObject::from_raw(context.context().cast()) };
        if let Err(error) = env.call_method(&activity, "exitToBackground", "()V", &[]) {
            let _ = env.exception_clear();
            return Err(error.into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        tracing::error!(%error, "无法把 Android 阅读器移到后台");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_page_cleanup_cannot_unregister_the_new_page() {
        let mut registry = BackRegistry::default();
        let (old_sender, mut old_receiver) = tokio::sync::mpsc::channel(1);
        let old = registry.register(old_sender);
        let (new_sender, mut new_receiver) = tokio::sync::mpsc::channel(1);
        let new = registry.register(new_sender);
        registry.unregister(old);
        assert!(registry.dispatch());
        assert!(old_receiver.try_recv().is_err());
        assert_eq!(new_receiver.try_recv(), Ok(()));
        registry.unregister(new);
        assert!(!registry.dispatch());
    }

    #[test]
    fn busy_ui_coalesces_back_but_closed_page_does_not_accept_it() {
        let mut registry = BackRegistry::default();
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        registry.register(sender);
        for _ in 0..100 {
            assert!(registry.dispatch());
        }
        assert_eq!(receiver.try_recv(), Ok(()));
        assert!(receiver.try_recv().is_err());
        drop(receiver);
        assert!(!registry.dispatch());
    }
}
