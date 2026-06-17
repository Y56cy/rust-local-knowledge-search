// ...existing code...
use jieba_rs::Jieba;

/// 对文本进行分词，返回 token 向量（中文使用 jieba 的搜索分词，英文保留按空白拆分）
pub fn tokenize(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    if contains_cjk(text) {
        let jieba = Jieba::new();
        jieba
            .cut_for_search(text, true)
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        text.split_whitespace()
            .map(|s| s.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
}

/// 将分词结果 join 为以空格分隔的字符串，方便送入默认的 whitespace tokenizer 或直接存入索引字段
pub fn join_tokens(text: &str) -> String {
    tokenize(text).join(" ")
}

/// 简单判断是否存在中文/日文/韩文（CJK）字符
pub fn contains_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0x4E00..=0x9FFF).contains(&u)   // CJK Unified Ideographs
        || (0x3040..=0x309F).contains(&u) // Hiragana
        || (0x30A0..=0x30FF).contains(&u) // Katakana
        || (0xAC00..=0xD7AF).contains(&u) // Hangul Syllables
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_chinese() {
        let s = "我爱自然语言处理";
        let toks = tokenize(s);
        assert!(toks.iter().any(|t| t.contains("自然")));
    }

    #[test]
    fn join_tokens_english() {
        let s = "Rust ownership borrowing";
        let j = join_tokens(s);
        assert_eq!(j, "Rust ownership borrowing");
    }
}
