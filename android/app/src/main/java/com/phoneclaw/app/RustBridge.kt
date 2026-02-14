package com.phoneclaw.app

object RustBridge {
    init {
        System.loadLibrary("mobile_jni")
    }

    external fun startServer(configPath: String): String
    external fun stopServer(): String

    // --- Accessibility Helpers called from Rust ---

    @JvmStatic
    fun performClick(x: Float, y: Float): Boolean {
        return PhoneClawAccessibilityService.instance?.click(x, y) ?: false
    }

    @JvmStatic
    fun performBack(): Boolean {
        return PhoneClawAccessibilityService.instance?.back() ?: false
    }

    @JvmStatic
    fun performHome(): Boolean {
        return PhoneClawAccessibilityService.instance?.home() ?: false
    }
    
    @JvmStatic
    fun performScroll(x1: Float, y1: Float, x2: Float, y2: Float): Boolean {
        return PhoneClawAccessibilityService.instance?.swipe(x1, y1, x2, y2) ?: false
    }

    @JvmStatic
    fun performInputText(text: String): Boolean {
        return PhoneClawAccessibilityService.instance?.inputText(text) ?: false
    }

    @JvmStatic
    fun performDumpHierarchy(): String {
        return PhoneClawAccessibilityService.instance?.dumpHierarchy() ?: "<error>Service not connected</error>"
    }

    @JvmStatic
    fun performTakeScreenshot(): ByteArray {
        return PhoneClawAccessibilityService.instance?.takeScreenshotSync() ?: ByteArray(0)
    }
}
