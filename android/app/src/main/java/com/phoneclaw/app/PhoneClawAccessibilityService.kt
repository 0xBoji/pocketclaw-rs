package com.phoneclaw.app

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo
import android.util.Log

class PhoneClawAccessibilityService : AccessibilityService() {

    companion object {
        var instance: PhoneClawAccessibilityService? = null
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        Log.d("PhoneClaw", "Accessibility Service Connected")
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // We can listen to events here if needed, e.g., window state changes
    }

    override fun onInterrupt() {
        Log.d("PhoneClaw", "Accessibility Service Interrupted")
    }

    override fun onDestroy() {
        super.onDestroy()
        instance = null
    }

    // --- Action Methods ---

    fun click(x: Float, y: Float): Boolean {
        val path = Path()
        path.moveTo(x, y)
        val builder = GestureDescription.Builder()
        val gesture = builder.addStroke(GestureDescription.StrokeDescription(path, 0, 50))
            .build()
        return dispatchGesture(gesture, null, null)
    }

    fun swipe(x1: Float, y1: Float, x2: Float, y2: Float, duration: Long = 300): Boolean {
        val path = Path()
        path.moveTo(x1, y1)
        path.lineTo(x2, y2)
        val builder = GestureDescription.Builder()
        val gesture = builder.addStroke(GestureDescription.StrokeDescription(path, 0, duration))
            .build()
        return dispatchGesture(gesture, null, null)
    }

    fun back(): Boolean {
        return performGlobalAction(GLOBAL_ACTION_BACK)
    }

    fun home(): Boolean {
        return performGlobalAction(GLOBAL_ACTION_HOME)
    }

    fun recentApps(): Boolean {
        return performGlobalAction(GLOBAL_ACTION_RECENTS)
    }

    // --- Advanced Features ---

    fun inputText(text: String): Boolean {
        val root = rootInActiveWindow ?: return false
        val focus = root.findFocus(AccessibilityNodeInfo.FOCUS_INPUT)
        
        if (focus != null && focus.isEditable) {
            val arguments = android.os.Bundle()
            arguments.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, text)
            val result = focus.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, arguments)
            focus.recycle()
            return result
        }
        
        // Fallback: Try to find any editable node if focus is null
        // This is a simple heuristic; might need refinement
        return false 
    }

    fun dumpHierarchy(): String {
        val root = rootInActiveWindow ?: return "<error>No active window root</error>"
        val sb = StringBuilder()
        dumpNode(root, sb, 0)
        return sb.toString()
    }

    private fun dumpNode(node: AccessibilityNodeInfo?, sb: StringBuilder, depth: Int) {
        if (node == null) return

        val indent = "  ".repeat(depth)
        sb.append(indent).append("<node")
        
        if (node.className != null) sb.append(" class=\"").append(node.className).append("\"")
        if (node.text != null) sb.append(" text=\"").append(escapeXml(node.text)).append("\"")
        if (node.contentDescription != null) sb.append(" desc=\"").append(escapeXml(node.contentDescription)).append("\"")
        if (node.viewIdResourceName != null) sb.append(" id=\"").append(node.viewIdResourceName).append("\"")
        
        val rect = android.graphics.Rect()
        node.getBoundsInScreen(rect)
        sb.append(" bounds=\"").append(rect.toShortString()).append("\"")
        
        if (node.isClickable) sb.append(" clickable=\"true\"")
        if (node.isEditable) sb.append(" editable=\"true\"")
        if (node.isVisibleToUser) sb.append(" visible=\"true\"")

        if (node.childCount == 0) {
            sb.append(" />\n")
        } else {
            sb.append(">\n")
            for (i in 0 until node.childCount) {
                dumpNode(node.getChild(i), sb, depth + 1)
            }
            sb.append(indent).append("</node>\n")
        }
        
        // node.recycle() - careful with recycling here if used recursively with getChild(), 
        // usually safer to let system handle it or recycle carefully in non-recursive loop.
        // For simple recursion, getting usage from getChild creates new instances.
    }

    private fun escapeXml(original: CharSequence?): String {
        if (original == null) return ""
        return original.toString()
            .replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
            .replace("'", "&apos;")
            .replace("\n", "&#10;")
    }

    // --- Screenshot ---
    
    fun takeScreenshotSync(): ByteArray? {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.R) {
            Log.e("PhoneClaw", "Screenshot requires Android 11+")
            return null
        }

        val latch = java.util.concurrent.CountDownLatch(1)
        var bitmap: android.graphics.Bitmap? = null
        val executor = java.util.concurrent.Executors.newSingleThreadExecutor()

        takeScreenshot(
            android.view.Display.DEFAULT_DISPLAY,
            executor,
            object : TakeScreenshotCallback {
                override fun onSuccess(screenshot: AccessibilityService.ScreenshotResult) {
                     try {
                        val hardwareBitmap = screenshot.hardwareBuffer.let { buffer ->
                            android.graphics.Bitmap.wrapHardwareBuffer(buffer, screenshot.colorSpace)
                        }
                        // Copy to software bitmap to access pixels/compress
                        bitmap = hardwareBitmap?.copy(android.graphics.Bitmap.Config.ARGB_8888, false)
                        hardwareBitmap?.recycle()
                        screenshot.hardwareBuffer.close()
                    } catch (e: Exception) {
                        Log.e("PhoneClaw", "Screenshot processing failed", e)
                    } finally {
                        latch.countDown()
                    }
                }

                override fun onFailure(errorCode: Int) {
                    Log.e("PhoneClaw", "Screenshot failed with error code: $errorCode")
                    latch.countDown()
                }
            }
        )

        try {
            latch.await(2000, java.util.concurrent.TimeUnit.MILLISECONDS)
        } catch (e: InterruptedException) {
            Log.e("PhoneClaw", "Screenshot timeout")
            return null
        }
        
        return bitmap?.let { bmp ->
            val stream = java.io.ByteArrayOutputStream()
            bmp.compress(android.graphics.Bitmap.CompressFormat.PNG, 80, stream)
            bmp.recycle()
            stream.toByteArray()
        }
    }
}
