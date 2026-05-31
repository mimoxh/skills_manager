use crate::{
    cherry_db::CherryDb,
    error::AppResult,
    hash::copy_dir_all,
    models::AgentProfile,
};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Parsed SKILL.md frontmatter metadata.
#[derive(Debug, Clone, Default)]
pub struct SkillMdMeta {
    pub name: Option<String>,
    pub description: Option<String>,
    pub version: Option<String>,
}

/// Parsed meta.json metadata (Cherry Studio marketplace format).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CherryMetaJson {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
}

pub struct CherryStudioAdapter {
    data_dir: PathBuf,
}

impl CherryStudioAdapter {
    pub fn new() -> Option<Self> {
        let data_dir = env::var_os("APPDATA")
            .map(PathBuf::from)
            .map(|p| p.join("CherryStudio").join("Data"))?;
        if data_dir.exists() {
            Some(Self { data_dir })
        } else {
            None
        }
    }

    pub fn skills_dir(&self) -> PathBuf {
        self.data_dir.join("Skills")
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("agents.db")
    }

    /// Detect Cherry Studio installation and return a single AgentProfile.
    pub fn detect(&self) -> Option<AgentProfile> {
        let skills_dir = self.skills_dir();
        if !skills_dir.exists() {
            return None;
        }
        Some(AgentProfile {
            id: format!("cherryStudio:{}", skills_dir.to_string_lossy()),
            name: "Cherry Studio".to_string(),
            agent_type: crate::models::AgentType::CherryStudio,
            skills_path: skills_dir.to_string_lossy().to_string(),
            adapter_config: None,
        })
    }

    /// Open the Cherry Studio database.
    pub fn open_db(&self) -> AppResult<CherryDb> {
        CherryDb::open(&self.db_path())
    }

    /// Install a skill into Cherry Studio:
    /// 1. Copy files to Data\Skills\{folder_name}\
    /// 2. Insert into skills table
    /// 3. Link to all Cherry Studio agents
    pub fn install_skill(
        &self,
        source_path: &Path,
        folder_name: &str,
    ) -> AppResult<String> {
        let target = self.skills_dir().join(folder_name);
        fs::create_dir_all(self.skills_dir())?;
        copy_dir_all(source_path, &target)?;

        let (name, description, _version) = read_skill_md_meta(&target);
        let content_hash = crate::hash::hash_dir(&target).unwrap_or_default();

        let db = self.open_db()?;
        let skill_id = match db.get_skill(folder_name)? {
            Some(existing) => {
                // Update existing record
                db.update_skill(
                    folder_name,
                    name.as_deref().unwrap_or(folder_name),
                    description.as_deref(),
                    &content_hash,
                )?;
                existing.id
            }
            None => {
                db.insert_skill(
                    name.as_deref().unwrap_or(folder_name),
                    description.as_deref(),
                    folder_name,
                    &content_hash,
                )?
            }
        };

        // Link to all Cherry Studio agents
        let agents = db.list_agents()?;
        let agent_ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
        if !agent_ids.is_empty() {
            db.enable_skill_for_agents(&skill_id, &agent_ids)?;
        }

        Ok(skill_id)
    }

    /// Uninstall a skill from Cherry Studio:
    /// 1. Delete from agents.db (CASCADE cleans agent_skills)
    /// 2. Delete directory
    pub fn uninstall_skill(&self, folder_name: &str) -> AppResult<()> {
        let db = self.open_db()?;
        db.delete_skill(folder_name)?;

        let target = self.skills_dir().join(folder_name);
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        Ok(())
    }
}

/// Read SKILL.md frontmatter to extract name, description, version.
pub fn read_skill_md_meta(skill_dir: &Path) -> (Option<String>, Option<String>, Option<String>) {
    let skill_md = skill_dir.join("SKILL.md");
    if let Ok(text) = fs::read_to_string(&skill_md) {
        if let Some((name, version, description)) = parse_frontmatter(&text) {
            return (name, description, version);
        }
    }
    (None, None, None)
}

/// Parse YAML frontmatter from SKILL.md content.
fn parse_frontmatter(text: &str) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let trimmed = text.trim();
    if !trimmed.starts_with("---") {
        return None;
    }
    let after_first = &trimmed[3..];
    let end_idx = after_first.find("\n---")?;
    let frontmatter = &after_first[..end_idx];

    let mut name = None;
    let mut version = None;
    let mut description = None;
    let mut collecting_block: Option<String> = None;
    let mut block_lines: Vec<String> = Vec::new();

    for line in frontmatter.lines() {
        let trimmed_line = line.trim();

        // Handle YAML block scalars
        if let Some(ref key) = collecting_block {
            if line.starts_with(' ') || line.starts_with('\t') {
                block_lines.push(trimmed_line.to_string());
                continue;
            } else {
                let block_value = block_lines.join("\n");
                if !block_value.is_empty() {
                    match key.as_str() {
                        "name" | "title" => name = Some(block_value),
                        "version" => version = Some(block_value),
                        "description" => description = Some(block_value),
                        _ => {}
                    }
                }
                collecting_block = None;
                block_lines.clear();
            }
        }

        let Some((key, value)) = trimmed_line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();

        // Detect block scalar indicators
        if matches!(value, "|" | ">" | "|-" | ">-" | "|+" | ">+") {
            collecting_block = Some(key.to_string());
            block_lines.clear();
            continue;
        }

        let value = value.trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            continue;
        }

        match key {
            "name" | "title" => name = Some(value.to_string()),
            "version" => version = Some(value.to_string()),
            "description" => description = Some(value.to_string()),
            _ => {}
        }
    }

    // Handle block scalar at end of frontmatter
    if let Some(ref key) = collecting_block {
        let block_value = block_lines.join("\n");
        if !block_value.is_empty() {
            match key.as_str() {
                "name" | "title" => name = Some(block_value),
                "version" => version = Some(block_value),
                "description" => description = Some(block_value),
                _ => {}
            }
        }
    }

    Some((name, version, description))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_with_pipe_description() {
        let text = r#"---
name: aihot
version: 1.0.0
description: |
  AI HOT skill.
  查询 AI 资讯。
---
# Body"#;
        let (name, version, desc) = parse_frontmatter(text).unwrap();
        assert_eq!(name.unwrap(), "aihot");
        assert_eq!(version.unwrap(), "1.0.0");
        assert_eq!(desc.unwrap(), "AI HOT skill.\n查询 AI 资讯。");
    }

    #[test]
    fn parses_frontmatter_with_inline_description() {
        let text = r#"---
name: humanizer
version: 2.1.1
description: 去除 AI 写作痕迹。
---
# Body"#;
        let (name, version, desc) = parse_frontmatter(text).unwrap();
        assert_eq!(name.unwrap(), "humanizer");
        assert_eq!(version.unwrap(), "2.1.1");
        assert_eq!(desc.unwrap(), "去除 AI 写作痕迹。");
    }

    #[test]
    fn returns_none_for_no_frontmatter() {
        let text = "# Just a heading\nSome content";
        assert!(parse_frontmatter(text).is_none());
    }

    #[test]
    fn read_skill_md_meta_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Test\nversion: 1.0.0\ndescription: A test.\n---\n# Body",
        )
        .unwrap();

        let (name, desc, version) = read_skill_md_meta(&skill_dir);
        assert_eq!(name.unwrap(), "Test");
        assert_eq!(version.unwrap(), "1.0.0");
        assert_eq!(desc.unwrap(), "A test.");
    }
}
