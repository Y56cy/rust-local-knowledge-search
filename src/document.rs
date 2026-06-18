use crate::config::AppConfig;
use crate::errors::KnowledgeError;
use crate::models::KnowledgeDocument;
use crate::utils::{canonical_or_original, file_extension, file_modified_utc, safe_title};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct DocumentLoader {
    config: AppConfig,
}

pub struct DocumentLoadReport {
    pub documents: Vec<KnowledgeDocument>,
    pub skipped_documents: usize,
}

impl DocumentLoader {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn load_one(&self, path: &Path) -> Result<KnowledgeDocument> {
        let metadata = fs::metadata(path)
            .with_context(|| format!("failed to read metadata: {}", path.display()))?;
        let bytes = metadata.len();
        if bytes > self.config.max_file_bytes {
            return Err(KnowledgeError::DocumentTooLarge {
                path: path.display().to_string(),
                size: bytes,
                limit: self.config.max_file_bytes,
            }
            .into());
        }
        let extension = file_extension(path);
        if !self.config.supports_extension(&extension) {
            return Err(KnowledgeError::UnsupportedFileType(extension).into());
        }
        let content = read_document_content(path, &extension)
            .with_context(|| format!("failed to read document: {}", path.display()))?;
        let path = canonical_or_original(path);
        let title = extract_title(&content).unwrap_or_else(|| safe_title(&path));
        let id = path.to_string_lossy().to_string();
        let modified = file_modified_utc(&path);
        Ok(KnowledgeDocument {
            id,
            title,
            path,
            extension,
            content,
            bytes,
            modified,
        })
    }

    pub fn load_dir(&self, dir: &Path) -> Result<Vec<KnowledgeDocument>> {
        Ok(self.load_dir_report(dir)?.documents)
    }

    pub fn load_dir_report(&self, dir: &Path) -> Result<DocumentLoadReport> {
        if !dir.exists() {
            bail!("document directory does not exist: {}", dir.display());
        }
        if !dir.is_dir() {
            bail!("document path is not a directory: {}", dir.display());
        }

        let mut docs = Vec::new();
        let mut skipped_documents = 0;
        for entry in WalkDir::new(dir) {
            let entry = entry
                .with_context(|| format!("failed to walk document directory: {}", dir.display()))?;
            let path = entry.path();
            if path.is_file() && self.is_supported_path(path) {
                match self.load_one(path) {
                    Ok(doc) => docs.push(doc),
                    Err(err) => {
                        skipped_documents += 1;
                        eprintln!("skip {}: {}", path.display(), err);
                    }
                }
            }
        }
        docs.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(DocumentLoadReport {
            documents: docs,
            skipped_documents,
        })
    }

    pub fn is_supported_path(&self, path: &Path) -> bool {
        let ext = file_extension(path);
        !ext.is_empty() && self.config.supports_extension(&ext)
    }
}

pub fn extract_title(content: &str) -> Option<String> {
    for line in content.lines().take(10) {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

pub fn read_preview(path: &Path, max_chars: usize) -> Result<String> {
    let ext = file_extension(path);
    let text = read_document_content(path, &ext)
        .with_context(|| format!("failed to preview document: {}", path.display()))?;
    let preview: String = text.chars().take(max_chars).collect();
    Ok(preview)
}

pub fn collect_supported_paths(dir: &Path, config: &AppConfig) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .map(|e| e.into_path())
        .filter(|p| p.is_file())
        .filter(|p| config.supports_extension(&file_extension(p)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn extract_markdown_title() {
        assert_eq!(extract_title("# Hello\nbody"), Some("Hello".to_string()));
    }

    #[test]
    fn load_dir_should_error_when_directory_is_missing() {
        let dir = tempdir().expect("tempdir");
        let missing = dir.path().join("missing");
        let loader = DocumentLoader::new(AppConfig::default());
        let err = loader
            .load_dir(&missing)
            .expect_err("missing dir should fail");
        assert!(err.to_string().contains("does not exist"));
    }
}

/// 读取不同类型文档的内容
pub fn read_document_content(path: &Path, extension: &str) -> Result<String> {
    match extension.to_lowercase().as_str() {
        "pdf" => read_pdf_content(path),
        "docx" => read_docx_content(path),
        _ => fs::read_to_string(path)
            .with_context(|| format!("failed to read text file: {}", path.display())),
    }
}

/// PDF 解析
fn read_pdf_content(path: &Path) -> Result<String> {
    let text = pdf_extract::extract_text(path)
        .with_context(|| format!("failed to extract text from pdf: {}", path.display()))?;
    Ok(text)
}

/// DOCX 解析（兼容 Windows 路径和 UTF-8 编码）
fn read_docx_content(path: &Path) -> Result<String> {
    use quick_xml::events::Event;
    use quick_xml::Reader;
    use std::io::Read;
    use zip::ZipArchive;

    let file =
        fs::File::open(path).with_context(|| format!("failed to open docx: {}", path.display()))?;
    let mut archive = ZipArchive::new(file)
        .with_context(|| format!("failed to read docx zip: {}", path.display()))?;

    let mut document_bytes = Vec::new();
    archive
        .by_name("word/document.xml")
        .with_context(|| format!("word/document.xml not found in {}", path.display()))?
        .read_to_end(&mut document_bytes)
        .with_context(|| format!("failed to read document.xml bytes: {}", path.display()))?;

    let document_xml = String::from_utf8(document_bytes)
        .with_context(|| format!("failed to parse document.xml as UTF-8: {}", path.display()))?;

    let mut reader = Reader::from_str(&document_xml);
    reader.trim_text(true);

    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(e)) => {
                let content = e
                    .unescape()
                    .with_context(|| "failed to unescape docx text")?;
                text.push_str(&content);
                text.push(' ');
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("failed to parse docx XML: {}", e)),
            _ => {}
        }
    }
    Ok(text)
}
