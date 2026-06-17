use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeDocument {
    pub id: String,
    pub title: String,
    pub path: PathBuf,
    pub extension: String,
    pub content: String,
    pub bytes: u64,
    pub modified: Option<DateTime<Utc>>,
}

impl KnowledgeDocument {
    pub fn preview(&self, max_chars: usize) -> String {
        let text = normalize_space(&self.content);
        truncate_chars(&text, max_chars)
    }

    pub fn line_count(&self) -> usize {
        self.content.lines().count()
    }

    pub fn word_count(&self) -> usize {
        self.content.split_whitespace().count()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SearchResult {
    pub title: String,
    pub path: PathBuf,
    pub score: f32,
    pub snippet: String,
    pub extension: String,
    pub bytes: u64,
    pub modified: Option<DateTime<Utc>>,
    pub match_count: usize,
}

impl SearchResult {
    pub fn render_summary(&self) -> String {
        format!(
            "[{:.2}] {} ({})",
            self.score,
            self.title,
            self.path.display()
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexStats {
    pub documents: usize,
    pub total_bytes: u64,
    pub total_words: usize,
    pub extensions: Vec<ExtensionStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExtensionStat {
    pub extension: String,
    pub count: usize,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHistoryItem {
    pub query: String,
    pub result_count: usize,
    pub searched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    pub path: PathBuf,
    pub added_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexManifestEntry {
    pub path: PathBuf,
    pub title: String,
    pub extension: String,
    pub bytes: u64,
    pub modified: Option<DateTime<Utc>>,
    pub indexed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexManifest {
    pub entries: Vec<IndexManifestEntry>,
}

impl IndexManifest {
    pub fn find_by_path(&self, path: &PathBuf) -> Option<&IndexManifestEntry> {
        self.entries.iter().find(|entry| &entry.path == path)
    }

    pub fn is_unchanged(&self, doc: &KnowledgeDocument) -> bool {
        self.find_by_path(&doc.path)
            .map(|entry| entry.bytes == doc.bytes && entry.modified == doc.modified)
            .unwrap_or(false)
    }
}

pub fn normalize_space(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut s: String = text.chars().take(max_chars).collect();
        s.push_str("...");
        s
    }
}
