package com.pocketclaw.app.wave

import android.os.Bundle
import android.widget.LinearLayout
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

class SkillsPermissionsActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val config = store.load()

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Screen 4: Skill Permissions"))
        root.addView(UiFactory.subtitle(this, "Xem skill manifests trong workspace va cap quyen su dung."))
        root.addView(UiFactory.hint(this, "Workspace: ${config.workspace}"))
        root.addView(UiFactory.hint(this, "Scan path: ${File(config.workspace, "skills").absolutePath}"))

        val skills = discoverSkills(config.workspace)
        val approved = loadApproved(store.approvedSkillsFile())

        val switches = mutableListOf<Pair<String, Switch>>()

        if (skills.isEmpty()) {
            root.addView(UiFactory.hint(this, "Khong tim thay skill nao trong workspace/skills"))
        } else {
            for (skill in skills) {
                val card = LinearLayout(this).apply {
                    orientation = LinearLayout.VERTICAL
                    setPadding(18, 18, 18, 18)
                    setBackgroundColor(0xFF1B2330.toInt())
                }

                val title = TextView(this).apply {
                    text = "${skill.name} (${skill.path})"
                    textSize = 14f
                    setTextColor(0xFFD1D5DB.toInt())
                }
                card.addView(title)

                if (skill.tools.isNotEmpty()) {
                    card.addView(UiFactory.hint(this, "Tools: ${skill.tools.joinToString(", ")}"))
                }

                val sw = Switch(this).apply {
                    text = "Approved"
                    setTextColor(0xFF9AA3B2.toInt())
                    isChecked = approved.contains(skill.name)
                }
                card.addView(sw)
                switches.add(skill.name to sw)

                root.addView(card)
                root.addView(UiFactory.spacer(this, 12))
            }
        }

        val saveBtn = UiFactory.actionButton(this, "Save Skill Approvals")
        saveBtn.setOnClickListener {
            val arr = JSONArray()
            for ((name, sw) in switches) {
                if (sw.isChecked) arr.put(name)
            }
            val obj = JSONObject().put("approved", arr)
            store.approvedSkillsFile().writeText(obj.toString(2))
            Toast.makeText(this, "Da luu approved skills", Toast.LENGTH_SHORT).show()
            finish()
        }
        root.addView(saveBtn)

        setContentView(scroll)
    }

    private fun loadApproved(file: File): Set<String> {
        if (!file.exists()) return emptySet()
        return try {
            val json = JSONObject(file.readText())
            val arr = json.optJSONArray("approved") ?: JSONArray()
            buildSet {
                for (i in 0 until arr.length()) add(arr.optString(i))
            }
        } catch (_: Exception) {
            emptySet()
        }
    }

    private fun discoverSkills(workspace: String): List<SkillView> {
        val skillsDir = File(workspace, "skills")
        if (!skillsDir.exists()) return emptyList()

        val found = mutableListOf<SkillView>()
        skillsDir.listFiles()?.forEach { dir ->
            if (!dir.isDirectory) return@forEach
            val toml = File(dir, "skill.toml")
            val skillMd = File(dir, "SKILL.md")
            if (!toml.exists() && !skillMd.exists()) return@forEach

            if (toml.exists()) {
                val text = toml.readText()
                val name = Regex("name\\s*=\\s*\"([^\"]+)\"").find(text)?.groupValues?.get(1) ?: dir.name
                val toolsBlock = Regex("tools\\s*=\\s*\\[([^\\]]+)\\]", RegexOption.DOT_MATCHES_ALL)
                    .find(text)
                    ?.groupValues
                    ?.get(1)
                    .orEmpty()
                val tools = Regex("\"([^\"]+)\"").findAll(toolsBlock).map { it.groupValues[1] }.toList()
                found.add(SkillView(name = name, path = toml.absolutePath, tools = tools))
                return@forEach
            }

            val legacyText = skillMd.readText()
            val legacyName = Regex("(?m)^name:\\s*([A-Za-z0-9_\\-]+)\\s*$")
                .find(legacyText)
                ?.groupValues
                ?.get(1)
                ?: dir.name
            val knownTools = listOf(
                "android_screen",
                "android_action",
                "web_fetch",
                "web_search",
                "exec_cmd",
                "read_file",
                "write_file",
                "list_dir",
                "sessions_list",
                "sessions_history",
                "sessions_send",
                "channel_health",
                "metrics_snapshot",
                "datetime_now"
            )
            val legacyTools = knownTools.filter { legacyText.contains("`$it`") }
            found.add(SkillView(name = legacyName, path = skillMd.absolutePath, tools = legacyTools))
        }
        return found.sortedBy { it.name.lowercase() }
    }
}

data class SkillView(
    val name: String,
    val path: String,
    val tools: List<String>
)
