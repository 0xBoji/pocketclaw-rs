package com.phoneclaw.app.wave

import org.json.JSONObject
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.io.BufferedReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.TimeUnit

class GatewayClient(private val authToken: String?) {
    private val wsClient: OkHttpClient = OkHttpClient.Builder()
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .build()

    private fun open(path: String, method: String): HttpURLConnection {
        val conn = URL("http://127.0.0.1:8080$path").openConnection() as HttpURLConnection
        conn.requestMethod = method
        conn.connectTimeout = 2500
        conn.readTimeout = 3000
        conn.setRequestProperty("Content-Type", "application/json")
        if (!authToken.isNullOrBlank()) {
            conn.setRequestProperty("Authorization", "Bearer $authToken")
        }
        return conn
    }

    fun status(): Result<JSONObject> = runCatching {
        val conn = open("/api/status", "GET")
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun metrics(): Result<JSONObject> = runCatching {
        val conn = open("/api/monitor/metrics", "GET")
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun channelsHealth(): Result<JSONObject> = runCatching {
        val conn = open("/api/channels/health", "GET")
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun sessions(limit: Int = 20): Result<JSONObject> = runCatching {
        val conn = open("/api/sessions?limit=$limit", "GET")
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun sessionMessages(sessionKey: String, limit: Int = 100): Result<JSONObject> = runCatching {
        val encoded = java.net.URLEncoder.encode(sessionKey, "UTF-8")
        val conn = open("/api/sessions/$encoded/messages?limit=$limit", "GET")
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun sendToSession(sessionKey: String, message: String, channel: String = "android.app"): Result<JSONObject> = runCatching {
        val conn = open("/api/sessions/send", "POST")
        conn.doOutput = true
        val body = JSONObject()
            .put("session_key", sessionKey)
            .put("message", message)
            .put("channel", channel)
        OutputStreamWriter(conn.outputStream).use { it.write(body.toString()) }
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun reload(): Result<JSONObject> = runCatching {
        val conn = open("/api/control/reload", "PUT")
        conn.doOutput = true
        OutputStreamWriter(conn.outputStream).use { it.write("{}") }
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun sendMessage(message: String, sessionKey: String = "android:local"): Result<JSONObject> = runCatching {
        val conn = open("/api/message", "POST")
        conn.doOutput = true
        val body = JSONObject().put("message", message).put("session_key", sessionKey)
        OutputStreamWriter(conn.outputStream).use { it.write(body.toString()) }
        conn.inputStream.use { stream ->
            val text = stream.bufferedReader().use(BufferedReader::readText)
            JSONObject(text)
        }
    }

    fun streamEvents(
        onEvent: (JSONObject) -> Unit,
        onError: (String) -> Unit,
        onClosed: (() -> Unit)? = null,
        includeHeartbeat: Boolean = false,
    ): WebSocket {
        val requestBuilder = Request.Builder()
            .url("ws://127.0.0.1:8080/ws/events")
        if (!authToken.isNullOrBlank()) {
            requestBuilder.header("Authorization", "Bearer $authToken")
        }

        return wsClient.newWebSocket(
            requestBuilder.build(),
            object : WebSocketListener() {
                override fun onMessage(webSocket: WebSocket, text: String) {
                    try {
                        val payload = JSONObject(text)
                        if (!includeHeartbeat && payload.optString("type", "") == "heartbeat") {
                            return
                        }
                        onEvent(payload)
                    } catch (e: Exception) {
                        onError("Invalid event payload: ${e.message}")
                    }
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    onError("WebSocket failed: ${t.message ?: "unknown"}")
                }

                override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                    onClosed?.invoke()
                }
            }
        )
    }
}
