use crate::errors::KnowledgeError;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub docs_dir: PathBuf,
    pub index_dir: PathBuf,
    pub data_dir: PathBuf,
    pub max_file_bytes: u64,
    pub supported_extensions: Vec<String>,
    pub default_limit: usize,
    pub snippet_chars: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            docs_dir: PathBuf::from("./docs"),
            index_dir: PathBuf::from("./knowledge_index"),
            data_dir: PathBuf::from("./knowledge_data"),
            max_file_bytes: 2 * 1024 * 1024,
            supported_extensions: vec![
                "md".into(),
                "txt".into(),
                "rs".into(),
                "toml".into(),
                "json".into(),
                "csv".into(),
                "log".into(),
                "yaml".into(),
                "yml".into(),
                "pdf".into(),
                "docx".into(),
            ],
            default_limit: 10,
            snippet_chars: 160,
        }
    }
}

impl AppConfig {
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if path.exists() {
            let text = fs::read_to_string(path)
                .with_context(|| format!("failed to read config: {}", path.display()))?;
            let cfg: AppConfig = serde_json::from_str(&text)
                .with_context(|| format!("failed to parse config: {}", path.display()))?;
            cfg.validate()?;
            Ok(cfg)
        } else {
            let cfg = AppConfig::default();
            cfg.save(path)?;
            Ok(cfg)
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text)
            .with_context(|| format!("failed to write config: {}", path.display()))?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.default_limit == 0 {
            return Err(
                KnowledgeError::InvalidConfig("default_limit must be positive".into()).into(),
            );
        }
        if self.snippet_chars < 20 {
            return Err(KnowledgeError::InvalidConfig(
                "snippet_chars should be at least 20".into(),
            )
            .into());
        }
        if self.supported_extensions.is_empty() {
            return Err(KnowledgeError::InvalidConfig(
                "supported_extensions cannot be empty".into(),
            )
            .into());
        }
        Ok(())
    }

    pub fn supports_extension(&self, ext: &str) -> bool {
        self.supported_extensions
            .iter()
            .any(|e| e.eq_ignore_ascii_case(ext))
    }
}

pub fn default_config_path() -> PathBuf {
    PathBuf::from("./knowledge_config.json")
}
