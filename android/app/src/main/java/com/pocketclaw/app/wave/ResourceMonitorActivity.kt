package com.pocketclaw.app.wave

import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import java.io.BufferedReader
import java.io.InputStreamReader

class ResourceMonitorActivity : AppCompatActivity() {
    private var logThread: Thread? = null
    private var isLogging = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val cfg = store.load()

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Screen 6: Resource & Log Monitor"))
        root.addView(UiFactory.subtitle(this, "Doc metrics tu /api/monitor/metrics va logcat."))

        val metricsText = UiFactory.input(this, "metrics", multiline = true).apply {
            isFocusable = false
            setText("No metrics yet")
        }
        root.addView(metricsText)

        val refreshBtn = UiFactory.actionButton(this, "Refresh Metrics")
        refreshBtn.setOnClickListener {
            Thread {
                val client = GatewayClient(cfg.gatewayAuthToken.ifBlank { null })
                val result = client.metrics()
                runOnUiThread {
                    if (result.isSuccess) {
                        metricsText.setText(result.getOrThrow().toString(2))
                    } else {
                        Toast.makeText(this, "Metrics fail: ${result.exceptionOrNull()?.message}", Toast.LENGTH_LONG).show()
                    }
                }
            }.start()
        }
        root.addView(refreshBtn)

        val logHeader = UiFactory.label(this, "Live Logs")
        root.addView(logHeader)

        val logScroll = ScrollView(this)
        val logView = TextView(this).apply {
            textSize = 11f
            setTextColor(0xFF86EFAC.toInt())
            typeface = android.graphics.Typeface.MONOSPACE
            text = "Logs not started\n"
        }
        logScroll.addView(logView)
        root.addView(logScroll)

        val startLogBtn = UiFactory.secondaryButton(this, "Start Log Capture")
        startLogBtn.setOnClickListener {
            if (isLogging) return@setOnClickListener
            isLogging = true
            startLogCapture(logView, logScroll)
        }
        root.addView(startLogBtn)

        val stopLogBtn = UiFactory.secondaryButton(this, "Stop Log Capture")
        stopLogBtn.setOnClickListener {
            isLogging = false
            logThread?.interrupt()
        }
        root.addView(stopLogBtn)

        setContentView(scroll)
    }

    private fun startLogCapture(logView: TextView, logScroll: ScrollView) {
        try {
            Runtime.getRuntime().exec(arrayOf("logcat", "-c"))
        } catch (_: Exception) {
        }

        logThread = Thread {
            try {
                val process = Runtime.getRuntime().exec(arrayOf("logcat", "-v", "time", "-s", "PocketClaw:*", "RustStdoutStderr:*"))
                val reader = BufferedReader(InputStreamReader(process.inputStream))
                val handler = Handler(Looper.getMainLooper())

                while (isLogging) {
                    val line = reader.readLine() ?: break
                    val displayLine = line.substringAfter("): ", line)
                    handler.post {
                        logView.append("$displayLine\n")
                        logScroll.post { logScroll.fullScroll(ScrollView.FOCUS_DOWN) }
                    }
                }
                process.destroy()
            } catch (e: Exception) {
                Handler(Looper.getMainLooper()).post {
                    logView.append("log error: ${e.message}\n")
                }
            }
        }
        logThread?.start()
    }

    override fun onDestroy() {
        super.onDestroy()
        isLogging = false
        logThread?.interrupt()
    }
}
