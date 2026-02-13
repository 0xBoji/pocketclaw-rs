package com.pocketclaw.app.wave

import android.content.Intent
import android.os.Bundle
import android.graphics.Typeface
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import com.pocketclaw.app.PocketClawService
import org.json.JSONArray
import org.json.JSONObject

class ControllerDashboardActivity : AppCompatActivity() {
    private lateinit var statusText: TextView
    private lateinit var channelHealthContainer: LinearLayout
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
        root.addView(UiFactory.title(this, "Screen 5: Agent Control Dashboard"))
        root.addView(UiFactory.subtitle(this, "Dieu khien server local tren Android cu."))

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

        val reloadBtn = UiFactory.secondaryButton(this, "Reload Config via API")
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

        root.addView(UiFactory.section(this, "Channel Health"))
        val healthHint = UiFactory.hint(this, "Xanh = healthy, vang = no recent traffic, do = errors.")
        root.addView(healthHint)

        val refreshHealthBtn = UiFactory.secondaryButton(this, "Refresh Channel Health")
        refreshHealthBtn.setOnClickListener { refreshChannelHealth() }
        root.addView(refreshHealthBtn)

        channelHealthContainer = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
        }
        root.addView(channelHealthContainer)

        root.addView(UiFactory.section(this, "Other Screens"))

        val navLayout = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        navLayout.addView(navButton("Screen 1: Workspace", WorkspaceCreatorActivity::class.java))
        navLayout.addView(navButton("Screen 2: Provider & Secrets", ProviderSecretsActivity::class.java))
        navLayout.addView(navButton("Screen 3: Channels", ChannelChatSetupActivity::class.java))
        navLayout.addView(navButton("Screen 4: Skills", SkillsPermissionsActivity::class.java))
        navLayout.addView(navButton("Screen 6: Monitor", ResourceMonitorActivity::class.java))
        navLayout.addView(navButton("Live Sessions + Chat", SessionLiveActivity::class.java))
        navLayout.addView(navButton("Screen 7: Safety", SafetySandboxActivity::class.java))
        root.addView(navLayout)

        setContentView(scroll)
        refreshChannelHealth()
    }

    override fun onResume() {
        super.onResume()
        refreshChannelHealth()
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

    private fun refreshChannelHealth() {
        val cfg = store.load()
        Thread {
            val result = GatewayClient(cfg.gatewayAuthToken.ifBlank { null }).channelsHealth()
            runOnUiThread {
                if (result.isSuccess) {
                    val channels = result.getOrThrow().optJSONArray("channels")
                    renderChannelCards(channels)
                } else {
                    Toast.makeText(
                        this,
                        "Channel health fail: ${result.exceptionOrNull()?.message}",
                        Toast.LENGTH_LONG
                    ).show()
                }
            }
        }.start()
    }

    private fun renderChannelCards(channels: JSONArray?) {
        channelHealthContainer.removeAllViews()

        if (channels == null || channels.length() == 0) {
            channelHealthContainer.addView(UiFactory.hint(this, "No channel health data."))
            return
        }

        for (i in 0 until channels.length()) {
            val item = channels.optJSONObject(i) ?: continue
            channelHealthContainer.addView(buildChannelCard(item))
        }
    }

    private fun buildChannelCard(item: JSONObject): LinearLayout {
        val channel = item.optString("channel", "unknown")
        val configured = item.optBoolean("configured", false)
        val nativeSupported = item.optBoolean("native_supported", false)
        val status = item.optString("status", "unknown")
        val errorCount = item.optLong("error_count", 0)
        val lastError = item.optString("last_error", "")
        val inbound = item.opt("last_inbound_at_ms")?.toString() ?: "-"
        val outbound = item.opt("last_outbound_at_ms")?.toString() ?: "-"
        val trend = item.optJSONObject("trend_1h")
        val trendInbound = trend?.optLong("inbound_count", 0) ?: 0
        val trendOutbound = trend?.optLong("outbound_count", 0) ?: 0
        val trendErrors = trend?.optLong("error_count", 0) ?: 0
        val stability = trend?.optString("stability", "unknown") ?: "unknown"

        val color = when {
            stability == "unstable" -> 0xFF7F1D1D.toInt() // red-ish
            stability == "degraded" -> 0xFF78350F.toInt() // amber-ish
            stability == "healthy" -> 0xFF14532D.toInt() // green-ish
            configured -> 0xFF78350F.toInt() // amber-ish
            else -> 0xFF1F2937.toInt() // gray
        }

        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(color)
            setPadding(18, 14, 18, 14)
            val lp = LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT,
                LinearLayout.LayoutParams.WRAP_CONTENT
            )
            lp.setMargins(0, 8, 0, 8)
            layoutParams = lp

            addView(TextView(this@ControllerDashboardActivity).apply {
                text = channel.uppercase()
                textSize = 14f
                typeface = Typeface.DEFAULT_BOLD
                setTextColor(0xFFF9FAFB.toInt())
            })
            addView(TextView(this@ControllerDashboardActivity).apply {
                text = "status=$status configured=$configured native=$nativeSupported errors=$errorCount"
                textSize = 12f
                setTextColor(0xFFE5E7EB.toInt())
            })
            addView(TextView(this@ControllerDashboardActivity).apply {
                text = "trend_1h: in=$trendInbound out=$trendOutbound err=$trendErrors stability=$stability"
                textSize = 11f
                setTextColor(0xFFD1D5DB.toInt())
            })
            addView(TextView(this@ControllerDashboardActivity).apply {
                text = "inbound=$inbound outbound=$outbound"
                textSize = 11f
                setTextColor(0xFFD1D5DB.toInt())
            })
            if (lastError.isNotBlank() && lastError != "null") {
                addView(TextView(this@ControllerDashboardActivity).apply {
                    text = "last_error=$lastError"
                    textSize = 11f
                    setTextColor(0xFFFCA5A5.toInt())
                })
            }
        }
    }
}
