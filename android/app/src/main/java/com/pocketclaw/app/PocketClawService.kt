package com.pocketclaw.app

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import com.pocketclaw.app.wave.ControllerDashboardActivity
import java.io.File

class PocketClawService : Service() {

    private val CHANNEL_ID = "PocketClawChannel"
    private var isRunning = false

    override fun onBind(intent: Intent): IBinder? {
        return null
    }

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == "STOP") {
            stopSelf()
            isRunning = false
            return START_NOT_STICKY
        }

        if (!isRunning) {
            val notificationIntent = Intent(this, ControllerDashboardActivity::class.java)
            val pendingIntent = PendingIntent.getActivity(this, 0, notificationIntent, PendingIntent.FLAG_IMMUTABLE)

            val stopIntent = Intent(this, PocketClawService::class.java).apply { action = "STOP" }
            val stopPendingIntent = PendingIntent.getService(this, 0, stopIntent, PendingIntent.FLAG_IMMUTABLE)

            val notification = NotificationCompat.Builder(this, CHANNEL_ID)
                .setContentTitle("PocketClaw Agent")
                .setContentText("Running local gateway on Android")
                .setSmallIcon(android.R.drawable.sym_def_app_icon)
                .setContentIntent(pendingIntent)
                .addAction(android.R.drawable.ic_menu_close_clear_cancel, "Stop", stopPendingIntent)
                .build()

            startForeground(1, notification)

            Thread {
                isRunning = true
                val configPath = setupConfigFile()
                RustBridge.startServer(configPath)
                stopSelf()
            }.start()
        }

        return START_STICKY
    }

    private fun setupConfigFile(): String {
        val configDir = File(filesDir, ".pocketclaw")
        if (!configDir.exists()) configDir.mkdirs()
        return File(configDir, "config.json").absolutePath
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val serviceChannel = NotificationChannel(
                CHANNEL_ID,
                "PocketClaw Service Channel",
                NotificationManager.IMPORTANCE_DEFAULT
            )
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(serviceChannel)
        }
    }
}
