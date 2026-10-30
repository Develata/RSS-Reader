use std::panic::{AssertUnwindSafe, catch_unwind};

use anyhow::{Context, anyhow};
use jni::{
    JNIEnv, JavaVM,
    objects::{JByteArray, JObject, JString, JValue},
    sys::{jint, jlong},
};
use rssr_domain::settings::MAX_CUSTOM_CSS_BYTES;

use super::{ImportRequests, ImportResult};

static REQUESTS: ImportRequests = ImportRequests::new();

pub(crate) async fn pick_css_bytes() -> anyhow::Result<Option<Vec<u8>>> {
    let mut wait = REQUESTS.begin(cancel_picker).map_err(anyhow::Error::msg)?;
    call_activity(
        "requestThemeCssImport",
        "(JI)V",
        &[JValue::Long(wait.id), JValue::Int(MAX_CUSTOM_CSS_BYTES as jint)],
    )?;
    wait.receive().await.map_err(anyhow::Error::msg)
}

fn cancel_picker(id: i64) {
    if let Err(error) = call_activity("cancelThemeCssImport", "(J)V", &[JValue::Long(id)]) {
        tracing::warn!(%error, "无法通知 Android 主题选择器调用方已离开");
    }
}

fn call_activity(
    method: &str,
    signature: &str,
    arguments: &[JValue<'_, '_>],
) -> anyhow::Result<()> {
    let context = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.context("无法访问 Android JavaVM")?;
    let mut env = vm.attach_current_thread().context("无法把主题导入线程附加到 Android JavaVM")?;
    let activity = unsafe { JObject::from_raw(context.context().cast()) };
    let result = env.call_method(&activity, method, signature, arguments);
    if result.is_err() && env.exception_check().unwrap_or(false) {
        // Report a recoverable picker failure, not a pending exception on a reused JNI thread.
        let _ = env.exception_clear();
    }
    result.context("无法调用 Android 系统选择器")?;
    Ok(())
}

fn read_result(
    env: &mut JNIEnv,
    status: jint,
    bytes: JByteArray,
    message: JString,
) -> anyhow::Result<Option<Vec<u8>>> {
    match status {
        0 => {
            // Bound the JNI copy too, even if a malformed host callback bypasses the reader.
            let length = env.get_array_length(&bytes).context("无法读取 Android CSS 文件长度")?;
            if length as usize > MAX_CUSTOM_CSS_BYTES {
                return Err(anyhow!("CSS 文件不能超过 {MAX_CUSTOM_CSS_BYTES} 字节。"));
            }
            Ok(Some(env.convert_byte_array(bytes).context("无法读取 Android CSS 文件内容")?))
        }
        1 => Ok(None),
        2 => {
            let message: String =
                env.get_string(&message).context("无法读取 Android 导入错误")?.into();
            Err(anyhow!(if message.is_empty() {
                "读取系统文档提供器失败。".into()
            } else {
                message
            }))
        }
        other => Err(anyhow!("Android 系统选择器返回未知状态：{other}")),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_dioxus_main_MainActivity_completeThemeCssImport(
    mut env: JNIEnv,
    _activity: JObject,
    id: jlong,
    status: jint,
    bytes: JByteArray,
    message: JString,
) {
    let callback = AssertUnwindSafe(|| {
        let result =
            read_result(&mut env, status, bytes, message).map_err(|error| error.to_string());
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        REQUESTS.complete(id, result);
    });
    if catch_unwind(callback).is_err() {
        // Include channel completion in the boundary: waking the receiver is external code.
        let failure: ImportResult = Err("Android 主题导入回调失败。".into());
        let _ = catch_unwind(AssertUnwindSafe(|| REQUESTS.complete(id, failure)));
        tracing::error!("Android 主题导入 JNI 回调发生 panic，已阻止其跨越 FFI 边界");
    }
}
