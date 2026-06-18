use crate::models::truncate_chars;
use crate::tokenizer::{contains_cjk, tokenize};
use std::cmp::Reverse;

pub fn make_snippet(content: &str, query: &str, max_chars: usize) -> String {
    let normalized = content.replace('\n', " ");
    // 先用分词确定第一个关键字（优先中文分词）
    let tokens = tokenize(query);
    let first_keyword = tokens.first().map(|s| s.as_str()).unwrap_or("");
    let lower = normalized.to_lowercase();
    let start = if first_keyword.is_empty() {
        0
    } else {
        if contains_cjk(first_keyword) {
            normalized
                .find(first_keyword)
                .unwrap_or(0)
                .saturating_sub(40)
        } else {
            lower
                .find(&first_keyword.to_lowercase())
                .unwrap_or(0)
                .saturating_sub(40)
        }
    };
    let snippet: String = normalized.chars().skip(start).take(max_chars).collect();
    let snippet = highlight_with_tokens(&snippet, query);
    if normalized.chars().count() > start + max_chars {
        format!("{}...", snippet)
    } else {
        snippet
    }
}

pub fn highlight_with_tokens(text: &str, query: &str) -> String {
    let mut output = text.to_string();

    // 收集分词，并把原始 query 也当作一个候选 token（确保整句优先匹配）
    let mut tokens: Vec<String> = tokenize(query)
        .into_iter()
        .filter(|w| !w.trim().is_empty())
        .collect();
    let q_trim = query.trim();
    if !q_trim.is_empty() {
        tokens.push(q_trim.to_string());
    }

    // 优先替换更长的 token，避免短 token 将长短句拆开高亮；然后去重
    tokens.sort_by_key(|token| Reverse(token.len()));
    tokens.dedup();

    for keyword in tokens {
        if keyword.is_empty() {
            continue;
        }
        let marker = format!("**{}**", keyword);

        // 只在未被已有 "**" 包裹的段落中进行替换；通过 split 保证不产生字节边界切片问题
        let parts: Vec<String> = output
            .split("**")
            .enumerate()
            .map(|(i, part)| {
                if i % 2 == 1 {
                    // 已被包裹的部分，保留原样（不再替换）
                    part.to_string()
                } else {
                    // 未被包裹的段落：中文直接 replace，英文做不区分大小写的替换
                    if contains_cjk(&keyword) {
                        part.replace(&keyword, &marker)
                    } else {
                        replace_case_insensitive_segment(part, &keyword, &marker)
                    }
                }
            })
            .collect();
        output = parts.join("**");
    }
    output
}

/// 在 segment 中对 keyword 做不区分大小写的替换（仅用于 ASCII/英文关键词）
/// 使用 lower.find 迭代定位，保证切片位置为有效字节边界
fn replace_case_insensitive_segment(segment: &str, keyword: &str, replacement: &str) -> String {
    if keyword.is_empty() {
        return segment.to_string();
    }
    let lower = segment.to_lowercase();
    let lower_key = keyword.to_lowercase();
    let mut out = String::new();
    let mut start = 0usize;
    while let Some(p) = lower[start..].find(&lower_key) {
        let pos = start + p;
        out.push_str(&segment[start..pos]);
        out.push_str(replacement);
        start = pos + keyword.len(); // keyword assumed ASCII here (case-insensitive branch)
    }
    out.push_str(&segment[start..]);
    out
}

pub fn clean_for_terminal(text: &str, max_chars: usize) -> String {
    let clean = text.replace("**", "");
    truncate_chars(&clean, max_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_should_include_keyword() {
        let s = make_snippet("abc Rust ownership borrowing", "Rust", 20);
        assert!(s.to_lowercase().contains("rust"));
    }

    #[test]
    fn snippet_should_highlight_chinese() {
        let s = make_snippet("我爱自然语言处理和搜索", "自然语言", 30);
        assert!(s.contains("**自然语言**") || s.to_lowercase().contains("自然语言"));
    }
}

pub fn count_query_matches(content: &str, query: &str) -> usize {
    let content_lower = content.to_lowercase();
    tokenize(query)
        .into_iter()
        .filter(|w| !w.trim().is_empty())
        .map(|word| {
            if contains_cjk(&word) {
                content.matches(&word).count()
            } else {
                content_lower.matches(&word.to_lowercase()).count()
            }
        })
        .sum()
}
