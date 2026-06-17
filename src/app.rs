use crate::config::AppConfig;
use crate::document::{read_preview, DocumentLoader};
use crate::indexer::{IncrementalReport, KnowledgeIndex, RebuildReport};
use crate::models::{Bookmark, IndexStats, SearchHistoryItem, SearchResult};
use crate::storage::LocalStore;
use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Search,
    Preview,
    Help,
    History,
    Bookmarks,
    Stats,
}

pub struct AppState {
    pub input: String,
    pub message: String,
    pub command_mode: bool,
    pub results: Vec<SearchResult>,
    pub total_matches: usize,
    pub selected: usize,
    pub bookmark_selected: usize,
    pub mode: AppMode,
    pub preview_text: String,
    pub preview_scroll: usize,
    pub history: Vec<SearchHistoryItem>,
    pub bookmarks: Vec<Bookmark>,
    pub stats: IndexStats,
    index: KnowledgeIndex,
    store: LocalStore,
    config: AppConfig,
}

impl AppState {
    pub fn new(index: KnowledgeIndex, store: LocalStore, config: AppConfig) -> Result<Self> {
        let history = store.load_history()?;
        let bookmarks = store.load_bookmarks()?;
        let stats = index.stats_from_manifest(&store).unwrap_or_default();
        Ok(Self {
            input: String::new(),
            message: "Type keyword and press Enter. Ctrl+O opens commands.".into(),
            command_mode: false,
            results: Vec::new(),
            total_matches: 0,
            selected: 0,
            bookmark_selected: 0,
            mode: AppMode::Search,
            preview_text: String::new(),
            preview_scroll: 0,
            history,
            bookmarks,
            stats,
            index,
            store,
            config,
        })
    }

    pub fn search(&mut self) -> Result<()> {
        let query = self.input.trim().to_string();
        if query.is_empty() {
            self.message = "Query is empty.".into();
            return Ok(());
        }
        let (total, results) = self
            .index
            .search_with_count(&query, self.config.default_limit)?;
        self.total_matches = total;
        self.results = results;
        self.selected = 0;
        self.store.push_history(query.clone(), total)?;
        self.history = self.store.load_history()?;
        self.message = format!("{} matches, showing top {}.", total, self.results.len());
        self.mode = AppMode::Search;
        Ok(())
    }

    pub fn rebuild_index(&mut self) -> Result<RebuildReport> {
        let loader = DocumentLoader::new(self.config.clone());
        let report = self
            .index
            .rebuild_from_dir(&self.config.docs_dir, &loader, &self.store)?;
        self.stats = self
            .index
            .stats_from_manifest(&self.store)
            .unwrap_or_default();
        self.message = format!("Rebuilt index: {} documents.", report.indexed_documents);
        Ok(report)
    }

    pub fn incremental_update(&mut self) -> Result<IncrementalReport> {
        let loader = DocumentLoader::new(self.config.clone());
        let report = self
            .index
            .incremental_update(&self.config.docs_dir, &loader, &self.store)?;
        self.stats = self
            .index
            .stats_from_manifest(&self.store)
            .unwrap_or_default();
        self.message = format!(
            "Updated index: {} changed, {} unchanged.",
            report.changed, report.unchanged
        );
        Ok(report)
    }

    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.results.get(self.selected)
    }

    pub fn selected_path(&self) -> Option<PathBuf> {
        self.selected_result().map(|r| r.path.clone())
    }

    pub fn next(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1) % self.results.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.results.is_empty() {
            self.selected = if self.selected == 0 {
                self.results.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn open_preview(&mut self) -> Result<()> {
        if let Some(path) = self.selected_path() {
            self.preview_text = read_preview(&path, 3000)?;
            self.preview_scroll = 0;
            self.mode = AppMode::Preview;
            self.message = format!("Preview: {}", path.display());
        } else {
            self.message = "No selected result.".into();
        }
        Ok(())
    }

    pub fn add_selected_bookmark(&mut self) -> Result<()> {
        if let Some(result) = self.selected_result() {
            let added = self
                .store
                .add_bookmark(result.title.clone(), result.path.clone())?;
            self.bookmarks = self.store.load_bookmarks()?;
            if !self.bookmarks.is_empty() {
                self.bookmark_selected = self.bookmarks.len() - 1;
            }
            self.message = if added {
                "Bookmark added.".into()
            } else {
                "Bookmark already exists.".into()
            };
        } else {
            self.message = "No selected result.".into();
        }
        Ok(())
    }

    fn remove_bookmark(&mut self, path: &Path) -> Result<()> {
        let removed = self.store.remove_bookmark(path)?;
        self.bookmarks = self.store.load_bookmarks()?;
        if self.bookmark_selected >= self.bookmarks.len() {
            self.bookmark_selected = self.bookmarks.len().saturating_sub(1);
        }
        self.message = if removed {
            "Bookmark removed.".into()
        } else {
            "Bookmark not found.".into()
        };
        Ok(())
    }

    pub fn selected_bookmark(&self) -> Option<&Bookmark> {
        self.bookmarks.get(self.bookmark_selected)
    }

    pub fn next_bookmark(&mut self) {
        if !self.bookmarks.is_empty() {
            self.bookmark_selected = (self.bookmark_selected + 1) % self.bookmarks.len();
        }
    }

    pub fn previous_bookmark(&mut self) {
        if !self.bookmarks.is_empty() {
            self.bookmark_selected = if self.bookmark_selected == 0 {
                self.bookmarks.len() - 1
            } else {
                self.bookmark_selected - 1
            };
        }
    }

    pub fn open_selected_bookmark(&mut self) -> Result<()> {
        if let Some(bookmark) = self.selected_bookmark() {
            let path = bookmark.path.clone();
            self.preview_text = read_preview(&path, 3000)?;
            self.preview_scroll = 0;
            self.mode = AppMode::Preview;
            self.message = format!("Preview bookmark: {}", path.display());
        } else {
            self.message = "No selected bookmark.".into();
        }
        Ok(())
    }

    pub fn remove_selected_bookmark(&mut self) -> Result<()> {
        if let Some(bookmark) = self.selected_bookmark() {
            let path = bookmark.path.clone();
            self.remove_bookmark(&path)?;
        } else {
            self.message = "No selected bookmark.".into();
        }
        Ok(())
    }

    pub fn scroll_preview_down(&mut self, lines: usize) {
        self.preview_scroll = self.preview_scroll.saturating_add(lines);
    }

    pub fn scroll_preview_up(&mut self, lines: usize) {
        self.preview_scroll = self.preview_scroll.saturating_sub(lines);
    }

    pub fn clear_history(&mut self) -> Result<()> {
        self.store.clear_history()?;
        self.history = Vec::new();
        self.message = "History cleared.".into();
        Ok(())
    }

    pub fn switch_mode(&mut self, mode: AppMode) {
        self.command_mode = false;
        self.mode = mode;
    }

    pub fn enter_command_mode(&mut self) {
        self.command_mode = true;
        self.message = "Command: p preview, b bookmark, h history, m bookmarks, s stats, u update, ? help, q quit.".into();
    }

    pub fn cancel_command_mode(&mut self) {
        self.command_mode = false;
        self.message = "Command cancelled.".into();
    }
}
