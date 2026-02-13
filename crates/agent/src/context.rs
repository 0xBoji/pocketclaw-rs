use pocketclaw_core::permissions::ApprovedSkills;
use pocketclaw_core::types::{Message, Role};
use pocketclaw_skills::SkillsLoader;
use std::collections::HashSet;
use std::path::PathBuf;

/// Maximum number of conversation history messages to include in context.
/// This prevents exceeding LLM token limits as conversations grow.
const MAX_HISTORY_MESSAGES: usize = 20;

pub struct ContextBuilder {
    workspace: PathBuf,
    skills_loader: SkillsLoader,
    approved_skills: ApprovedSkills,
}

impl ContextBuilder {
    const ALLOW_ALL_MARKER: &'static str = "*";
    const SAFE_DEFAULT_TOOLS: [&'static str; 8] = [
        "read_file",
        "list_dir",
        "web_fetch",
        "web_search",
        "sessions_list",
        "sessions_history",
        "channel_health",
        "datetime_now",
    ];

    pub fn new(workspace: PathBuf) -> Self {
        let approved_skills = ApprovedSkills::load(&ApprovedSkills::default_path());
        Self {
            workspace: workspace.clone(),
            skills_loader: SkillsLoader::new(workspace),
            approved_skills,
        }
    }

    /// Get the set of tools allowed by currently approved skills.
    /// Strict default-deny remains: no approved skills => no tools.
    ///
    /// Compatibility rules for approved legacy skills:
    /// - No permissions block => allow all registered tools.
    /// - Empty permissions.tools => allow all registered tools.
    pub fn get_allowed_tools(&self) -> Vec<String> {
        let mut allowed = HashSet::new();
        let skills = self.skills_loader.list_skills();

        for skill in skills {
            if self.approved_skills.is_approved(&skill.name) {
                match &skill.permissions {
                    Some(perms) => {
                        if perms.tools.is_empty() {
                            return vec![Self::ALLOW_ALL_MARKER.to_string()];
                        }
                        for tool in &perms.tools {
                            allowed.insert(tool.clone());
                        }
                    }
                    None => return vec![Self::ALLOW_ALL_MARKER.to_string()],
                }
            }
        }
        if allowed.is_empty() {
            return Self::SAFE_DEFAULT_TOOLS
                .iter()
                .map(|t| t.to_string())
                .collect();
        }
        allowed.into_iter().collect()
    }

    pub fn build(
        &self,
        history: &[Message],
        summary: Option<&str>,
        current_message: &str,
    ) -> Vec<Message> {
        let mut messages = Vec::new();

        // 1. System Prompt
        let system_prompt = self.build_system_prompt();
        messages.push(Message::new("system", "global", Role::System, &system_prompt));

        // 2. Summary (Long-term memory or compressed context)
        if let Some(s) = summary {
            messages.push(Message::new(
                "system",
                "global",
                Role::System,
                &format!("Previous conversation summary: {}", s),
            ));
        }

        // 3. Relevant Skills (load always-on skills)
        // Only load APPROVED skills
        let skills = self.skills_loader.list_skills();
        for skill in skills {
            if self.approved_skills.is_approved(&skill.name) && skill.always && skill.available {
                messages.push(Message::new(
                    "system",
                    "global",
                    Role::System,
                    &format!("Skill: {}\n{}", skill.name, skill.content),
                ));
            }
        }

        // 4. Conversation History (sliding window — only last N messages)
        let history_window = if history.len() > MAX_HISTORY_MESSAGES {
            // Include a note that older messages were trimmed
            messages.push(Message::new(
                "system",
                "global",
                Role::System,
                &format!(
                    "[{} older messages omitted — see summary above for context]",
                    history.len() - MAX_HISTORY_MESSAGES
                ),
            ));
            &history[history.len() - MAX_HISTORY_MESSAGES..]
        } else {
            history
        };
        messages.extend_from_slice(history_window);

        // 5. Current Message
        messages.push(Message::new(
            "cli",
            "current",
            Role::User,
            current_message,
        ));

        messages
    }

    fn build_system_prompt(&self) -> String {
        let mut prompt = String::from("You are PocketClaw, an intelligent AI assistant.\n");
        prompt.push_str("You must answer the user's request accurately and concisely.\n");
        prompt.push_str("If you need to perform actions, use the provided tools.\n");

        // Load workspace context files if they exist
        let context_files = ["AGENTS.md", "SOUL.md", "USER.md", "TOOLS.md", "IDENTITY.md"];
        for filename in &context_files {
            let path = self.workspace.join(filename);
            if let Ok(content) = std::fs::read_to_string(&path) {
                prompt.push_str(&format!("\n--- {} ---\n{}\n", filename, content));
            }
        }

        prompt
    }
}
