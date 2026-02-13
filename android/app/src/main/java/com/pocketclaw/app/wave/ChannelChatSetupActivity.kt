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
        root.addView(UiFactory.subtitle(this, "Cau hinh Telegram/Discord/Slack/WhatsApp va test API message local."))

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

        root.addView(UiFactory.section(this, "Slack"))
        root.addView(UiFactory.label(this, "Slack Bot Token"))
        val slackBotTokenInput = UiFactory.input(this, "xoxb-...", secret = true)
        slackBotTokenInput.setText(config.slackBotToken)
        root.addView(slackBotTokenInput)

        root.addView(UiFactory.label(this, "Default Channel ID (optional)"))
        val slackChannelInput = UiFactory.input(this, "C0123456789")
        slackChannelInput.setText(config.slackDefaultChannel)
        root.addView(slackChannelInput)

        root.addView(UiFactory.section(this, "WhatsApp Cloud API"))
        root.addView(UiFactory.label(this, "WhatsApp Access Token"))
        val whatsappTokenInput = UiFactory.input(this, "EAAG...", secret = true)
        whatsappTokenInput.setText(config.whatsappToken)
        root.addView(whatsappTokenInput)

        root.addView(UiFactory.label(this, "Phone Number ID"))
        val whatsappPhoneIdInput = UiFactory.input(this, "1234567890")
        whatsappPhoneIdInput.setText(config.whatsappPhoneNumberId)
        root.addView(whatsappPhoneIdInput)

        root.addView(UiFactory.label(this, "Default Recipient (E.164, optional)"))
        val whatsappDefaultToInput = UiFactory.input(this, "+84901234567")
        whatsappDefaultToInput.setText(config.whatsappDefaultTo)
        root.addView(whatsappDefaultToInput)

        root.addView(UiFactory.label(this, "Webhook Verify Token (optional)"))
        val whatsappVerifyTokenInput = UiFactory.input(this, "my-verify-token")
        whatsappVerifyTokenInput.setText(config.whatsappVerifyToken)
        root.addView(whatsappVerifyTokenInput)

        root.addView(UiFactory.label(this, "Webhook App Secret (optional)"))
        val whatsappAppSecretInput = UiFactory.input(this, "app-secret", secret = true)
        whatsappAppSecretInput.setText(config.whatsappAppSecret)
        root.addView(whatsappAppSecretInput)

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
            config.slackBotToken = slackBotTokenInput.text.toString().trim()
            config.slackDefaultChannel = slackChannelInput.text.toString().trim()
            config.whatsappToken = whatsappTokenInput.text.toString().trim()
            config.whatsappPhoneNumberId = whatsappPhoneIdInput.text.toString().trim()
            config.whatsappDefaultTo = whatsappDefaultToInput.text.toString().trim()
            config.whatsappVerifyToken = whatsappVerifyTokenInput.text.toString().trim()
            config.whatsappAppSecret = whatsappAppSecretInput.text.toString().trim()
            store.save(config)
            Toast.makeText(this, "Da luu channel setup", Toast.LENGTH_SHORT).show()
            finish()
        }
        root.addView(saveBtn)

        setContentView(scroll)
    }
}
