package com.pocketclaw.app.wave

import android.os.Bundle
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import okhttp3.WebSocket

class ResourceMonitorActivity : AppCompatActivity() {
    private var eventSocket: WebSocket? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val cfg = store.load()

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Screen 6: Event Stream Monitor"))
        root.addView(UiFactory.subtitle(this, "Chi hien gateway event stream (WS)."))

        val eventsHeader = UiFactory.label(this, "Gateway Events (WebSocket)")
        root.addView(eventsHeader)

        val eventsScroll = ScrollView(this)
        val eventsView = TextView(this).apply {
            textSize = 11f
            setTextColor(0xFF93C5FD.toInt())
            typeface = android.graphics.Typeface.MONOSPACE
            text = "Event stream stopped\n"
        }
        eventsScroll.addView(eventsView)
        root.addView(eventsScroll)

        val startEventsBtn = UiFactory.secondaryButton(this, "Start Event Stream")
        startEventsBtn.setOnClickListener {
            if (eventSocket != null) return@setOnClickListener
            val client = GatewayClient(cfg.gatewayAuthToken.ifBlank { null })
            eventSocket = client.streamEvents(
                onEvent = { event ->
                    runOnUiThread {
                        appendLine(eventsView, eventsScroll, event.toString())
                    }
                },
                onError = { error ->
                    runOnUiThread {
                        appendLine(eventsView, eventsScroll, "error: $error")
                        Toast.makeText(this, "Event stream error", Toast.LENGTH_SHORT).show()
                    }
                }
            )
            appendLine(eventsView, eventsScroll, "connecting ws://127.0.0.1:8080/ws/events")
        }
        root.addView(startEventsBtn)

        val stopEventsBtn = UiFactory.secondaryButton(this, "Stop Event Stream")
        stopEventsBtn.setOnClickListener {
            eventSocket?.close(1000, "user stop")
            eventSocket = null
            appendLine(eventsView, eventsScroll, "event stream stopped")
        }
        root.addView(stopEventsBtn)

        setContentView(scroll)
    }

    override fun onDestroy() {
        super.onDestroy()
        eventSocket?.close(1000, "activity destroy")
        eventSocket = null
    }

    private fun appendLine(textView: TextView, scrollView: ScrollView, line: String) {
        val maxChars = 24_000
        val newText = buildString {
            append(textView.text)
            append(line)
            append('\n')
        }
        textView.text = if (newText.length > maxChars) {
            newText.takeLast(maxChars)
        } else {
            newText
        }
        scrollView.post { scrollView.fullScroll(ScrollView.FOCUS_DOWN) }
    }
}
