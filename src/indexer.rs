// ...existing code...
use crate::config::AppConfig;
use crate::document::DocumentLoader;
use crate::highlighter::{count_query_matches, make_snippet};
use crate::models::{
    ExtensionStat, IndexManifest, IndexManifestEntry, IndexStats, KnowledgeDocument, SearchResult,
};
use crate::storage::LocalStore;
use crate::tokenizer::join_tokens;
use anyhow::{Context, Result};
use chrono::Utc;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tantivy::collector::{Count, TopDocs};
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, TEXT};
use tantivy::{doc, Index, IndexWriter, TantivyDocument, Term};

#[derive(Clone)]
pub struct KnowledgeIndex {
    index: Index,
    title: Field,
    path: Field,
    extension: Field,
    content: Field,
    content_tokenized: Field,
    bytes: Field,
    modified: Field,
    snippet_chars: usize,
}

#[derive(Debug, Clone)]
pub struct RebuildReport {
    pub indexed_documents: usize,
    pub total_bytes: u64,
    pub skipped_documents: usize,
}

#[derive(Debug, Clone)]
pub struct IncrementalReport {
    pub discovered: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub total_bytes: u64,
}

impl KnowledgeIndex {
    pub fn open_or_create(index_dir: &Path, config: &AppConfig) -> Result<Self> {
        if index_dir.exists() {
            let index = Index::open_in_dir(index_dir)
                .with_context(|| format!("failed to open index: {}", index_dir.display()))?;
            Self::from_index(index, config.snippet_chars)
        } else {
            std::fs::create_dir_all(index_dir)?;
            let schema = build_schema();
            let index = Index::create_in_dir(index_dir, schema)?;
            Self::from_index(index, config.snippet_chars)
        }
    }

    fn from_index(index: Index, snippet_chars: usize) -> Result<Self> {
        let schema = index.schema();
        let title = schema.get_field("title")?;
        let path = schema.get_field("path")?;
        let extension = schema.get_field("extension")?;
        let content = schema.get_field("content")?;
        let content_tokenized = schema.get_field("content_tokenized")?;
        let bytes = schema.get_field("bytes")?;
        let modified = schema.get_field("modified")?;
        Ok(Self {
            index,
            title,
            path,
            extension,
            content,
            content_tokenized,
            bytes,
            modified,
            snippet_chars,
        })
    }

    pub fn rebuild_from_dir(
        &self,
        docs_dir: &Path,
        loader: &DocumentLoader,
        store: &LocalStore,
    ) -> Result<RebuildReport> {
        let documents = loader.load_dir(docs_dir)?;
        let total_bytes = documents.iter().map(|d| d.bytes).sum();
        self.clear_and_add(&documents)?;
        let manifest = manifest_from_documents(&documents);
        store.save_manifest(&manifest)?;
        Ok(RebuildReport {
            indexed_documents: documents.len(),
            total_bytes,
            skipped_documents: 0,
        })
    }

    pub fn incremental_update(
        &self,
        docs_dir: &Path,
        loader: &DocumentLoader,
        store: &LocalStore,
    ) -> Result<IncrementalReport> {
        let docs = loader.load_dir(docs_dir)?;
        let old_manifest = store.load_manifest()?;
        let mut changed = Vec::new();
        let mut unchanged = 0;
        for doc in docs.iter() {
            if old_manifest.is_unchanged(doc) {
                unchanged += 1;
            } else {
                changed.push(doc.clone());
            }
        }
        if !changed.is_empty() {
            let mut writer = self.index.writer(50_000_000)?;
            for doc in &changed {
                let term = Term::from_field_text(self.path, &doc.path.to_string_lossy());
                writer.delete_term(term);
                self.add_document_with_writer(&mut writer, doc)?;
            }
            writer.commit()?;
        }
        let manifest = manifest_from_documents(&docs);
        store.save_manifest(&manifest)?;
        let total_bytes = docs.iter().map(|d| d.bytes).sum();
        Ok(IncrementalReport {
            discovered: docs.len(),
            changed: changed.len(),
            unchanged,
            total_bytes,
        })
    }

