use thiserror::Error;

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error("unsupported file type: {0}")]
    UnsupportedFileType(String),

    #[error("document is too large: {path}, size={size} bytes, limit={limit} bytes")]
    DocumentTooLarge { path: String, size: u64, limit: u64 },

    #[error("document not found: {0}")]
    DocumentNotFound(String),

    #[error("empty search query")]
    EmptySearchQuery,

    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
}
