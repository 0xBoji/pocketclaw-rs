package com.pocketclaw.app.wave

import android.graphics.Typeface
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import okhttp3.WebSocket
import org.json.JSONObject

class SessionLiveActivity : AppCompatActivity() {
    private lateinit var gatewayClient: GatewayClient
    private var eventSocket: WebSocket? = null
    private var isStreaming: Boolean = false
    private var reconnectAttempts: Int = 0
    private var liteMode: Boolean = true
    private var selectedSessionKey: String? = null
    private var messageRefreshScheduled: Boolean = false
    private lateinit var selectedSessionText: TextView
    private lateinit var messagesText: TextView
    private lateinit var messagesScroll: ScrollView
    private lateinit var sessionListLayout: LinearLayout
    private val reconnectHandler = Handler(Looper.getMainLooper())
    private val messageRefreshHandler = Handler(Looper.getMainLooper())

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val cfg = AppConfigStore(this).load()
        liteMode = cfg.liteMode
        gatewayClient = GatewayClient(cfg.gatewayAuthToken.ifBlank { null })

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Live Sessions + Chat Stream"))
        root.addView(UiFactory.subtitle(this, "Xem session realtime, tai lich su, va gui tin nhan vao session."))

        selectedSessionText = UiFactory.label(this, "Selected session: (none)")
        root.addView(selectedSessionText)

        val refreshSessionsBtn = UiFactory.actionButton(this, "Refresh Sessions")
        refreshSessionsBtn.setOnClickListener {
            fetchSessions(gatewayClient)
        }
        root.addView(refreshSessionsBtn)

        sessionListLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }
        root.addView(sessionListLayout)

        root.addView(UiFactory.section(this, "Session Chat"))

        messagesScroll = ScrollView(this)
        messagesText = TextView(this).apply {
            textSize = 12f
            setTextColor(0xFFE5E7EB.toInt())
            typeface = Typeface.MONOSPACE
            text = "No session selected\n"
        }
        messagesScroll.addView(messagesText)
        root.addView(messagesScroll)

        val messageInput = UiFactory.input(this, "message to selected session")
        root.addView(messageInput)

        val sendBtn = UiFactory.secondaryButton(this, "Send To Selected Session")
        sendBtn.setOnClickListener {
            val key = selectedSessionKey
            if (key.isNullOrBlank()) {
                Toast.makeText(this, "Select a session first", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }
            val text = messageInput.text.toString().trim()
            if (text.isBlank()) return@setOnClickListener

            Thread {
                val result = gatewayClient.sendToSession(key, text, "android.session_live")
                runOnUiThread {
                    if (result.isSuccess) {
                        messageInput.setText("")
                        appendChatLine("[queued] $text")
                    } else {
                        Toast.makeText(
                            this,
                            "Send failed: ${result.exceptionOrNull()?.message}",
                            Toast.LENGTH_LONG
                        ).show()
                    }
                }
            }.start()
        }
        root.addView(sendBtn)

        val startStreamBtn = UiFactory.secondaryButton(this, "Start Realtime Stream")
        startStreamBtn.setOnClickListener {
            if (isStreaming) return@setOnClickListener
            isStreaming = true
            reconnectAttempts = 0
            appendChatLine("[stream] starting realtime stream")
            startRealtimeStream(gatewayClient)
        }
        root.addView(startStreamBtn)

        val stopStreamBtn = UiFactory.secondaryButton(this, "Stop Realtime Stream")
        stopStreamBtn.setOnClickListener {
            stopRealtimeStream("user stop")
        }
        root.addView(stopStreamBtn)

        setContentView(scroll)
        fetchSessions(gatewayClient)
    }

    private fun fetchSessions(client: GatewayClient) {
        Thread {
            val result = client.sessions(30)
            runOnUiThread {
                if (result.isFailure) {
                    Toast.makeText(
                        this,
                        "Failed to load sessions: ${result.exceptionOrNull()?.message}",
                        Toast.LENGTH_LONG
                    ).show()
                    return@runOnUiThread
                }

                val sessions = result.getOrThrow().optJSONArray("sessions")
                renderSessions(client, sessions)
            }
        }.start()
    }

    private fun renderSessions(client: GatewayClient, sessions: org.json.JSONArray?) {
        sessionListLayout.removeAllViews()

        if (sessions == null || sessions.length() == 0) {
            sessionListLayout.addView(UiFactory.hint(this, "No sessions found yet"))
            return
        }

        for (i in 0 until sessions.length()) {
            val item = sessions.optJSONObject(i) ?: continue
            val key = item.optString("session_key", "")
            if (key.isBlank()) continue
            val count = item.optInt("message_count", 0)
            val updated = item.optString("updated_at", "")

            val row = TextView(this).apply {
                text = "$key  •  msgs: $count  •  $updated"
                textSize = 13f
                setTextColor(0xFF93C5FD.toInt())
                setPadding(0, 8, 0, 8)
                setOnClickListener {
                    selectedSessionKey = key
                    selectedSessionText.text = "Selected session: $key"
                    loadSessionMessages(client, key)
                }
            }
            sessionListLayout.addView(row)
        }
    }

    private fun loadSessionMessages(client: GatewayClient, sessionKey: String) {
        Thread {
            val result = client.sessionMessages(sessionKey, 200)
            runOnUiThread {
                if (result.isFailure) {
                    Toast.makeText(
                        this,
                        "Load messages failed: ${result.exceptionOrNull()?.message}",
                        Toast.LENGTH_LONG
                    ).show()
                    return@runOnUiThread
                }
                val payload = result.getOrThrow()
                val messages = payload.optJSONArray("messages")
                renderMessages(messages)
            }
        }.start()
    }

    private fun scheduleSessionRefresh() {
        if (messageRefreshScheduled) return
        val key = selectedSessionKey ?: return
        messageRefreshScheduled = true
        val delayMs = if (liteMode) 1200L else 350L
        messageRefreshHandler.postDelayed({
            messageRefreshScheduled = false
            val selected = selectedSessionKey ?: return@postDelayed
            if (selected != key) return@postDelayed
            loadSessionMessages(gatewayClient, selected)
        }, delayMs)
    }

    private fun renderMessages(messages: org.json.JSONArray?) {
        if (messages == null || messages.length() == 0) {
            messagesText.text = "(empty session)\n"
            return
        }

        val sb = StringBuilder()
        for (i in 0 until messages.length()) {
            val msg = messages.optJSONObject(i) ?: continue
            val role = msg.optString("role", "unknown")
            val sender = msg.optString("sender_id", "")
            val content = msg.optString("content", "")
            sb.append("[").append(role).append("][").append(sender).append("] ")
                .append(content).append("\n\n")
        }
        messagesText.text = sb.toString()
        messagesScroll.post { messagesScroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    private fun handleEvent(client: GatewayClient, event: JSONObject) {
        val eventType = event.optString("type", "")
        if (eventType == "connected") {
            reconnectAttempts = 0
            runOnUiThread { appendChatLine("[stream] connected") }
            return
        }
        if (eventType == "heartbeat") {
            return
        }
        if (eventType != "inbound_message" && eventType != "outbound_message") {
            return
        }

        val key = selectedSessionKey ?: return
        val message = event.optJSONObject("message") ?: return
        if (message.optString("session_key", "") != key) {
            return
        }

        runOnUiThread {
            val role = message.optString("role", "unknown")
            val sender = message.optString("sender_id", "")
            val content = message.optString("content", "")
            appendChatLine("[$role][$sender]\n$content")
        }
        scheduleSessionRefresh()
    }

    private fun appendChatLine(line: String) {
        val maxChars = 24_000
        val next = buildString {
            append(messagesText.text)
            append(line)
            append('\n')
        }
        messagesText.text = if (next.length > maxChars) next.takeLast(maxChars) else next
        messagesScroll.post { messagesScroll.fullScroll(ScrollView.FOCUS_DOWN) }
    }

    override fun onDestroy() {
        super.onDestroy()
        stopRealtimeStream("activity destroy")
    }

    private fun startRealtimeStream(client: GatewayClient) {
        eventSocket = client.streamEvents(
            onEvent = { event -> handleEvent(client, event) },
            onError = { err ->
                runOnUiThread {
                    appendChatLine("[stream] error: $err")
                    if (isStreaming) scheduleReconnect(client)
                }
            },
            onClosed = {
                runOnUiThread {
                    appendChatLine("[stream] closed")
                    if (isStreaming) scheduleReconnect(client)
                }
            },
            includeHeartbeat = false,
        )
    }

    private fun scheduleReconnect(client: GatewayClient) {
        if (!isStreaming) return
        reconnectAttempts += 1
        val base = if (liteMode) 2000L else 1000L
        val maxDelay = if (liteMode) 15_000L else 10_000L
        val delayMs = (base * (1L shl (reconnectAttempts - 1).coerceAtMost(3))).coerceAtMost(maxDelay)
        appendChatLine("[stream] reconnect in ${delayMs}ms (attempt $reconnectAttempts)")
        reconnectHandler.postDelayed({
            if (!isStreaming) return@postDelayed
            eventSocket?.cancel()
            eventSocket = null
            startRealtimeStream(client)
        }, delayMs)
    }

    private fun stopRealtimeStream(reason: String) {
        isStreaming = false
        reconnectAttempts = 0
        reconnectHandler.removeCallbacksAndMessages(null)
        messageRefreshHandler.removeCallbacksAndMessages(null)
        messageRefreshScheduled = false
        eventSocket?.close(1000, reason)
        eventSocket?.cancel()
        eventSocket = null
        appendChatLine("[stream] stopped: $reason")
    }
}
