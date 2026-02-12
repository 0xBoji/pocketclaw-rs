package com.pocketclaw.app.wave

import android.os.Bundle
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import java.io.File

class WorkspaceCreatorActivity : AppCompatActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val store = AppConfigStore(this)
        val config = store.load()

        val (scroll, root) = UiFactory.screen(this)
        root.addView(UiFactory.title(this, "Screen 1: Workspace Creator"))
        root.addView(UiFactory.subtitle(this, "Chon workspace local de chay gateway tren Android."))

        root.addView(UiFactory.label(this, "Workspace Path"))
        val workspaceInput = UiFactory.input(this, File(filesDir, "workspace").absolutePath)
        workspaceInput.setText(if (config.workspace.isBlank()) File(filesDir, "workspace").absolutePath else config.workspace)
        root.addView(workspaceInput)
        root.addView(UiFactory.hint(this, "Nen dung thu muc trong app storage de tranh loi quyen truy cap."))

        val saveBtn = UiFactory.actionButton(this, "Save Workspace")
        saveBtn.setOnClickListener {
            val path = workspaceInput.text.toString().trim()
            if (path.isEmpty()) {
                Toast.makeText(this, "Workspace path khong duoc rong", Toast.LENGTH_SHORT).show()
                return@setOnClickListener
            }

            val dir = File(path)
            if (!dir.exists()) dir.mkdirs()

            config.workspace = dir.absolutePath
            store.save(config)
            Toast.makeText(this, "Da luu workspace", Toast.LENGTH_SHORT).show()
            finish()
        }
        root.addView(saveBtn)

        setContentView(scroll)
    }
}
