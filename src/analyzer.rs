use crate::models::{IndexManifest, SearchHistoryItem, SearchResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryAnalysis {
    pub original: String,
    pub terms: Vec<String>,
    pub phrase_count: usize,
    pub has_field_filter: bool,
    pub estimated_complexity: QueryComplexity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueryComplexity {
    Empty,
    Simple,
    Medium,
    Complex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistorySummary {
    pub total_queries: usize,
    pub unique_queries: usize,
    pub most_frequent: Vec<(String, usize)>,
    pub zero_result_queries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResultQualitySummary {
    pub result_count: usize,
    pub max_score: f32,
    pub min_score: f32,
    pub average_score: f32,
    pub extensions: Vec<(String, usize)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorpusSummary {
    pub documents: usize,
    pub total_bytes: u64,
    pub largest_document: Option<String>,
    pub extension_distribution: Vec<(String, usize)>,
}

pub fn analyze_query(query: &str) -> QueryAnalysis {
    let original = query.to_string();
    let terms = tokenize_query(query);
    let phrase_count = count_phrases(query);
    let has_field_filter = query.contains(':');
    let estimated_complexity = estimate_complexity(&terms, phrase_count, has_field_filter);
    QueryAnalysis {
        original,
        terms,
        phrase_count,
        has_field_filter,
        estimated_complexity,
    }
}

pub fn tokenize_query(query: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;
    for ch in query.chars() {
        match ch {
            '"' => {
                in_quote = !in_quote;
                if !in_quote && !current.trim().is_empty() {
                    terms.push(current.trim().to_lowercase());
                    current.clear();
                }
            }
            c if c.is_whitespace() && !in_quote => {
                if !current.trim().is_empty() {
                    terms.push(current.trim().to_lowercase());
                    current.clear();
                }
            }
            c => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        terms.push(current.trim().to_lowercase());
    }
    terms
}

pub fn count_phrases(query: &str) -> usize {
    let mut count = 0;
    let mut in_quote = false;
    let mut has_content = false;
    for ch in query.chars() {
        if ch == '"' {
            if in_quote && has_content {
                count += 1;
            }
            in_quote = !in_quote;
            has_content = false;
        } else if in_quote && !ch.is_whitespace() {
            has_content = true;
        }
    }
    count
}

pub fn estimate_complexity(
    terms: &[String],
    phrase_count: usize,
    has_field_filter: bool,
) -> QueryComplexity {
    if terms.is_empty() {
        QueryComplexity::Empty
    } else if terms.len() <= 2 && phrase_count == 0 && !has_field_filter {
        QueryComplexity::Simple
    } else if terms.len() <= 5 && phrase_count <= 1 {
        QueryComplexity::Medium
    } else {
        QueryComplexity::Complex
    }
}

pub fn summarize_history(history: &[SearchHistoryItem]) -> HistorySummary {
    let mut freq: BTreeMap<String, usize> = BTreeMap::new();
    let mut zero_result_queries = 0;
    for item in history {
        *freq.entry(item.query.clone()).or_insert(0) += 1;
        if item.result_count == 0 {
            zero_result_queries += 1;
        }
    }
    let unique_queries = freq.len();
    let mut most_frequent: Vec<(String, usize)> = freq.into_iter().collect();
    most_frequent.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    most_frequent.truncate(10);
    HistorySummary {
        total_queries: history.len(),
        unique_queries,
        most_frequent,
        zero_result_queries,
    }
}

pub fn summarize_results(results: &[SearchResult]) -> ResultQualitySummary {
    if results.is_empty() {
        return ResultQualitySummary {
            result_count: 0,
            max_score: 0.0,
            min_score: 0.0,
            average_score: 0.0,
            extensions: Vec::new(),
        };
    }
    let mut max_score = f32::MIN;
    let mut min_score = f32::MAX;
    let mut sum = 0.0;
    let mut ext_count: BTreeMap<String, usize> = BTreeMap::new();
    for result in results {
        max_score = max_score.max(result.score);
        min_score = min_score.min(result.score);
        sum += result.score;
        *ext_count.entry(result.extension.clone()).or_insert(0) += 1;
    }
    let mut extensions: Vec<(String, usize)> = ext_count.into_iter().collect();
    extensions.sort_by(|a, b| b.1.cmp(&a.1));
    ResultQualitySummary {
        result_count: results.len(),
        max_score,
        min_score,
        average_score: sum / results.len() as f32,
        extensions,
    }
}

pub fn summarize_corpus(manifest: &IndexManifest) -> CorpusSummary {
    let mut total_bytes = 0;
    let mut largest_document: Option<(String, u64)> = None;
    let mut ext_count: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &manifest.entries {
        total_bytes += entry.bytes;
        *ext_count.entry(entry.extension.clone()).or_insert(0) += 1;
        match largest_document {
            Some((_, size)) if size >= entry.bytes => {}
            _ => largest_document = Some((entry.path.display().to_string(), entry.bytes)),
        }
    }
    let mut extension_distribution: Vec<(String, usize)> = ext_count.into_iter().collect();
    extension_distribution.sort_by(|a, b| b.1.cmp(&a.1));
    CorpusSummary {
        documents: manifest.entries.len(),
        total_bytes,
        largest_document: largest_document.map(|(path, _)| path),
        extension_distribution,
    }
}

pub fn recommend_queries(history: &[SearchHistoryItem], manifest: &IndexManifest) -> Vec<String> {
    let mut suggestions = BTreeSet::new();
    for item in history.iter().rev().take(20) {
        for term in tokenize_query(&item.query) {
            if term.len() >= 3 {
                suggestions.insert(term);
            }
        }
    }
    for entry in manifest.entries.iter().take(20) {
        if !entry.extension.is_empty() {
            suggestions.insert(format!("extension:{}", entry.extension));
        }
        for part in entry.title.split_whitespace() {
            let clean = part
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if clean.len() >= 4 {
                suggestions.insert(clean);
            }
        }
    }
    suggestions.into_iter().take(12).collect()
}

pub fn explain_query(query: &str) -> Vec<String> {
    let analysis = analyze_query(query);
    let mut lines = Vec::new();
    lines.push(format!("Original query: {}", analysis.original));
    lines.push(format!("Terms: {}", analysis.terms.join(", ")));
    lines.push(format!("Phrase count: {}", analysis.phrase_count));
    lines.push(format!("Field filter: {}", analysis.has_field_filter));
    lines.push(format!(
        "Estimated complexity: {:?}",
        analysis.estimated_complexity
    ));
    if analysis.terms.is_empty() {
        lines.push("Suggestion: enter at least one keyword.".into());
    } else if analysis.terms.len() == 1 {
        lines.push("Suggestion: add another keyword to narrow down results.".into());
    } else {
        lines.push("Suggestion: use quoted phrases for exact matching when needed.".into());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_tokenizer_keeps_phrase() {
        let terms = tokenize_query("Rust \"memory safety\" title:index");
        assert_eq!(terms, vec!["rust", "memory safety", "title:index"]);
    }

    #[test]
    fn query_analysis_detects_filter() {
        let analysis = analyze_query("title:Rust ownership");
        assert!(analysis.has_field_filter);
        assert_eq!(analysis.terms.len(), 2);
    }
}
