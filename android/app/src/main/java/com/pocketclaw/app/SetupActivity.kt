package com.pocketclaw.app

import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.widget.LinearLayout
import androidx.appcompat.app.AppCompatActivity
import com.pocketclaw.app.wave.AppConfigStore
import com.pocketclaw.app.wave.ChannelChatSetupActivity
import com.pocketclaw.app.wave.ControllerDashboardActivity
import com.pocketclaw.app.wave.ProviderSecretsActivity
import com.pocketclaw.app.wave.ResourceMonitorActivity
import com.pocketclaw.app.wave.SkillsPermissionsActivity
import com.pocketclaw.app.wave.UiFactory

class SetupActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val config = store.load()

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Wave D Controller Setup"))
        root.addView(UiFactory.subtitle(this, "Bo man hinh toi gian cho Android cu."))

        root.addView(UiFactory.section(this, "Mandatory"))
        root.addView(navButton("Screen 2: Provider & Secrets", ProviderSecretsActivity::class.java))
        root.addView(navButton("Screen 3: Channel Chat Setup", ChannelChatSetupActivity::class.java))

        root.addView(UiFactory.section(this, "Operations"))
        root.addView(navButton("Screen 4: Skill Manifest Viewer & Permissions", SkillsPermissionsActivity::class.java))
        root.addView(navButton("Screen 6: Resource & Log Monitor", ResourceMonitorActivity::class.java))

        root.addView(UiFactory.spacer(this, 20))
        root.addView(UiFactory.hint(this, "Config path: ${store.configPath()}"))
        root.addView(UiFactory.hint(this, "Workspace: ${config.workspace.ifBlank { "(not set)" }}"))

        val continueBtn = UiFactory.actionButton(this, "Continue to Dashboard")
        continueBtn.setOnClickListener {
            startActivity(Intent(this, ControllerDashboardActivity::class.java))
            finish()
        }
        root.addView(continueBtn)

        setContentView(scroll)
    }

    private fun navButton(text: String, cls: Class<*>): LinearLayout {
        val row = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, 6, 0, 6)
        }
        val btn = UiFactory.secondaryButton(this, text)
        btn.setOnClickListener { startActivity(Intent(this, cls)) }
        row.addView(btn)
        return row
    }

    companion object {
        fun hasConfig(context: Context): Boolean {
            val store = AppConfigStore(context)
            return store.hasConfig()
        }
    }
}
