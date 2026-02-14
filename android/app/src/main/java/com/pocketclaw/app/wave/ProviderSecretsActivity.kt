package com.pocketclaw.app.wave

import android.os.Bundle
import android.widget.ArrayAdapter
import android.widget.Spinner
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity

class ProviderSecretsActivity : AppCompatActivity() {
    private val assistantAddressOptions = arrayOf("minh", "toi", "tro ly")
    private val userAddressOptions = arrayOf("ban", "anh/chi", "quy khach")
    private val toneOptions = arrayOf("than thien, ngan gon", "chuyen nghiep", "tu nhien")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val config = store.load()

        val providers = ModelCatalog.providers
        val providerModels = ModelCatalog.providerModels
        var selectedModel = config.model
        val (scroll, root) = UiFactory.screen(this)

        root.addView(UiFactory.title(this, "Screen 2: Provider & Secrets"))
        root.addView(UiFactory.subtitle(this, "Nguoi dung chi can config key + model la dung duoc."))

        root.addView(UiFactory.section(this, "LLM Provider"))
        root.addView(UiFactory.label(this, "Provider"))
        val providerSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(this@ProviderSecretsActivity, android.R.layout.simple_spinner_dropdown_item, providers)
            setSelection(providers.indexOf(config.provider).coerceAtLeast(0))
        }
        root.addView(providerSpinner)

        root.addView(UiFactory.label(this, "API Key (required)"))
        val apiKeyInput = UiFactory.input(this, "sk-...", secret = true)
        apiKeyInput.setText(config.apiKey)
        root.addView(apiKeyInput)

        root.addView(UiFactory.label(this, "Model (required)"))
        val modelOptions = mutableListOf<String>()
        val modelAdapter = ArrayAdapter(this, android.R.layout.simple_spinner_dropdown_item, modelOptions)
        val modelSpinner = Spinner(this).apply { adapter = modelAdapter }
        root.addView(modelSpinner)

        root.addView(UiFactory.label(this, "System Prompt"))
        val promptInput = UiFactory.input(this, "You are a helpful AI assistant.", multiline = true)
        promptInput.setText(config.systemPrompt)
        root.addView(promptInput)

        root.addView(UiFactory.section(this, "Addressing"))
        root.addView(UiFactory.label(this, "Assistant refers to self as"))
        val assistantSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@ProviderSecretsActivity,
                android.R.layout.simple_spinner_dropdown_item,
                assistantAddressOptions
            )
            setSelection(assistantAddressOptions.indexOf(config.assistantSelfAddress).coerceAtLeast(0))
        }
        root.addView(assistantSpinner)

        root.addView(UiFactory.label(this, "Assistant addresses user as"))
        val userSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@ProviderSecretsActivity,
                android.R.layout.simple_spinner_dropdown_item,
                userAddressOptions
            )
            setSelection(userAddressOptions.indexOf(config.userAddress).coerceAtLeast(0))
        }
        root.addView(userSpinner)

        root.addView(UiFactory.label(this, "Tone"))
        val toneSpinner = Spinner(this).apply {
            adapter = ArrayAdapter(
                this@ProviderSecretsActivity,
                android.R.layout.simple_spinner_dropdown_item,
                toneOptions
            )
            setSelection(toneOptions.indexOf(config.addressingTone).coerceAtLeast(0))
        }
        root.addView(toneSpinner)

        root.addView(UiFactory.section(this, "Extra Secrets"))
        root.addView(UiFactory.label(this, "Groq API Key (voice optional)"))
        val groqInput = UiFactory.input(this, "gsk_...", secret = true)
        groqInput.setText(config.groqKey)
        root.addView(groqInput)

        root.addView(UiFactory.label(this, "Brave Search API Key (optional)"))
        val braveInput = UiFactory.input(this, "BSA...", secret = true)
        braveInput.setText(config.braveKey)
        root.addView(braveInput)

        root.addView(UiFactory.label(this, "Gateway Auth Token (optional)"))
        val authTokenInput = UiFactory.input(this, "Bearer token for API", secret = true)
        authTokenInput.setText(config.gatewayAuthToken)
        root.addView(authTokenInput)

        root.addView(UiFactory.label(this, "Google Sheets Spreadsheet ID (optional)"))
        val sheetInput = UiFactory.input(this, "1Abc...")
        sheetInput.setText(config.sheetId)
        root.addView(sheetInput)

        root.addView(UiFactory.label(this, "Service Account JSON (optional)"))
        val serviceAccountInput = UiFactory.input(this, "{...}", multiline = true)
        serviceAccountInput.setText(config.serviceAccountJson)
        root.addView(serviceAccountInput)

        root.addView(UiFactory.spacer(this))
        val saveBtn = UiFactory.actionButton(this, "Save Provider Settings")
        saveBtn.setOnClickListener {
            val key = apiKeyInput.text.toString().trim()
            val model = selectedModel.trim()
            if (key.isBlank() || model.isBlank()) {
                Toast.makeText(this, "API key va model la bat buoc", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }

            config.provider = providerSpinner.selectedItem.toString()
            config.apiKey = key
            config.model = model
            config.systemPrompt = promptInput.text.toString().trim().ifBlank { "You are a helpful AI assistant." }
            config.assistantSelfAddress = assistantSpinner.selectedItem.toString()
            config.userAddress = userSpinner.selectedItem.toString()
            config.addressingTone = toneSpinner.selectedItem.toString()
            config.groqKey = groqInput.text.toString().trim()
            config.braveKey = braveInput.text.toString().trim()
            config.gatewayAuthToken = authTokenInput.text.toString().trim()
            config.sheetId = sheetInput.text.toString().trim()
            config.serviceAccountJson = serviceAccountInput.text.toString().trim()

            store.save(config)
            Toast.makeText(this, "Da luu provider/secrets", Toast.LENGTH_SHORT).show()
            finish()
        }
        root.addView(saveBtn)

        fun refreshModels(provider: String) {
            val list = providerModels[provider].orEmpty().toMutableList()
            if (selectedModel.isNotBlank() && selectedModel !in list) {
                list.add(0, selectedModel)
            }
            if (list.isEmpty()) {
                list.add("gpt-5.2-mini")
            }
            modelOptions.clear()
            modelOptions.addAll(list)
            modelAdapter.notifyDataSetChanged()
            val index = modelOptions.indexOf(selectedModel).let { if (it >= 0) it else 0 }
            modelSpinner.setSelection(index)
            selectedModel = modelOptions[index]
        }

        providerSpinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                refreshModels(providers[position])
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })

        modelSpinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>?, view: android.view.View?, position: Int, id: Long) {
                if (position in modelOptions.indices) {
                    selectedModel = modelOptions[position]
                }
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>?) {}
        })

        refreshModels(config.provider)

        setContentView(scroll)
    }
}