    pub fn clear_and_add(&self, documents: &[KnowledgeDocument]) -> Result<()> {
        let mut writer = self.index.writer(50_000_000)?;
        writer.delete_all_documents()?;
        for doc in documents {
            self.add_document_with_writer(&mut writer, doc)?;
        }
        writer.commit()?;
        Ok(())
    }

    fn add_document_with_writer(
        &self,
        writer: &mut IndexWriter,
        d: &KnowledgeDocument,
    ) -> Result<()> {
        // 将原始 content 存为 content 字段，同时把分词后的 tokenized 内容放入 content_tokenized 字段用于检索
        let tokenized = join_tokens(&d.content);
        writer.add_document(doc!(
            self.title => d.title.clone(),
            self.path => d.path.to_string_lossy().to_string(),
            self.extension => d.extension.clone(),
            self.content => d.content.clone(), // 存储原始文本以便 snippet/显示
            self.content_tokenized => tokenized,
            self.bytes => d.bytes as i64,
            self.modified => d.modified.map(|m| m.to_rfc3339()).unwrap_or_default()
        ))?;
        Ok(())
    }

    pub fn search(&self, query_text: &str, limit: usize) -> Result<Vec<SearchResult>> {
        if query_text.trim().is_empty() {
            return Ok(Vec::new());
        }
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let parser = QueryParser::for_index(
            &self.index,
            vec![self.title, self.content_tokenized, self.extension],
        );
        // 对用户 query 做分词并 join，以便与索引时使用的分词一致
        let tokenized_query = join_tokens(query_text);
        let query = parser.parse_query(&tokenized_query)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;
        let mut results = Vec::new();
        for (score, address) in top_docs {
            let doc: TantivyDocument = searcher.doc(address)?;
            results.push(self.result_from_document(doc, score, query_text));
        }
        Ok(results)
    }

    pub fn count_matches(&self, query_text: &str) -> Result<usize> {
        if query_text.trim().is_empty() {
            return Ok(0);
        }
        let reader = self.index.reader()?;
        let searcher = reader.searcher();
        let parser = QueryParser::for_index(
            &self.index,
            vec![self.title, self.content_tokenized, self.extension],
        );
        let tokenized_query = join_tokens(query_text);
        let query = parser.parse_query(&tokenized_query)?;
        let count = searcher.search(&query, &Count)?;
        Ok(count)
    }

    pub fn search_with_count(
        &self,
        query_text: &str,
        limit: usize,
    ) -> Result<(usize, Vec<SearchResult>)> {
        let total = self.count_matches(query_text)?;
        let results = self.search(query_text, limit)?;
        Ok((total, results))
    }

    pub fn stats_from_manifest(&self, store: &LocalStore) -> Result<IndexStats> {
        let manifest = store.load_manifest()?;
        Ok(stats_from_manifest(&manifest))
    }

    fn result_from_document(
        &self,
        doc: TantivyDocument,
        score: f32,
        query_text: &str,
    ) -> SearchResult {
        let title = first_text(&doc, self.title).unwrap_or_else(|| "untitled".to_string());
        let path = first_text(&doc, self.path).unwrap_or_default();
        let extension = first_text(&doc, self.extension).unwrap_or_default();
        // content field still holds original stored text for snippet/highlight
        let content = first_text(&doc, self.content).unwrap_or_default();
        let bytes = first_i64(&doc, self.bytes).unwrap_or(0).max(0) as u64;
        let modified_text = first_text(&doc, self.modified).unwrap_or_default();
        let modified = chrono::DateTime::parse_from_rfc3339(&modified_text)
            .ok()
            .map(|dt| dt.with_timezone(&Utc));
        let match_count = count_query_matches(&format!("{} {}", title, content), query_text);
        SearchResult {
            title,
            path: PathBuf::from(path),
            score,
            snippet: make_snippet(&content, query_text, self.snippet_chars),
            extension,
            bytes,
            modified,
            match_count,
        }
    }
}

