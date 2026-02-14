package com.pocketclaw.app

import android.content.Intent

object RustBridge {
    init {
        System.loadLibrary("mobile_jni")
    }

    external fun startServer(configPath: String): String
    external fun stopServer(): String

    // --- Accessibility Helpers called from Rust ---

    @JvmStatic
    fun performClick(x: Float, y: Float): Boolean {
        return PocketClawAccessibilityService.instance?.click(x, y) ?: false
    }

    @JvmStatic
    fun performBack(): Boolean {
        return PocketClawAccessibilityService.instance?.back() ?: false
    }

    @JvmStatic
    fun performHome(): Boolean {
        return PocketClawAccessibilityService.instance?.home() ?: false
    }

    @JvmStatic
    fun performLaunchApp(app: String): Boolean {
        val context = PocketClawAccessibilityService.instance?.applicationContext ?: return false
        val pm = context.packageManager
        val key = app.trim().lowercase()

        val aliases = mapOf(
            "facebook" to listOf("com.facebook.katana", "com.facebook.lite"),
            "fb" to listOf("com.facebook.katana", "com.facebook.lite"),
            "messenger" to listOf("com.facebook.orca"),
            "telegram" to listOf("org.telegram.messenger", "org.thunderdog.challegram"),
            "zalo" to listOf("com.zing.zalo"),
            "discord" to listOf("com.discord"),
            "slack" to listOf("com.Slack"),
            "chrome" to listOf("com.android.chrome")
        )

        val candidates = buildList {
            add(key)
            addAll(aliases[key].orEmpty())
        }.distinct()

        for (pkg in candidates) {
            val launchIntent = pm.getLaunchIntentForPackage(pkg) ?: continue
            launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            return try {
                context.startActivity(launchIntent)
                true
            } catch (_: Exception) {
                false
            }
        }

        // Fallback: if user gives app label instead of package, try best-effort match.
        val intent = Intent(Intent.ACTION_MAIN, null).apply {
            addCategory(Intent.CATEGORY_LAUNCHER)
        }
        val activities = pm.queryIntentActivities(intent, 0)
        val matched = activities.firstOrNull { info ->
            val label = info.loadLabel(pm)?.toString()?.lowercase().orEmpty()
            label.contains(key)
        } ?: return false

        val launchIntent = pm.getLaunchIntentForPackage(matched.activityInfo.packageName) ?: return false
        launchIntent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        return try {
            context.startActivity(launchIntent)
            true
        } catch (_: Exception) {
            false
        }
    }
    
    @JvmStatic
    fun performScroll(x1: Float, y1: Float, x2: Float, y2: Float): Boolean {
        return PocketClawAccessibilityService.instance?.swipe(x1, y1, x2, y2) ?: false
    }

    @JvmStatic
    fun performInputText(text: String): Boolean {
        return PocketClawAccessibilityService.instance?.inputText(text) ?: false
    }

    @JvmStatic
    fun performDumpHierarchy(): String {
        return PocketClawAccessibilityService.instance?.dumpHierarchy() ?: "<error>Service not connected</error>"
    }

    @JvmStatic
    fun performTakeScreenshot(): ByteArray {
        return PocketClawAccessibilityService.instance?.takeScreenshotSync() ?: ByteArray(0)
    }
}
