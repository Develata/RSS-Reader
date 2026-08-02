use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Mutex,
};

use anyhow::{Context, anyhow, bail};
use jni::{
    JNIEnv, JavaVM,
    objects::{JObject, JString, JValue},
    sys::jint,
};
use tokio::sync::oneshot;

const DEFAULT_FILE_NAME: &str = "rss-reader-theme.css";
const STATUS_SUCCESS: jint = 0;
const STATUS_CANCELLED: jint = 1;
const STATUS_ERROR: jint = 2;

type ExportResult = Result<bool, String>;

static PENDING_EXPORT: Mutex<Option<oneshot::Sender<ExportResult>>> = Mutex::new(None);

pub(super) async fn save_css_file_with_android_picker(raw: &str) -> anyhow::Result<bool> {
    let (sender, receiver) = oneshot::channel();
    {
        let mut pending =
            PENDING_EXPORT.lock().map_err(|_| anyhow!("Android 主题导出状态锁已损坏。"))?;
        if pending.is_some() {
            bail!("已有主题文件正在导出，请先完成或取消系统保存器。");
        }
        *pending = Some(sender);
    }

    if let Err(error) = launch_android_picker(raw, DEFAULT_FILE_NAME) {
        clear_pending_export();
        return Err(error);
    }

    match receiver.await {
        Ok(Ok(saved)) => Ok(saved),
        Ok(Err(message)) => Err(anyhow!(message)),
        Err(_) => Err(anyhow!("Android 系统保存器未返回导出结果。")),
    }
}

fn launch_android_picker(raw: &str, suggested_name: &str) -> anyhow::Result<()> {
    let context = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.context("无法访问 Android JavaVM")?;
    let mut env = vm.attach_current_thread().context("无法把主题导出线程附加到 Android JavaVM")?;
    let activity = unsafe { JObject::from_raw(context.context().cast()) };
    let css = JObject::from(env.new_string(raw).context("无法把主题 CSS 传给 Android")?);
    let name =
        JObject::from(env.new_string(suggested_name).context("无法把主题导出文件名传给 Android")?);

    env.call_method(
        &activity,
        "requestThemeCssExport",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[JValue::Object(&css), JValue::Object(&name)],
    )
    .context("无法启动 Android 系统保存器")?;

    Ok(())
}

fn clear_pending_export() {
    match PENDING_EXPORT.lock() {
        Ok(mut pending) => {
            pending.take();
        }
        Err(_) => tracing::error!("Android 主题导出状态锁已损坏，无法清理在途请求"),
    }
}

fn finish_pending_export(result: ExportResult) {
    let sender = match PENDING_EXPORT.lock() {
        Ok(mut pending) => pending.take(),
        Err(_) => {
            tracing::error!("Android 主题导出状态锁已损坏，无法接收系统保存器结果");
            None
        }
    };

    match sender {
        Some(sender) => {
            if sender.send(result).is_err() {
                tracing::warn!("Android 主题导出调用方已离开，系统保存器结果被丢弃");
            }
        }
        None => tracing::warn!("收到孤立的 Android 主题导出回调"),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_dioxus_main_MainActivity_completeThemeCssExport(
    mut env: JNIEnv,
    _activity: JObject,
    status: jint,
    message: JString,
) {
    let callback = AssertUnwindSafe(|| {
        let message = env
            .get_string(&message)
            .map(|value| value.into())
            .unwrap_or_else(|error| format!("无法读取 Android 导出错误：{error}"));

        let result = match status {
            STATUS_SUCCESS => Ok(true),
            STATUS_CANCELLED => Ok(false),
            STATUS_ERROR => Err(if message.is_empty() {
                "Android 系统保存器写入失败。".to_string()
            } else {
                message
            }),
            other => Err(format!("Android 系统保存器返回未知状态：{other}")),
        };
        finish_pending_export(result);
    });

    if catch_unwind(callback).is_err() {
        tracing::error!("Android 主题导出 JNI 回调发生 panic，已阻止其跨越 FFI 边界");
    }
}
