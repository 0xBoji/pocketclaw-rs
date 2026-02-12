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
    pub fn new(workspace: PathBuf) -> Self {
        let approved_skills = ApprovedSkills::load(&ApprovedSkills::default_path());
        Self {
            workspace: workspace.clone(),
            skills_loader: SkillsLoader::new(workspace),
            approved_skills,
        }
    }

    /// Get the set of tools allowed by currently approved skills.
    /// If no skills are approved, returns empty set (unless core tools are implicitly allowed?).
    /// Note: Core tools should probably be allowed by default or managed by a "core" skill.
    /// For now, we'll assume core tools are NOT subject to skill permissions unless specified.
    /// Wait, the user requirement "Default Deny" implies strictness.
    /// But `registry.is_tool_allowed` returns true if allowed_tools is empty.
    /// To enforce deny, we must pass a non-empty list if we want to restrict.
    ///
    /// Strategy:
    /// - If no skills are approved, we might want to allow ONLY core tools (exec, fs, etc)?
    /// - Or allow NOTHING?
    ///
    /// Let's return a list of tool names.
    pub fn get_allowed_tools(&self) -> Vec<String> {
        let mut allowed = HashSet::new();
        let skills = self.skills_loader.list_skills();

        for skill in skills {
            if self.approved_skills.is_approved(&skill.name) {
                if let Some(perms) = &skill.permissions {
                    for tool in &perms.tools {
                        allowed.insert(tool.clone());
                    }
                }
            }
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
