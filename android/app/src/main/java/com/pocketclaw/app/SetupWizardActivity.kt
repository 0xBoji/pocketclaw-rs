package com.pocketclaw.app

import android.content.Intent
import android.os.Bundle
import android.view.Gravity
import android.widget.ArrayAdapter
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import android.widget.Spinner
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import com.pocketclaw.app.wave.AppConfigData
import com.pocketclaw.app.wave.AppConfigStore
import com.pocketclaw.app.wave.ControllerDashboardActivity
import com.pocketclaw.app.wave.ModelCatalog
import com.pocketclaw.app.wave.UiFactory

class SetupWizardActivity : AppCompatActivity() {
    private enum class Mode { QUICKSTART, MANUAL }

    private val providers = ModelCatalog.providers
    private val providerModels = ModelCatalog.providerModels
    private val channels = arrayOf("telegram", "discord", "slack", "whatsapp")
    private val channelLabels = mapOf(
        "telegram" to "Telegram",
        "discord" to "Discord",
        "slack" to "Slack",
        "whatsapp" to "WhatsApp",
    )
    private val assistantAddressOptions = arrayOf("minh", "toi", "tro ly")
    private val userAddressOptions = arrayOf("ban", "anh/chi", "quy khach")
    private val toneOptions = arrayOf("than thien, ngan gon", "chuyen nghiep", "tu nhien")

    private lateinit var store: AppConfigStore
    private lateinit var config: AppConfigData

    private var mode: Mode = Mode.QUICKSTART
    private var step: Int = 0

    private var selectedProvider: String = "openai"
    private var selectedModel: String = "gpt-5.2-mini"
    private var apiKey: String = ""
    private var selectedChannel: String = "telegram"
    private var channelApiKey: String = ""
    private val channelKeyDrafts = mutableMapOf<String, String>()
    private var assistantSelfAddress: String = "minh"
    private var userAddress: String = "ban"
    private var addressingTone: String = "than thien, ngan gon"

    private lateinit var contentRoot: LinearLayout

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        store = AppConfigStore(this)
        config = store.load()

        selectedProvider = config.provider.ifBlank { providers.firstOrNull() ?: "openai" }
        if (selectedProvider !in providers) {
            selectedProvider = providers.firstOrNull() ?: "openai"
        }
        selectedModel = config.model.ifBlank {
            providerModels[selectedProvider]?.firstOrNull() ?: "gpt-5.2-mini"
        }
        apiKey = config.apiKey

        val defaultChannel = when {
            config.telegramToken.isNotBlank() -> "telegram"
            config.discordToken.isNotBlank() -> "discord"
            config.slackBotToken.isNotBlank() -> "slack"
            config.whatsappToken.isNotBlank() -> "whatsapp"
            else -> "telegram"
        }
        channelKeyDrafts["telegram"] = config.telegramToken
        channelKeyDrafts["discord"] = config.discordToken
        channelKeyDrafts["slack"] = config.slackBotToken
        channelKeyDrafts["whatsapp"] = config.whatsappToken
        selectedChannel = defaultChannel
        channelApiKey = channelKeyFor(selectedChannel)
        assistantSelfAddress = config.assistantSelfAddress.takeIf { it in assistantAddressOptions } ?: "minh"
        userAddress = config.userAddress.takeIf { it in userAddressOptions } ?: "ban"
        addressingTone = config.addressingTone.takeIf { it in toneOptions } ?: "than thien, ngan gon"

        val frame = FrameLayout(this)
        val (scroll, root) = UiFactory.screen(this)
        contentRoot = root
        frame.addView(scroll)

