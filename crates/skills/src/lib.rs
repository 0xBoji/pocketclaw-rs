use serde::Deserialize;
use std::path::PathBuf;
use walkdir::WalkDir;
use regex::Regex;
use std::fs;
use tracing::{warn, error};

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub requirements: Option<SkillRequirements>,
    pub permissions: Option<SkillPermissions>,
    pub always: bool,
    pub available: bool,
    pub missing_requirements: Vec<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub always: bool,
    pub requires: Option<SkillRequirements>,
    pub permissions: Option<SkillPermissions>,
}

/// The TOML manifest structure for skill.toml
#[derive(Debug, Clone, Deserialize)]
pub struct SkillManifest {
    pub metadata: ManifestMetadata,
    pub permissions: Option<SkillPermissions>,
    pub requirements: Option<SkillRequirements>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManifestMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub always: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillRequirements {
    #[serde(default)]
    pub bins: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
}

/// Permission manifest for a skill — declares what the skill is allowed to do.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SkillPermissions {
    /// Tool names this skill is allowed to use (e.g. ["exec_cmd", "read_file"]).
    #[serde(default)]
    pub tools: Vec<String>,
    /// File system scope: "workspace" (default) or "system".
    #[serde(default = "default_fs_scope")]
    pub fs_scope: String,
    /// Allowed network domains for web tools (e.g. ["api.github.com"]).
    #[serde(default)]
    pub network_domains: Vec<String>,
    /// Override max exec timeout for this skill (seconds).
    pub max_exec_timeout: Option<u64>,
}

fn default_fs_scope() -> String {
    "workspace".to_string()
}

pub struct SkillsLoader {
    workspace_path: PathBuf,
}

impl SkillsLoader {
    pub fn new(workspace_path: PathBuf) -> Self {
        Self { workspace_path }
    }

    pub fn list_skills(&self) -> Vec<Skill> {
        let skills_dir = self.workspace_path.join("skills");
        let mut skills = Vec::new();

        if !skills_dir.exists() {
            return skills;
        }

        // We only want to look at immediate subdirectories of "skills"
        let entries = match fs::read_dir(&skills_dir) {
            Ok(e) => e,
            Err(e) => {
                warn!("Failed to read skills directory: {}", e);
                return skills;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Check for skill.toml first (A6: Manifest file)
            let manifest_path = path.join("skill.toml");
            if manifest_path.exists() {
                match self.load_skill_from_manifest(&manifest_path) {
                    Ok(skill) => {
                        skills.push(skill);
                        continue;
                    }
                    Err(e) => {
                        warn!("Failed to load skill manifest at {:?}: {}", manifest_path, e);
                        // Fallthrough to try SKILL.md? No, simpler to prioritize manifest if present.
                    }
                }
            }

            // Fallback to SKILL.md (Legacy)
            let skill_md_path = path.join("SKILL.md");
            if skill_md_path.exists() {
                match self.load_skill_md(&skill_md_path) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => warn!("Failed to load SKILL.md at {:?}: {}", skill_md_path, e),
                }
            }
        }

        skills
    }

    /// Load skill from skill.toml + README.md (or just description)
    fn load_skill_from_manifest(&self, path: &PathBuf) -> anyhow::Result<Skill> {
        let content = fs::read_to_string(path)?;
        let manifest: SkillManifest = toml::from_str(&content)?;

        // Attempt to load content from README.md in the same dir
        let readme_path = path.parent().unwrap().join("README.md");
        let body = if readme_path.exists() {
            fs::read_to_string(readme_path).unwrap_or_else(|_| manifest.metadata.description.clone())
        } else {
            manifest.metadata.description.clone()
        };

        let (available, missing) = self.check_requirements(&manifest.requirements);

        Ok(Skill {
            name: manifest.metadata.name,
            description: manifest.metadata.description,
            content: body,
            requirements: manifest.requirements,
            permissions: manifest.permissions,
            always: manifest.metadata.always,
            available,
            missing_requirements: missing,
            version: Some(manifest.metadata.version),
        })
    }

    /// Load legacy SKILL.md format
    fn load_skill_md(&self, path: &PathBuf) -> anyhow::Result<Skill> {
        let content = fs::read_to_string(path)?;
        let (frontmatter_str, body) = self.extract_frontmatter(&content);

        let metadata: SkillMetadata = serde_json::from_str(&frontmatter_str)
            .unwrap_or_else(|_| SkillMetadata {
                name: path.parent().unwrap().file_name().unwrap().to_string_lossy().to_string(),
                description: "No description provided".to_string(),
                always: false,
                requires: None,
                permissions: None,
            });

        let (available, missing) = self.check_requirements(&metadata.requires);

        Ok(Skill {
            name: metadata.name,
            description: metadata.description,
            content: body.to_string(),
            requirements: metadata.requires,
            permissions: metadata.permissions,
            always: metadata.always,
            available,
            missing_requirements: missing,
            version: None,
        })
    }

    fn extract_frontmatter<'a>(&self, content: &'a str) -> (String, &'a str) {
        let re = Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)").unwrap();
        if let Some(caps) = re.captures(content) {
            let frontmatter = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            let body = caps.get(2).map(|m| m.as_str()).unwrap_or(content);
            return (frontmatter.to_string(), body);
        }
        ("{}".to_string(), content)
    }

    fn check_requirements(&self, requires: &Option<SkillRequirements>) -> (bool, Vec<String>) {
        let mut missing = Vec::new();
        if let Some(req) = requires {
            for bin in &req.bins {
                 if which::which(bin).is_err() {
                     missing.push(format!("CLI: {}", bin));
                 }
            }
            for env in &req.env {
                if std::env::var(env).is_err() {
                    missing.push(format!("ENV: {}", env));
                }
            }
        }
        (missing.is_empty(), missing)
    }
}
