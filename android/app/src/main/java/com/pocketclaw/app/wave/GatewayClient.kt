package com.pocketclaw.app.wave

import org.json.JSONObject
import java.io.BufferedReader
import java.io.OutputStreamWriter
import java.net.HttpURLConnection
import java.net.URL

class GatewayClient(private val authToken: String?) {
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
}
