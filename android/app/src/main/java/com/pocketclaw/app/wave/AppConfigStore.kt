package com.pocketclaw.app.wave

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

data class AppConfigData(
    var workspace: String = "",
    var provider: String = "openai",
    var apiKey: String = "",
    var model: String = "",
    var systemPrompt: String = "You are a helpful AI assistant.",
    var groqKey: String = "",
    var telegramToken: String = "",
    var discordToken: String = "",
    var braveKey: String = "",
    var sheetId: String = "",
    var serviceAccountJson: String = "",
    var gatewayAuthToken: String = "",
    var sandboxExecEnabled: Boolean = true,
    var sandboxTimeoutSecs: Int = 30,
    var sandboxMaxOutputBytes: Int = 65536,
    var sandboxNetworkAllowlist: String = ""
)

class AppConfigStore(private val context: Context) {
    private val configDir: File = File(context.filesDir, ".pocketclaw")
    private val configFile: File = File(configDir, "config.json")

    fun ensureDirs() {
        if (!configDir.exists()) configDir.mkdirs()
        val workspace = File(context.filesDir, "workspace")
        if (!workspace.exists()) workspace.mkdirs()
    }

    fun hasConfig(): Boolean = configFile.exists()

    fun configPath(): String = configFile.absolutePath

    fun load(): AppConfigData {
        ensureDirs()
        if (!configFile.exists()) {
            return AppConfigData(workspace = File(context.filesDir, "workspace").absolutePath)
        }

        return try {
            val json = JSONObject(configFile.readText())
            val data = AppConfigData()
            data.workspace = json.optString("workspace", File(context.filesDir, "workspace").absolutePath)

            val providers = json.optJSONObject("providers")
            if (providers != null) {
                val priority = listOf("openai", "google", "anthropic", "openrouter", "groq")
                for (name in priority) {
                    if (providers.has(name)) {
                        data.provider = name
                        val p = providers.optJSONObject(name)
                        data.apiKey = p?.optString("api_key", "") ?: ""
                        data.model = p?.optString("model", "") ?: ""
                        break
                    }
                }

                val groqObj = providers.optJSONObject("groq")
                if (groqObj != null) {
                    data.groqKey = groqObj.optString("api_key", "")
                }
            }

            val agents = json.optJSONObject("agents")
            val defaultAgent = agents?.optJSONObject("default")
            if (defaultAgent != null) {
                data.systemPrompt = defaultAgent.optString("system_prompt", data.systemPrompt)
                if (data.model.isEmpty()) {
                    data.model = defaultAgent.optString("model", "")
                }
            }

            val telegram = json.optJSONObject("telegram")
            if (telegram != null) data.telegramToken = telegram.optString("token", "")

            val discord = json.optJSONObject("discord")
            if (discord != null) data.discordToken = discord.optString("token", "")

            val web = json.optJSONObject("web")
            if (web != null) {
                data.braveKey = web.optString("brave_key", "")
                data.gatewayAuthToken = web.optString("auth_token", "")
            }

            val sheets = json.optJSONObject("google_sheets")
            if (sheets != null) {
                data.sheetId = sheets.optString("spreadsheet_id", "")
                data.serviceAccountJson = sheets.optString("service_account_json", "")
            }

            val sandbox = json.optJSONObject("sandbox")
            if (sandbox != null) {
                data.sandboxExecEnabled = sandbox.optBoolean("exec_enabled", true)
                data.sandboxTimeoutSecs = sandbox.optInt("exec_timeout_secs", 30)
                data.sandboxMaxOutputBytes = sandbox.optInt("max_output_bytes", 65536)
                val allowlist = sandbox.optJSONArray("network_allowlist")
                if (allowlist != null) {
                    val domains = mutableListOf<String>()
                    for (i in 0 until allowlist.length()) {
                        domains.add(allowlist.optString(i))
                    }
                    data.sandboxNetworkAllowlist = domains.joinToString(",")
                }
            }

            data
        } catch (_: Exception) {
            AppConfigData(workspace = File(context.filesDir, "workspace").absolutePath)
        }
    }

    fun save(data: AppConfigData) {
        ensureDirs()

        val providers = JSONObject()
        val providerObj = JSONObject().apply {
            put("api_key", data.apiKey)
            put("model", data.model)
            if (data.provider == "openrouter") {
                put("api_base", "https://openrouter.ai/api/v1")
            }
        }
        providers.put(data.provider, providerObj)

        if (data.groqKey.isNotBlank() && data.provider != "groq") {
            providers.put("groq", JSONObject().put("api_key", data.groqKey))
        }

        val webObj = JSONObject()
        if (data.braveKey.isNotBlank()) webObj.put("brave_key", data.braveKey)
        if (data.gatewayAuthToken.isNotBlank()) webObj.put("auth_token", data.gatewayAuthToken)

        val sandboxAllowlist = JSONArray()
        data.sandboxNetworkAllowlist
            .split(",")
            .map { it.trim() }
            .filter { it.isNotBlank() }
            .forEach { sandboxAllowlist.put(it) }

        val root = JSONObject().apply {
            put("workspace", data.workspace)
            put("providers", providers)
            put("agents", JSONObject().put("default", JSONObject().apply {
                put("model", data.model)
                put("system_prompt", data.systemPrompt)
                put("max_tokens", 4096)
                put("temperature", 0.7)
            }))

            if (data.telegramToken.isNotBlank()) {
                put("telegram", JSONObject().put("token", data.telegramToken))
            }
            if (data.discordToken.isNotBlank()) {
                put("discord", JSONObject().put("token", data.discordToken))
            }
            if (webObj.length() > 0) put("web", webObj)
            if (data.sheetId.isNotBlank() && data.serviceAccountJson.isNotBlank()) {
                put("google_sheets", JSONObject().apply {
                    put("spreadsheet_id", data.sheetId)
                    put("service_account_json", data.serviceAccountJson)
                })
            }

            // Reserved for Android safety panel; ignored by backend if unsupported.
            put("sandbox", JSONObject().apply {
                put("exec_enabled", data.sandboxExecEnabled)
                put("exec_timeout_secs", data.sandboxTimeoutSecs)
                put("max_output_bytes", data.sandboxMaxOutputBytes)
                put("network_allowlist", sandboxAllowlist)
            })
        }

        configFile.writeText(root.toString(2))
    }

    fun approvedSkillsFile(): File {
        val dir = File(context.filesDir, ".pocketclaw")
        if (!dir.exists()) dir.mkdirs()
        return File(dir, "approved_skills.json")
    }
}
