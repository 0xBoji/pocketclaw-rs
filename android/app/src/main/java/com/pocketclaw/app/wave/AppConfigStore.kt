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
    var whatsappToken: String = "",
    var whatsappPhoneNumberId: String = "",
    var whatsappDefaultTo: String = "",
    var whatsappVerifyToken: String = "",
    var whatsappAppSecret: String = "",
    var slackBotToken: String = "",
    var slackDefaultChannel: String = "",
    var braveKey: String = "",
    var sheetId: String = "",
    var serviceAccountJson: String = "",
    var gatewayAuthToken: String = "",
    var sandboxExecEnabled: Boolean = true,
    var sandboxTimeoutSecs: Int = 30,
    var sandboxMaxOutputBytes: Int = 65536,
    var sandboxNetworkAllowlist: String = "",
    var liteMode: Boolean = true,
    var wsHeartbeatSecs: Int = 30,
    var healthWindowMinutes: Int = 30,
    var dedupeMaxEntries: Int = 1024,
    var adapterMaxInflight: Int = 1,
    var adapterRetryJitterMs: Int = 250,
    var assistantSelfAddress: String = "minh",
    var userAddress: String = "ban",
    var addressingTone: String = "than thien, ngan gon"
)

class AppConfigStore(private val context: Context) {
    companion object {
        private const val PERSONA_BLOCK_START = "<!-- pocketclaw:persona:start -->"
        private const val PERSONA_BLOCK_END = "<!-- pocketclaw:persona:end -->"
        private const val DEFAULT_ANDROID_NAV_SKILL = """
---
name: android_nav
description: Interact with Android UI via accessibility tools.
---

# Android Navigation

Use the tools below when user asks to open apps or navigate UI:
- `android_screen`
- `android_action`

Workflow:
1. `android_action` with `home`.
2. `android_screen` with `dump_hierarchy`.
3. Find target app/button by text/content-desc.
4. `android_action` with `click` on matched bounds.
5. Repeat dump/click for next step.
"""
    }

    private val configDir: File = File(context.filesDir, ".pocketclaw")
    private val configFile: File = File(configDir, "config.json")

