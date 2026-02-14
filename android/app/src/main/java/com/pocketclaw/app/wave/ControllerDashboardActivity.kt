package com.pocketclaw.app.wave

import android.content.Intent
import android.os.Bundle
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import com.pocketclaw.app.PocketClawService

class ControllerDashboardActivity : AppCompatActivity() {
    private lateinit var statusText: TextView
    private lateinit var store: AppConfigStore

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        store = AppConfigStore(this)
        if (!store.hasConfig()) {
            startActivity(Intent(this, com.pocketclaw.app.SetupActivity::class.java))
            finish()
            return
        }

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Agent Control Dashboard"))
        root.addView(UiFactory.subtitle(this, "Dieu khien server local va dieu huong 2/3/4/monitor."))

        statusText = UiFactory.label(this, "Status: Ready")
        root.addView(statusText)

        val startBtn = UiFactory.actionButton(this, "Start Server")
        startBtn.setOnClickListener {
            startService(Intent(this, PocketClawService::class.java))
            statusText.text = "Status: Running"
        }
        root.addView(startBtn)

        root.addView(UiFactory.spacer(this, 10))

        val stopBtn = UiFactory.secondaryButton(this, "Stop Server")
        stopBtn.setOnClickListener {
            startService(Intent(this, PocketClawService::class.java).apply { action = "STOP" })
            statusText.text = "Status: Stopped"
        }
        root.addView(stopBtn)

        root.addView(UiFactory.spacer(this, 10))

        val reloadBtn = UiFactory.secondaryButton(this, "Reload Config")
        reloadBtn.setOnClickListener {
            val cfg = store.load()
            Thread {
                val result = GatewayClient(cfg.gatewayAuthToken.ifBlank { null }).reload()
                runOnUiThread {
                    if (result.isSuccess) {
                        Toast.makeText(this, "Reload triggered", Toast.LENGTH_SHORT).show()
                    } else {
                        Toast.makeText(this, "Reload fail: ${result.exceptionOrNull()?.message}", Toast.LENGTH_LONG).show()
                    }
                }
            }.start()
        }
        root.addView(reloadBtn)

        root.addView(UiFactory.section(this, "Screens"))

        val navLayout = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        navLayout.addView(navButton("Screen 2: Provider & Secrets", ProviderSecretsActivity::class.java))
        navLayout.addView(navButton("Screen 3: Channels", ChannelChatSetupActivity::class.java))
        navLayout.addView(navButton("Screen 4: Skills", SkillsPermissionsActivity::class.java))
        navLayout.addView(navButton("Screen 6: Monitor", ResourceMonitorActivity::class.java))
        root.addView(navLayout)

        setContentView(scroll)
    }

    private fun navButton(text: String, target: Class<*>): TextView {
        return TextView(this).apply {
            this.text = text
            textSize = 14f
            setTextColor(0xFF93C5FD.toInt())
            setPadding(0, 10, 0, 10)
            setOnClickListener { startActivity(Intent(this@ControllerDashboardActivity, target)) }
        }
    }
}
