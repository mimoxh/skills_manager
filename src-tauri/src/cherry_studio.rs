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
            user_tags: Vec::new(),
        })
    }

    /// Open the Cherry Studio database.
    pub fn open_db(&self) -> AppResult<CherryDb> {
        CherryDb::open(&self.db_path())
    }

    /// Install a skill into Cherry Studio:
    /// 1. Copy files to Data\Skills\{folder_name}\
    /// 2. Insert/update skill + link to all agents in a single DB transaction
    pub fn install_skill(
        &self,
        source_path: &Path,
        folder_name: &str,
    ) -> AppResult<String> {
        let target = self.skills_dir().join(folder_name);
        fs::create_dir_all(self.skills_dir())?;
        copy_dir_all(source_path, &target)?;

        let (name, description, _version) = read_skill_md_meta(&target);
        let content_hash = crate::hash::hash_dir(&target)?;

        let db = self.open_db()?;
        let agents = db.list_agents()?;
        let agent_ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
        // 单连接 + 单事务：插入/更新 skill 并与 agents 关联原子完成，避免半安装状态
        db.upsert_skill_and_link_agents(
            name.as_deref().unwrap_or(folder_name),
            description.as_deref(),
            folder_name,
            &content_hash,
            &agent_ids,
        )
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
        if let Some(fm) = crate::manifest::parse_skill_frontmatter(&text) {
            return (fm.name, fm.description, fm.version);
        }
    }
    (None, None, None)
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
        let fm = crate::manifest::parse_skill_frontmatter(text).unwrap();
        assert_eq!(fm.name.unwrap(), "aihot");
        assert_eq!(fm.version.unwrap(), "1.0.0");
        assert_eq!(fm.description.unwrap(), "AI HOT skill.\n查询 AI 资讯。");
    }

    #[test]
    fn parses_frontmatter_with_inline_description() {
        let text = r#"---
name: humanizer
version: 2.1.1
description: 去除 AI 写作痕迹。
---
# Body"#;
        let fm = crate::manifest::parse_skill_frontmatter(text).unwrap();
        assert_eq!(fm.name.unwrap(), "humanizer");
        assert_eq!(fm.version.unwrap(), "2.1.1");
        assert_eq!(fm.description.unwrap(), "去除 AI 写作痕迹。");
    }

    #[test]
    fn returns_none_for_no_frontmatter() {
        let text = "# Just a heading\nSome content";
        assert!(crate::manifest::parse_skill_frontmatter(text).is_none());
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
