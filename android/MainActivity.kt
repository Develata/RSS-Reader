package dev.dioxus.main

import android.os.Build
import android.os.Bundle
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import java.nio.charset.StandardCharsets

typealias BuildConfig = io.github.develata.rssreader.BuildConfig

class MainActivity : WryActivity() {
    private data class PendingThemeExport(val css: String)

    private var pendingThemeExport: PendingThemeExport? = null

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

    override fun onDestroy() {
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
