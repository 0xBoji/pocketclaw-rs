package com.pocketclaw.app.wave

import android.os.Bundle
import android.widget.CheckBox
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

class SafetySandboxActivity : AppCompatActivity() {
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

        val saveBtn = UiFactory.actionButton(this, "Save Safety Settings")
        saveBtn.setOnClickListener {
            val timeout = timeoutInput.text.toString().trim().toIntOrNull() ?: 30
            val maxOut = outputInput.text.toString().trim().toIntOrNull() ?: 65536

            config.sandboxExecEnabled = execToggle.isChecked
            config.sandboxTimeoutSecs = timeout.coerceAtLeast(1)
            config.sandboxMaxOutputBytes = maxOut.coerceAtLeast(1024)
            config.sandboxNetworkAllowlist = allowlistInput.text.toString().trim()

            store.save(config)
            Toast.makeText(this, "Da luu safety settings", Toast.LENGTH_SHORT).show()
            finish()
        }
        root.addView(saveBtn)

        setContentView(scroll)
    }
}