    fun ensureDirs() {
        if (!configDir.exists()) configDir.mkdirs()
        val workspace = File(context.filesDir, "workspace")
        if (!workspace.exists()) workspace.mkdirs()
        ensureWorkspaceScaffold(workspace.absolutePath)
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
            ensureWorkspaceScaffold(data.workspace)

            val providers = json.optJSONObject("providers")
            if (providers != null) {
                val priority = ModelCatalog.providers.toList()
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

            val whatsapp = json.optJSONObject("whatsapp")
            if (whatsapp != null) {
                data.whatsappToken = whatsapp.optString("token", "")
                data.whatsappPhoneNumberId = whatsapp.optString("phone_number_id", "")
                data.whatsappDefaultTo = whatsapp.optString("default_to", "")
                data.whatsappVerifyToken = whatsapp.optString("verify_token", "")
                data.whatsappAppSecret = whatsapp.optString("app_secret", "")
            }

            val slack = json.optJSONObject("slack")
            if (slack != null) {
                data.slackBotToken = slack.optString("bot_token", "")
                data.slackDefaultChannel = slack.optString("default_channel", "")
            }

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

            val runtime = json.optJSONObject("runtime")
            if (runtime != null) {
                data.wsHeartbeatSecs = runtime.optInt("ws_heartbeat_secs", data.wsHeartbeatSecs)
                data.healthWindowMinutes = runtime.optInt("health_window_minutes", data.healthWindowMinutes)
                data.dedupeMaxEntries = runtime.optInt("dedupe_max_entries", data.dedupeMaxEntries)
                data.adapterMaxInflight = runtime.optInt("adapter_max_inflight", data.adapterMaxInflight)
                data.adapterRetryJitterMs = runtime.optInt("adapter_retry_jitter_ms", data.adapterRetryJitterMs)
            }

            val persona = json.optJSONObject("persona")
            val addressing = persona?.optJSONObject("addressing")
            if (addressing != null) {
                data.assistantSelfAddress = addressing.optString("assistant_self", data.assistantSelfAddress)
                    .trim()
                    .ifBlank { data.assistantSelfAddress }
                data.userAddress = addressing.optString("user", data.userAddress)
                    .trim()
                    .ifBlank { data.userAddress }
                data.addressingTone = addressing.optString("tone", data.addressingTone)
                    .trim()
                    .ifBlank { data.addressingTone }
            }

            val isLiteProfile = data.wsHeartbeatSecs >= 25 &&
                data.healthWindowMinutes <= 35 &&
                data.dedupeMaxEntries <= 1200 &&
                data.adapterMaxInflight <= 1
            data.liteMode = isLiteProfile

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
            if (data.slackBotToken.isNotBlank()) {
                put("slack", JSONObject().apply {
                    put("bot_token", data.slackBotToken)
                    if (data.slackDefaultChannel.isNotBlank()) {
                        put("default_channel", data.slackDefaultChannel)
                    }
                })
            }
            if (data.whatsappToken.isNotBlank()) {
                put("whatsapp", JSONObject().apply {
                    put("token", data.whatsappToken)
                    if (data.whatsappPhoneNumberId.isNotBlank()) {
                        put("phone_number_id", data.whatsappPhoneNumberId)
                    }
                    if (data.whatsappDefaultTo.isNotBlank()) {
                        put("default_to", data.whatsappDefaultTo)
                    }
                    if (data.whatsappVerifyToken.isNotBlank()) {
                        put("verify_token", data.whatsappVerifyToken)
                    }
                    if (data.whatsappAppSecret.isNotBlank()) {
                        put("app_secret", data.whatsappAppSecret)
                    }
                })
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
            put("runtime", JSONObject().apply {
                put("ws_heartbeat_secs", data.wsHeartbeatSecs.coerceIn(3, 120))
                put("health_window_minutes", data.healthWindowMinutes.coerceIn(5, 60))
                put("dedupe_max_entries", data.dedupeMaxEntries.coerceIn(128, 20000))
                put("adapter_max_inflight", data.adapterMaxInflight.coerceIn(1, 8))
                put("adapter_retry_jitter_ms", data.adapterRetryJitterMs.coerceIn(0, 2000))
            })
            put("persona", JSONObject().put("addressing", JSONObject().apply {
                put("assistant_self", data.assistantSelfAddress.trim().ifBlank { "minh" })
                put("user", data.userAddress.trim().ifBlank { "ban" })
                put("tone", data.addressingTone.trim().ifBlank { "than thien, ngan gon" })
            }))
        }

        configFile.writeText(root.toString(2))
        ensureWorkspaceScaffold(data.workspace)
        syncUserProfile(data)
    }

    fun approvedSkillsFile(): File {
        val dir = File(context.filesDir, ".pocketclaw")
        if (!dir.exists()) dir.mkdirs()
        return File(dir, "approved_skills.json")
    }

    private fun syncUserProfile(data: AppConfigData) {
        val workspacePath = data.workspace.ifBlank { File(context.filesDir, "workspace").absolutePath }
        val workspaceDir = File(workspacePath)
        if (!workspaceDir.exists()) workspaceDir.mkdirs()

        val userFile = File(workspaceDir, "USER.md")
        val block = buildPersonaBlock(data).trim()
        val existing = if (userFile.exists()) userFile.readText() else ""

        val replaced = replacePersonaBlock(existing, block)
        userFile.writeText(replaced.trimEnd() + "\n")
    }

    private fun ensureWorkspaceScaffold(workspacePath: String) {
        val workspace = File(workspacePath.ifBlank { File(context.filesDir, "workspace").absolutePath })
        if (!workspace.exists()) workspace.mkdirs()

        val skillsDir = File(workspace, "skills")
        if (!skillsDir.exists()) skillsDir.mkdirs()

        val androidNavDir = File(skillsDir, "android_nav")
        if (!androidNavDir.exists()) androidNavDir.mkdirs()

        val skillFile = File(androidNavDir, "SKILL.md")
        if (!skillFile.exists() || skillFile.readText().isBlank()) {
            skillFile.writeText(DEFAULT_ANDROID_NAV_SKILL.trim() + "\n")
        }
    }

    private fun replacePersonaBlock(existing: String, block: String): String {
        val start = existing.indexOf(PERSONA_BLOCK_START)
        val end = existing.indexOf(PERSONA_BLOCK_END)
        val wrapped = "$PERSONA_BLOCK_START\n$block\n$PERSONA_BLOCK_END"

        if (start >= 0 && end > start) {
            val prefix = existing.substring(0, start).trimEnd()
            val suffix = existing.substring(end + PERSONA_BLOCK_END.length).trimStart()
            return when {
                prefix.isBlank() && suffix.isBlank() -> wrapped
                prefix.isBlank() -> "$wrapped\n\n$suffix"
                suffix.isBlank() -> "$prefix\n\n$wrapped"
                else -> "$prefix\n\n$wrapped\n\n$suffix"
            }
        }

        val base = existing.trimEnd()
        return if (base.isBlank()) wrapped else "$base\n\n$wrapped"
    }

    private fun buildPersonaBlock(data: AppConfigData): String {
        val assistant = data.assistantSelfAddress.trim().ifBlank { "minh" }
        val user = data.userAddress.trim().ifBlank { "ban" }
        val tone = data.addressingTone.trim().ifBlank { "than thien, ngan gon" }

        return """
## Preferred Addressing
- Refer to yourself as "$assistant".
- Address the user as "$user".
- Maintain tone: "$tone".
- Apply this from the first reply unless the user asks to change.
        """.trimIndent()
    }
}