fn build_schema() -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field("title", TEXT | STORED);
    schema_builder.add_text_field("path", TEXT | STORED);
    schema_builder.add_text_field("extension", TEXT | STORED);
    // 原始 content 用于存储/显示
    schema_builder.add_text_field("content", TEXT | STORED);
    // 用于索引的分词后内容（不必 STORED）
    schema_builder.add_text_field("content_tokenized", TEXT);
    schema_builder.add_i64_field("bytes", STORED);
    schema_builder.add_text_field("modified", STORED);
    schema_builder.build()
}

fn first_text(doc: &TantivyDocument, field: Field) -> Option<String> {
    doc.get_first(field)
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
}

fn first_i64(doc: &TantivyDocument, field: Field) -> Option<i64> {
    doc.get_first(field).and_then(|v| v.as_i64())
}

fn manifest_from_documents(documents: &[KnowledgeDocument]) -> IndexManifest {
    let entries = documents
        .iter()
        .map(|doc| IndexManifestEntry {
            path: doc.path.clone(),
            title: doc.title.clone(),
            extension: doc.extension.clone(),
            bytes: doc.bytes,
            modified: doc.modified,
            indexed_at: Utc::now(),
        })
        .collect();
    IndexManifest { entries }
}

fn stats_from_manifest(manifest: &IndexManifest) -> IndexStats {
    let mut map: BTreeMap<String, ExtensionStat> = BTreeMap::new();
    let mut total_bytes = 0;
    for entry in &manifest.entries {
        total_bytes += entry.bytes;
        let stat = map.entry(entry.extension.clone()).or_insert(ExtensionStat {
            extension: entry.extension.clone(),
            count: 0,
            bytes: 0,
        });
        stat.count += 1;
        stat.bytes += entry.bytes;
    }
    IndexStats {
        documents: manifest.entries.len(),
        total_bytes,
        total_words: 0,
        extensions: map.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::document::DocumentLoader;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn index_should_count_matches() -> Result<()> {
        let dir = tempdir()?;
        let docs = dir.path().join("docs");
        let index_dir = dir.path().join("idx");
        let data_dir = dir.path().join("data");
        fs::create_dir_all(&docs)?;
        fs::write(docs.join("a.md"), "# Rust\nRust ownership and borrowing")?;
        fs::write(docs.join("b.md"), "# Search\nTantivy search in Rust")?;
        let cfg = AppConfig {
            docs_dir: docs.clone(),
            index_dir: index_dir.clone(),
            data_dir: data_dir.clone(),
            ..AppConfig::default()
        };
        let loader = DocumentLoader::new(cfg.clone());
        let store = LocalStore::new(data_dir)?;
        let index = KnowledgeIndex::open_or_create(&index_dir, &cfg)?;
        index.rebuild_from_dir(&docs, &loader, &store)?;
        let (count, results) = index.search_with_count("Rust", 10)?;
        assert_eq!(count, 2);
        assert_eq!(results.len(), 2);
        Ok(())
    }

    #[test]
    fn chinese_search_should_find() -> Result<()> {
        let dir = tempdir()?;
        let docs = dir.path().join("docs");
        let index_dir = dir.path().join("idx");
        let data_dir = dir.path().join("data");
        fs::create_dir_all(&docs)?;
        fs::write(docs.join("c.md"), "我爱自然语言处理和搜索")?;
        let cfg = AppConfig {
            docs_dir: docs.clone(),
            index_dir: index_dir.clone(),
            data_dir: data_dir.clone(),
            ..AppConfig::default()
        };
        let loader = DocumentLoader::new(cfg.clone());
        let store = LocalStore::new(data_dir)?;
        let index = KnowledgeIndex::open_or_create(&index_dir, &cfg)?;
        index.rebuild_from_dir(&docs, &loader, &store)?;
        let (count, results) = index.search_with_count("自然语言", 10)?;
        assert!(count >= 1, "expected at least one match for Chinese query");
        assert!(!results.is_empty());
        Ok(())
    }
}
