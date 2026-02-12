package com.pocketclaw.app.wave

import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

class ChannelChatSetupActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val config = store.load()

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Screen 3: Channel Chat Setup"))
        root.addView(UiFactory.subtitle(this, "Cau hinh Telegram/Discord va test API message local."))

        root.addView(UiFactory.section(this, "Telegram"))
        root.addView(UiFactory.label(this, "Telegram Bot Token"))
        val telegramInput = UiFactory.input(this, "123456:ABC...", secret = true)
        telegramInput.setText(config.telegramToken)
        root.addView(telegramInput)

        root.addView(UiFactory.section(this, "Discord"))
        root.addView(UiFactory.label(this, "Discord Bot Token"))
        val discordInput = UiFactory.input(this, "MTk...", secret = true)
        discordInput.setText(config.discordToken)
        root.addView(discordInput)

        root.addView(UiFactory.section(this, "Local Gateway Smoke Test"))
        val testBtn = UiFactory.secondaryButton(this, "Send Test Message to /api/message")
        testBtn.setOnClickListener {
            Thread {
                val client = GatewayClient(config.gatewayAuthToken.ifBlank { null })
                val result = client.sendMessage("ping from android setup")
                runOnUiThread {
                    if (result.isSuccess) {
                        Toast.makeText(this, "Gateway accepted message", Toast.LENGTH_SHORT).show()
                    } else {
                        Toast.makeText(this, "Gateway test fail: ${result.exceptionOrNull()?.message}", Toast.LENGTH_LONG).show()
                    }
                }
            }.start()
        }
        root.addView(testBtn)

        root.addView(UiFactory.spacer(this))
        val saveBtn = UiFactory.actionButton(this, "Save Channel Setup")
        saveBtn.setOnClickListener {
            config.telegramToken = telegramInput.text.toString().trim()
            config.discordToken = discordInput.text.toString().trim()
            store.save(config)
            Toast.makeText(this, "Da luu channel setup", Toast.LENGTH_SHORT).show()
            finish()
        }
        root.addView(saveBtn)

        setContentView(scroll)
    }
}
