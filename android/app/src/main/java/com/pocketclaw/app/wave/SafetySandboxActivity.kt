package com.pocketclaw.app.wave

import android.os.Bundle
import android.widget.CheckBox
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

class SafetySandboxActivity : AppCompatActivity() {
    private fun applyLiteProfile(config: AppConfigData, enabled: Boolean) {
        if (enabled) {
            config.wsHeartbeatSecs = 30
            config.healthWindowMinutes = 30
            config.dedupeMaxEntries = 1024
            config.adapterMaxInflight = 1
            config.adapterRetryJitterMs = 300
        } else {
            config.wsHeartbeatSecs = 15
            config.healthWindowMinutes = 60
            config.dedupeMaxEntries = 2048
            config.adapterMaxInflight = 2
            config.adapterRetryJitterMs = 150
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val config = store.load()

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Screen 7: Safety & Sandbox"))
        root.addView(UiFactory.subtitle(this, "Cau hinh gioi han toan cho tool execution/network."))

        val execToggle = CheckBox(this).apply {
            text = "Enable exec_cmd"
            setTextColor(0xFFD1D5DB.toInt())
            isChecked = config.sandboxExecEnabled
        }
        root.addView(execToggle)

        root.addView(UiFactory.label(this, "Exec timeout (seconds)"))
        val timeoutInput = UiFactory.input(this, "30")
        timeoutInput.setText(config.sandboxTimeoutSecs.toString())
        root.addView(timeoutInput)

        root.addView(UiFactory.label(this, "Max output bytes"))
        val outputInput = UiFactory.input(this, "65536")
        outputInput.setText(config.sandboxMaxOutputBytes.toString())
        root.addView(outputInput)

        root.addView(UiFactory.label(this, "Network allowlist domains (comma separated)"))
        val allowlistInput = UiFactory.input(this, "example.com,api.example.com")
        allowlistInput.setText(config.sandboxNetworkAllowlist)
        root.addView(allowlistInput)

        root.addView(UiFactory.hint(this, "Luu y: backend hien tai co the chua ap dung day du truong sandbox trong config."))

        root.addView(UiFactory.section(this, "Performance (Android old devices)"))
        val liteToggle = CheckBox(this).apply {
            text = "Lite mode (recommended for old Android)"
            setTextColor(0xFFD1D5DB.toInt())
            isChecked = config.liteMode
        }
        root.addView(liteToggle)

        root.addView(UiFactory.label(this, "WS heartbeat seconds"))
        val wsHeartbeatInput = UiFactory.input(this, "30")
        wsHeartbeatInput.setText(config.wsHeartbeatSecs.toString())
        root.addView(wsHeartbeatInput)

        root.addView(UiFactory.label(this, "Channel health window minutes"))
        val healthWindowInput = UiFactory.input(this, "30")
        healthWindowInput.setText(config.healthWindowMinutes.toString())
        root.addView(healthWindowInput)

        root.addView(UiFactory.label(this, "Dedupe max entries"))
        val dedupeMaxInput = UiFactory.input(this, "1024")
        dedupeMaxInput.setText(config.dedupeMaxEntries.toString())
        root.addView(dedupeMaxInput)

        root.addView(UiFactory.label(this, "Adapter max inflight"))
        val adapterInflightInput = UiFactory.input(this, "1")
        adapterInflightInput.setText(config.adapterMaxInflight.toString())
        root.addView(adapterInflightInput)

        root.addView(UiFactory.label(this, "Adapter retry jitter ms"))
        val adapterJitterInput = UiFactory.input(this, "300")
        adapterJitterInput.setText(config.adapterRetryJitterMs.toString())
        root.addView(adapterJitterInput)

        val saveBtn = UiFactory.actionButton(this, "Save Safety Settings")
        saveBtn.setOnClickListener {
            val timeout = timeoutInput.text.toString().trim().toIntOrNull() ?: 30
            val maxOut = outputInput.text.toString().trim().toIntOrNull() ?: 65536
            val wsHeartbeat = wsHeartbeatInput.text.toString().trim().toIntOrNull() ?: 30
            val healthWindow = healthWindowInput.text.toString().trim().toIntOrNull() ?: 30
            val dedupeMax = dedupeMaxInput.text.toString().trim().toIntOrNull() ?: 1024
            val adapterInflight = adapterInflightInput.text.toString().trim().toIntOrNull() ?: 1
            val adapterJitter = adapterJitterInput.text.toString().trim().toIntOrNull() ?: 300

            config.sandboxExecEnabled = execToggle.isChecked
            config.sandboxTimeoutSecs = timeout.coerceAtLeast(1)
            config.sandboxMaxOutputBytes = maxOut.coerceAtLeast(1024)
            config.sandboxNetworkAllowlist = allowlistInput.text.toString().trim()
            config.liteMode = liteToggle.isChecked

            if (config.liteMode) {
                applyLiteProfile(config, true)
            } else {
                config.wsHeartbeatSecs = wsHeartbeat.coerceIn(3, 120)
                config.healthWindowMinutes = healthWindow.coerceIn(5, 60)
                config.dedupeMaxEntries = dedupeMax.coerceIn(128, 20000)
                config.adapterMaxInflight = adapterInflight.coerceIn(1, 8)
                config.adapterRetryJitterMs = adapterJitter.coerceIn(0, 2000)
            }

            store.save(config)
            Toast.makeText(this, "Da luu safety settings", Toast.LENGTH_SHORT).show()
            finish()
        }
        root.addView(saveBtn)

        setContentView(scroll)
    }
}
