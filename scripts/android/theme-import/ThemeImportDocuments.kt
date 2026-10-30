package dev.dioxus.main

import android.database.Cursor
import android.database.MatrixCursor
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract.Document
import android.provider.DocumentsContract.Root
import android.provider.DocumentsProvider
import java.io.File
import java.io.FileNotFoundException

// Added only to the isolated acceptance APK, never to the production manifest/source set.
// Uses the actual DocumentsUI -> ContentResolver path; failures/delays are controlled fixtures.
class ThemeImportDocuments : DocumentsProvider() {
    private val names = listOf("valid", "boundary", "invalid-utf8", "oversize", "invalid-css", "read-failure", "delayed")
    private val documentColumns = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME,
        Document.COLUMN_MIME_TYPE, Document.COLUMN_FLAGS, Document.COLUMN_SIZE)
    private val rootColumns = arrayOf(Root.COLUMN_ROOT_ID, Root.COLUMN_TITLE, Root.COLUMN_FLAGS,
        Root.COLUMN_DOCUMENT_ID, Root.COLUMN_MIME_TYPES)

    override fun onCreate(): Boolean {
        check(context!!.packageName == "io.github.develata.rssreader.backpolicytest")
        return true
    }

    override fun queryRoots(projection: Array<out String>?): Cursor =
        MatrixCursor(projection ?: rootColumns).apply {
            newRow().add(Root.COLUMN_ROOT_ID, "rssr-theme-fixtures")
                .add(Root.COLUMN_TITLE, "RSSR theme import test")
                .add(Root.COLUMN_FLAGS, Root.FLAG_LOCAL_ONLY)
                .add(Root.COLUMN_DOCUMENT_ID, "root")
                .add(Root.COLUMN_MIME_TYPES, "text/css")
        }

    override fun queryDocument(id: String, projection: Array<out String>?): Cursor =
        MatrixCursor(projection ?: documentColumns).apply { addDocument(this, id) }

    override fun queryChildDocuments(id: String, projection: Array<out String>?, sortOrder: String?): Cursor {
        check(id == "root")
        return MatrixCursor(projection ?: documentColumns).apply {
            names.forEach { addDocument(this, it) }
        }
    }

    private fun addDocument(cursor: MatrixCursor, id: String) {
        check(id == "root" || id in names)
        cursor.newRow().add(Document.COLUMN_DOCUMENT_ID, id)
            .add(Document.COLUMN_DISPLAY_NAME, if (id == "root") "RSSR theme import test" else "$id.css")
            .add(Document.COLUMN_MIME_TYPE, if (id == "root") Document.MIME_TYPE_DIR else "text/css")
            .add(Document.COLUMN_FLAGS, 0)
            // Deliberately inaccurate metadata: the app must bound bytes actually read.
            .add(Document.COLUMN_SIZE, 1)
    }

    override fun openDocument(id: String, mode: String, signal: CancellationSignal?): ParcelFileDescriptor {
        check(id in names && mode == "r")
        if (id == "read-failure") throw FileNotFoundException("RSSR injected provider read failure")
        val bytes = when (id) {
            "valid", "delayed" -> "\uFEFF/* Android 导入 */\r\n:root { --rssr-import-test: 37; }\r\n".toByteArray(Charsets.UTF_8)
            "boundary" -> ("/*" + "x".repeat(65532) + "*/").toByteArray(Charsets.UTF_8)
            "oversize" -> ("/*" + "x".repeat(65533) + "*/").toByteArray(Charsets.UTF_8)
            "invalid-utf8" -> byteArrayOf(0xc3.toByte(), 0x28)
            "invalid-css" -> "body {".toByteArray(Charsets.UTF_8)
            else -> error("unknown fixture")
        }
        val pipe = ParcelFileDescriptor.createReliablePipe()
        Thread({
            try {
                ParcelFileDescriptor.AutoCloseOutputStream(pipe[1]).use { output ->
                    if (id == "delayed") {
                        val control = File(context!!.filesDir, "theme-import-fixture").apply { mkdirs() }
                        File(control, "opened").writeText("delayed")
                        val deadline = System.nanoTime() + 60_000_000_000L
                        while (!File(control, "release").exists()) {
                            check(System.nanoTime() < deadline) { "fixture release timed out" }
                            signal?.throwIfCanceled()
                            Thread.sleep(50)
                        }
                    }
                    output.write(bytes)
                }
            } catch (error: Exception) {
                runCatching { pipe[1].closeWithError(error.message ?: "fixture failure") }
            }
        }, "rssr-theme-fixture").start()
        return pipe[0]
    }
}
