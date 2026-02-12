package com.pocketclaw.app.wave

import android.content.Context
import android.graphics.Color
import android.graphics.Typeface
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

object UiFactory {
    fun screen(context: Context): Pair<ScrollView, LinearLayout> {
        val scroll = ScrollView(context).apply {
            setBackgroundColor(Color.parseColor("#10131a"))
            setPadding(36, 36, 36, 36)
        }
        val root = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER_HORIZONTAL
        }
        scroll.addView(root)
        return Pair(scroll, root)
    }

    fun title(context: Context, text: String): TextView = TextView(context).apply {
        this.text = text
        textSize = 24f
        setTextColor(Color.WHITE)
        typeface = Typeface.DEFAULT_BOLD
        setPadding(0, 16, 0, 8)
    }

    fun subtitle(context: Context, text: String): TextView = TextView(context).apply {
        this.text = text
        textSize = 13f
        setTextColor(Color.parseColor("#9aa3b2"))
        setPadding(0, 0, 0, 28)
    }

    fun section(context: Context, text: String): TextView = TextView(context).apply {
        this.text = text
        textSize = 17f
        setTextColor(Color.parseColor("#7dd3fc"))
        typeface = Typeface.DEFAULT_BOLD
        setPadding(0, 20, 0, 10)
    }

    fun label(context: Context, text: String): TextView = TextView(context).apply {
        this.text = text
        textSize = 13f
        setTextColor(Color.parseColor("#d1d5db"))
        setPadding(0, 8, 0, 6)
    }

    fun hint(context: Context, text: String): TextView = TextView(context).apply {
        this.text = text
        textSize = 11f
        setTextColor(Color.parseColor("#6b7280"))
        setPadding(0, 4, 0, 14)
    }

    fun input(context: Context, hint: String, multiline: Boolean = false, secret: Boolean = false): EditText {
        val inputType = when {
            secret -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
            multiline -> InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            else -> InputType.TYPE_CLASS_TEXT
        }

        return EditText(context).apply {
            this.hint = hint
            this.inputType = inputType
            textSize = 14f
            setTextColor(Color.WHITE)
            setHintTextColor(Color.parseColor("#5f6673"))
            setBackgroundColor(Color.parseColor("#1b2330"))
            setPadding(20, 20, 20, 20)
            if (multiline) {
                minLines = 3
                gravity = Gravity.TOP or Gravity.START
            }
        }
    }

    fun spacer(context: Context, h: Int = 18): View = View(context).apply {
        layoutParams = LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, h)
    }

    fun actionButton(context: Context, text: String): Button = Button(context).apply {
        this.text = text
        textSize = 15f
        setTextColor(Color.WHITE)
        setBackgroundColor(Color.parseColor("#2563eb"))
        setPadding(20, 22, 20, 22)
    }

    fun secondaryButton(context: Context, text: String): Button = Button(context).apply {
        this.text = text
        textSize = 14f
        setTextColor(Color.parseColor("#d1d5db"))
        setBackgroundColor(Color.parseColor("#263244"))
        setPadding(20, 18, 20, 18)
    }
}
