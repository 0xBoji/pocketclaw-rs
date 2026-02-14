use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::info;

/// Tracks which skills the user has approved.
/// Default-deny: skills must be explicitly approved before use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovedSkills {
    /// Set of approved skill names
    approved: HashSet<String>,
}

impl Default for ApprovedSkills {
    fn default() -> Self {
        Self {
            approved: HashSet::new(),
        }
    }
}

impl ApprovedSkills {
    /// Load approved skills from file, or return empty (default-deny).
    pub fn load(path: &Path) -> Self {
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(store) = serde_json::from_str::<ApprovedSkills>(&data) {
                    info!("Loaded {} approved skills", store.approved.len());
                    return store;
                }
            }
        }
        Self::default()
    }

    /// Save approved skills to file.
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self)?;
        std::fs::write(path, data)?;
        Ok(())
    }

    /// Check if a skill is approved.
    pub fn is_approved(&self, skill_name: &str) -> bool {
        self.approved.contains(skill_name)
    }

    /// Approve a skill (after user consent).
    pub fn approve(&mut self, skill_name: String) {
        self.approved.insert(skill_name);
    }

    /// Revoke approval for a skill.
    pub fn revoke(&mut self, skill_name: &str) {
        self.approved.remove(skill_name);
    }

    /// Get the default path for the approved skills file.
    pub fn default_path() -> PathBuf {
        if let Ok(explicit_path) = std::env::var("PHONECLAW_APPROVED_SKILLS_PATH") {
            if !explicit_path.trim().is_empty() {
                return PathBuf::from(explicit_path);
            }
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".phoneclaw/approved_skills.json")
    }
}

/// Result of checking a skill's permissions against a requested tool.
#[derive(Debug)]
pub enum PermissionCheck {
    /// Tool is allowed by the skill's permissions.
    Allowed,
    /// Skill not approved by user (default-deny).
    SkillNotApproved { skill_name: String },
    /// Tool not in the skill's allowed tool list.
    ToolNotAllowed { skill_name: String, tool: String },
}

/// Check if a skill is allowed to use a specific tool.
pub fn check_skill_permission(
    approved: &ApprovedSkills,
    skill_name: &str,
    allowed_tools: &[String],
    requested_tool: &str,
) -> PermissionCheck {
    // Default-deny: skill must be approved first
    if !approved.is_approved(skill_name) {
        return PermissionCheck::SkillNotApproved {
            skill_name: skill_name.to_string(),
        };
    }

    // Check if the tool is in the skill's allowed list
    // Empty list = all tools allowed (for approved skills)
    if !allowed_tools.is_empty() && !allowed_tools.iter().any(|t| t == requested_tool) {
        return PermissionCheck::ToolNotAllowed {
            skill_name: skill_name.to_string(),
            tool: requested_tool.to_string(),
        };
    }

    PermissionCheck::Allowed
}
