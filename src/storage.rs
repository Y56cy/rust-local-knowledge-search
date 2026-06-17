use crate::models::{Bookmark, IndexManifest, SearchHistoryItem};
use anyhow::{Context, Result};
use chrono::Utc;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalStore {
    root: PathBuf,
}

impl LocalStore {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn history_path(&self) -> PathBuf {
        self.root.join("history.json")
    }

    pub fn bookmarks_path(&self) -> PathBuf {
        self.root.join("bookmarks.json")
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join("manifest.json")
    }

    pub fn load_history(&self) -> Result<Vec<SearchHistoryItem>> {
        read_json_or_default(&self.history_path())
    }

    pub fn push_history(&self, query: String, result_count: usize) -> Result<()> {
        let mut history = self.load_history()?;
        history.push(SearchHistoryItem {
            query,
            result_count,
            searched_at: Utc::now(),
        });
        if history.len() > 200 {
            let keep_from = history.len() - 200;
            history = history.split_off(keep_from);
        }
        write_json_pretty(&self.history_path(), &history)
    }

    pub fn clear_history(&self) -> Result<()> {
        write_json_pretty(&self.history_path(), &Vec::<SearchHistoryItem>::new())
    }

    pub fn load_bookmarks(&self) -> Result<Vec<Bookmark>> {
        read_json_or_default(&self.bookmarks_path())
    }

    pub fn add_bookmark(&self, title: String, path: PathBuf) -> Result<bool> {
        let mut bookmarks = self.load_bookmarks()?;
        if bookmarks.iter().any(|b| b.path == path) {
            return Ok(false);
        }
        bookmarks.push(Bookmark {
            title,
            path,
            added_at: Utc::now(),
        });
        write_json_pretty(&self.bookmarks_path(), &bookmarks)?;
        Ok(true)
    }

    pub fn remove_bookmark(&self, path: &Path) -> Result<bool> {
        let mut bookmarks = self.load_bookmarks()?;
        let before = bookmarks.len();
        bookmarks.retain(|b| b.path != path);
        write_json_pretty(&self.bookmarks_path(), &bookmarks)?;
        Ok(bookmarks.len() != before)
    }

    pub fn load_manifest(&self) -> Result<IndexManifest> {
        read_json_or_default(&self.manifest_path())
    }

    pub fn save_manifest(&self, manifest: &IndexManifest) -> Result<()> {
        write_json_pretty(&self.manifest_path(), manifest)
    }
}

pub fn read_json_or_default<T>(path: &Path) -> Result<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read json: {}", path.display()))?;
    let value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse json: {}", path.display()))?;
    Ok(value)
}

pub fn write_json_pretty<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(value)?;
    fs::write(path, text).with_context(|| format!("failed to write json: {}", path.display()))?;
    Ok(())
}