        val settingsBtn = UiFactory.secondaryButton(this, "Settings")
        val settingsLp = FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.WRAP_CONTENT,
            FrameLayout.LayoutParams.WRAP_CONTENT
        ).apply {
            gravity = Gravity.END or Gravity.BOTTOM
            marginEnd = 28
            bottomMargin = 28
        }
        settingsBtn.layoutParams = settingsLp
        settingsBtn.setOnClickListener {
            startActivity(Intent(this, SetupActivity::class.java))
        }
        frame.addView(settingsBtn)

        setContentView(frame)
        renderStep()
    }

    private fun renderStep() {
        contentRoot.removeAllViews()
        contentRoot.addView(UiFactory.title(this, "PocketClaw Setup Wizard"))
        contentRoot.addView(UiFactory.subtitle(this, "Step ${step + 1}/7"))
        contentRoot.addView(UiFactory.hint(this, progressDots(step, 7)))

        when (step) {
            0 -> renderModeStep()
            1 -> renderProviderStep()
            2 -> renderApiKeyStep()
            3 -> renderModelStep()
            4 -> renderChannelStep()
            5 -> renderChannelKeyStep()
            6 -> renderAddressingStep()
        }

        contentRoot.addView(UiFactory.spacer(this, 20))
        renderNav()
    }

    private fun renderModeStep() {
        contentRoot.addView(UiFactory.section(this, "Choose Setup Type"))
        contentRoot.addView(UiFactory.hint(this, "Quickstart is recommended."))

        val quickId = 1
        val manualId = 2
        val radio = RadioGroup(this).apply {
            orientation = RadioGroup.VERTICAL
            addView(RadioButton(this@SetupWizardActivity).apply {
                id = quickId
                text = "Quickstart"
                textSize = 15f
                setTextColor(0xFFE5E7EB.toInt())
            })
            addView(RadioButton(this@SetupWizardActivity).apply {
                id = manualId
                text = "Manual"
                textSize = 15f
                setTextColor(0xFFE5E7EB.toInt())
            })
            check(if (mode == Mode.QUICKSTART) quickId else manualId)
            setOnCheckedChangeListener { _, checkedId ->
                mode = if (checkedId == manualId) Mode.MANUAL else Mode.QUICKSTART
            }
        }
        contentRoot.addView(radio)
    }

    private fun renderProviderStep() {
        contentRoot.addView(UiFactory.section(this, "Model/Auth Provider"))
        contentRoot.addView(UiFactory.label(this, "Select provider"))

        val spinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@SetupWizardActivity,
                android.R.layout.simple_spinner_dropdown_item,
                providers
            )
            setSelection(providers.indexOf(selectedProvider).coerceAtLeast(0))
        }
        contentRoot.addView(spinner)

        spinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                selectedProvider = providers[position]
                val models = providerModels[selectedProvider].orEmpty()
                if (selectedModel !in models && models.isNotEmpty()) {
                    selectedModel = models.first()
                }
            }

            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })
    }

    private fun renderApiKeyStep() {
        contentRoot.addView(UiFactory.section(this, "Enter API Key"))
        contentRoot.addView(UiFactory.label(this, "API key for $selectedProvider"))
        val apiInput = UiFactory.input(this, "Enter API key", secret = true)
        apiInput.setText(apiKey)
        contentRoot.addView(apiInput)
        apiInput.addTextChangedListener(SimpleTextWatcher { apiKey = it })
    }

    private fun renderModelStep() {
        contentRoot.addView(UiFactory.section(this, "Choose Model"))
        val models = providerModels[selectedProvider].orEmpty()
        contentRoot.addView(UiFactory.label(this, "Select model for $selectedProvider"))

        val spinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@SetupWizardActivity,
                android.R.layout.simple_spinner_dropdown_item,
                models
            )
            val index = models.indexOf(selectedModel).coerceAtLeast(0)
            setSelection(index)
        }
        contentRoot.addView(spinner)
        spinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                if (position in models.indices) {
                    selectedModel = models[position]
                }
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })
    }

    private fun renderChannelStep() {
        contentRoot.addView(UiFactory.section(this, "Select Channel (Quickstart)"))
        val channelDisplay = channels.map { channelLabels[it] ?: it }
        val spinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@SetupWizardActivity,
                android.R.layout.simple_spinner_dropdown_item,
                channelDisplay
            )
            setSelection(channels.indexOf(selectedChannel).coerceAtLeast(0))
        }
        contentRoot.addView(spinner)
        spinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                channelKeyDrafts[selectedChannel] = channelApiKey
                selectedChannel = channels[position]
                channelApiKey = channelKeyFor(selectedChannel)
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })
    }

    private fun renderChannelKeyStep() {
        contentRoot.addView(UiFactory.section(this, "Channel API Key"))
        val selectedLabel = channelLabels[selectedChannel] ?: selectedChannel
        contentRoot.addView(UiFactory.label(this, "Enter key/token for $selectedLabel"))
        val keyInput = UiFactory.input(this, channelApiKeyPlaceholder(selectedChannel), secret = true)
        keyInput.setText(channelApiKey)
        contentRoot.addView(keyInput)
        keyInput.addTextChangedListener(SimpleTextWatcher {
            channelApiKey = it
            channelKeyDrafts[selectedChannel] = it
        })
        contentRoot.addView(UiFactory.hint(this, "Press Next to configure addressing style."))
    }

    private fun renderAddressingStep() {
        contentRoot.addView(UiFactory.section(this, "Addressing Style"))
        contentRoot.addView(UiFactory.hint(this, "Applied from the first chat message after setup."))

        contentRoot.addView(UiFactory.label(this, "Assistant refers to self as"))
        val assistantSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@SetupWizardActivity,
                android.R.layout.simple_spinner_dropdown_item,
                assistantAddressOptions
            )
            setSelection(assistantAddressOptions.indexOf(assistantSelfAddress).coerceAtLeast(0))
        }
        assistantSpinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                assistantSelfAddress = assistantAddressOptions[position]
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })
        contentRoot.addView(assistantSpinner)

        contentRoot.addView(UiFactory.label(this, "Assistant addresses user as"))
        val userSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@SetupWizardActivity,
                android.R.layout.simple_spinner_dropdown_item,
                userAddressOptions
            )
            setSelection(userAddressOptions.indexOf(userAddress).coerceAtLeast(0))
        }
        userSpinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                userAddress = userAddressOptions[position]
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })
        contentRoot.addView(userSpinner)

        contentRoot.addView(UiFactory.label(this, "Tone"))
        val toneSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@SetupWizardActivity,
                android.R.layout.simple_spinner_dropdown_item,
                toneOptions
            )
            setSelection(toneOptions.indexOf(addressingTone).coerceAtLeast(0))
        }
        toneSpinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                addressingTone = toneOptions[position]
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })
        contentRoot.addView(toneSpinner)
    }

    private fun renderNav() {
        val nav = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER
        }

        val backBtn = UiFactory.secondaryButton(this, "Back")
        backBtn.isEnabled = step > 0
        backBtn.alpha = if (step > 0) 1f else 0.4f
        backBtn.setOnClickListener {
            if (step > 0) {
                step -= 1
                renderStep()
            }
        }
        nav.addView(backBtn)

        val nextBtn = UiFactory.actionButton(this, if (step == 6) "Done" else "Next")
        nextBtn.setOnClickListener {
            onNext()
        }
        nav.addView(nextBtn)

        contentRoot.addView(nav)
    }

    private fun onNext() {
        if (step == 0 && mode == Mode.MANUAL) {
            startActivity(Intent(this, SetupActivity::class.java))
            finish()
            return
        }

        if (step == 2 && apiKey.isBlank()) {
            Toast.makeText(this, "API key is required", Toast.LENGTH_SHORT).show()
            return
        }

        if (step == 5 && channelApiKey.isBlank()) {
            Toast.makeText(this, "Channel API key is required", Toast.LENGTH_SHORT).show()
            return
        }

        if (step < 6) {
            step += 1
            renderStep()
            return
        }

        saveConfigAndDone()
    }

    private fun saveConfigAndDone() {
        config.provider = selectedProvider
        config.apiKey = apiKey
        config.model = selectedModel
        config.systemPrompt = config.systemPrompt.ifBlank { "You are a helpful AI assistant." }

        when (selectedChannel) {
            "telegram" -> config.telegramToken = channelApiKey
            "discord" -> config.discordToken = channelApiKey
            "slack" -> config.slackBotToken = channelApiKey
            "whatsapp" -> config.whatsappToken = channelApiKey
        }
        config.assistantSelfAddress = assistantSelfAddress
        config.userAddress = userAddress
        config.addressingTone = addressingTone

        store.save(config)
        Toast.makeText(this, "Setup complete", Toast.LENGTH_SHORT).show()
        startActivity(Intent(this, ControllerDashboardActivity::class.java))
        finish()
    }

    private fun channelKeyFor(channel: String): String {
        channelKeyDrafts[channel]?.let { return it }
        return when (channel) {
            "telegram" -> config.telegramToken
            "discord" -> config.discordToken
            "slack" -> config.slackBotToken
            "whatsapp" -> config.whatsappToken
            else -> ""
        }
    }

    private fun channelApiKeyPlaceholder(channel: String): String {
        return when (channel) {
            "telegram" -> "Telegram Bot Token (123456:ABC...)"
            "discord" -> "Discord Bot Token (MTk...)"
            "slack" -> "Slack Bot Token (xoxb-...)"
            "whatsapp" -> "WhatsApp Access Token (EAAG...)"
            else -> "Enter API key"
        }
    }

    private fun progressDots(currentStep: Int, totalSteps: Int): String {
        val sb = StringBuilder()
        for (i in 0 until totalSteps) {
            if (i == currentStep) sb.append("●") else sb.append("○")
            if (i < totalSteps - 1) sb.append(" ")
        }
        return sb.toString()
    }
}

private class SimpleTextWatcher(private val onChanged: (String) -> Unit) : android.text.TextWatcher {
    override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) {}
    override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) {
        onChanged(s?.toString().orEmpty().trim())
    }
    override fun afterTextChanged(s: android.text.Editable?) {}
}
