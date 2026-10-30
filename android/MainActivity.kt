package dev.dioxus.main

import android.os.Build
import android.os.Bundle
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets
import java.util.concurrent.atomic.AtomicBoolean

typealias BuildConfig = io.github.develata.rssreader.BuildConfig

class MainActivity : WryActivity() {
    private data class PendingThemeExport(val css: String)

    private var pendingThemeExport: PendingThemeExport? = null

    private class PendingThemeImport(val id: Long, val maxBytes: Int) {
        val cancelled = AtomicBoolean(false)
    }

    private var pendingThemeImport: PendingThemeImport? = null

    // 返回键交给应用（use_mobile_back_navigation.rs）：页面内回到上一页或首页，首页上 5 秒内
    // 按两次才退到后台。不沿 WebView 的历史后退：应用内跳转不进那份历史，它只记页内锚点。
    override val handleBackNavigation: Boolean = false

    private val createThemeDocument = registerForActivityResult(
        ActivityResultContracts.CreateDocument("text/css")
    ) { uri ->
        val pending = pendingThemeExport ?: return@registerForActivityResult
        if (uri == null) {
            finishThemeExport(pending, STATUS_CANCELLED, "")
            return@registerForActivityResult
        }

        Thread({
            val result = runCatching {
                contentResolver.openOutputStream(uri, "wt")?.use { output ->
                    output.write(pending.css.toByteArray(StandardCharsets.UTF_8))
                    output.flush()
                } ?: error("系统文档提供器未返回可写输出流。")
            }

            runOnUiThread {
                result.fold(
                    onSuccess = { finishThemeExport(pending, STATUS_SUCCESS, "") },
                    onFailure = { error ->
                        finishThemeExport(
                            pending,
                            STATUS_ERROR,
                            error.message ?: "写入系统文档提供器失败。"
                        )
                    }
                )
            }
        }, "rssr-theme-export").start()
    }

    private val openThemeDocument = registerForActivityResult(
        ActivityResultContracts.OpenDocument()
    ) { uri ->
        val pending = pendingThemeImport ?: return@registerForActivityResult
        if (uri == null || pending.cancelled.get()) {
            finishThemeImport(pending, STATUS_CANCELLED)
            return@registerForActivityResult
        }

        Thread({
            val result = runCatching {
                contentResolver.openInputStream(uri)?.use { input ->
                    val output = ByteArrayOutputStream()
                    val buffer = ByteArray(8192)
                    while (true) {
                        check(!pending.cancelled.get()) { "主题导入已取消。" }
                        // Do not trust provider metadata; read at most one byte over the limit.
                        val count = input.read(buffer, 0, minOf(buffer.size, pending.maxBytes - output.size() + 1))
                        if (count < 0) break
                        check(output.size() + count <= pending.maxBytes) {
                            "CSS 文件不能超过 ${pending.maxBytes} 字节。"
                        }
                        output.write(buffer, 0, count)
                    }
                    output.toByteArray()
                } ?: error("系统文档提供器未返回可读输入流。")
            }
            runOnUiThread {
                result.fold(
                    onSuccess = { bytes -> finishThemeImport(pending, STATUS_SUCCESS, bytes) },
                    onFailure = { error ->
                        finishThemeImport(pending, STATUS_ERROR, message = error.message ?: "读取系统文档提供器失败。")
                    }
                )
            }
        }, "rssr-theme-import").start()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        // targetSdk 仍为 34；仅 debug/API 35+ 主动进入 edge-to-edge，提前验证未来行为。
        if (BuildConfig.DEBUG && Build.VERSION.SDK_INT >= 35) {
            enableEdgeToEdge()
        }
        super.onCreate(savedInstanceState)
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                if (!dispatchSystemBack()) moveTaskToBack(true)
            }
        })
    }

    fun exitToBackground() {
        runOnUiThread { moveTaskToBack(true) }
    }

    fun requestThemeCssExport(css: String, suggestedName: String) {
        runOnUiThread {
            if (pendingThemeExport != null) {
                completeThemeCssExport(STATUS_ERROR, "已有主题文件正在导出，请先完成或取消系统保存器。")
                return@runOnUiThread
            }

            val pending = PendingThemeExport(css)
            pendingThemeExport = pending
            runCatching { createThemeDocument.launch(suggestedName) }
                .onFailure { error ->
                    finishThemeExport(
                        pending,
                        STATUS_ERROR,
                        error.message ?: "无法启动 Android 系统保存器。"
                    )
                }
        }
    }

    fun requestThemeCssImport(id: Long, maxBytes: Int) {
        runOnUiThread {
            if (isDestroyed || maxBytes <= 0 || maxBytes == Int.MAX_VALUE) {
                completeThemeCssImport(id, STATUS_ERROR, byteArrayOf(), "Android 主题选择器不可用。")
                return@runOnUiThread
            }
            if (pendingThemeImport != null) {
                completeThemeCssImport(id, STATUS_ERROR, byteArrayOf(), "已有主题文件正在载入，请先完成或取消系统选择器。")
                return@runOnUiThread
            }
            val pending = PendingThemeImport(id, maxBytes)
            pendingThemeImport = pending
            // CSS is also reported as text/plain or octet-stream by document providers.
            runCatching { openThemeDocument.launch(arrayOf("text/css", "text/plain", "application/octet-stream")) }
                .onFailure { error ->
                    finishThemeImport(pending, STATUS_ERROR, message = error.message ?: "无法启动 Android 系统选择器。")
                }
        }
    }

    fun cancelThemeCssImport(id: Long) {
        runOnUiThread {
            pendingThemeImport?.takeIf { it.id == id }?.cancelled?.set(true)
            // ActivityResult supplies no request ID. Keep the physical picker/read slot until
            // its result returns, so a cancelled pick can never be assigned to a later caller.
        }
    }

    override fun onDestroy() {
        pendingThemeImport?.let { pending ->
            pending.cancelled.set(true)
            finishThemeImport(pending, STATUS_ERROR, message = "Android Activity 已销毁，主题导入未完成。")
        }
        pendingThemeExport?.let { pending ->
            finishThemeExport(pending, STATUS_ERROR, "Android Activity 已销毁，主题导出未完成。")
        }
        super.onDestroy()
    }

    private fun finishThemeExport(pending: PendingThemeExport, status: Int, message: String) {
        if (pendingThemeExport !== pending) {
            return
        }
        pendingThemeExport = null
        completeThemeCssExport(status, message)
    }

    private external fun completeThemeCssExport(status: Int, message: String)

    private fun finishThemeImport(
        pending: PendingThemeImport,
        status: Int,
        bytes: ByteArray = byteArrayOf(),
        message: String = ""
    ) {
        if (pendingThemeImport !== pending) return
        pendingThemeImport = null
        completeThemeCssImport(pending.id, status, bytes, message)
    }

    private external fun completeThemeCssImport(id: Long, status: Int, bytes: ByteArray, message: String)

    private external fun dispatchSystemBack(): Boolean

    private companion object {
        const val STATUS_SUCCESS = 0
        const val STATUS_CANCELLED = 1
        const val STATUS_ERROR = 2

        init {
            System.loadLibrary("main")
        }
    }
}
