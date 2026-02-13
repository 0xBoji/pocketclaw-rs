package com.pocketclaw.app

import android.content.Intent
import android.os.Bundle
import androidx.appcompat.app.AppCompatActivity
import com.pocketclaw.app.wave.AppConfigStore
import com.pocketclaw.app.wave.ControllerDashboardActivity

class MainActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val next = if (store.hasConfig()) {
            Intent(this, ControllerDashboardActivity::class.java)
        } else {
            Intent(this, SetupWizardActivity::class.java)
        }

        startActivity(next)
        finish()
    }
}
