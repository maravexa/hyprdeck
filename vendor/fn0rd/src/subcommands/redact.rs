//! The `fnord fnord` redactor — replace a configurable percentage of words
//! with `FNORD` (or the configured replacement). The fnords were always
//! there. This just makes them visible.

use std::io::{self, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::cli::FnordRedactArgs;
use crate::config::Config;
use crate::error::FnordError;
use crate::subcommands::util::hash_str;

/// Structural words preserved when `preserve_structure` is true.
const STRUCTURAL_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "to", "of", "and", "or",
    "but", "in", "on", "at", "for", "with", "by", "as", "it", "its", "this", "that", "these",
    "those", "i", "you", "he", "she", "we", "they", "my", "your", "his", "her", "our", "their",
];

/// A single token of the tokenized input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    Word(String),
    NonWord(String),
}

pub fn run(args: &FnordRedactArgs, config: &Config, json_out: bool) -> Result<(), FnordError> {
    let input = read_input(args.file.as_deref())?;

    let replacement = args
        .replacement
        .clone()
        .unwrap_or_else(|| config.fnord.replacement.clone());
    let rate = args.rate.unwrap_or(config.fnord.rate).clamp(0.0, 1.0);
    let preserve_structure = config.fnord.preserve_structure && !args.pure_chaos;

    let seed = match args.seed.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            if !config.fnord.seed.is_empty() {
                config.fnord.seed.clone()
            } else {
                // Minute-truncated Unix timestamp
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
                    / 60;
                ts.to_string()
            }
        }
    };

    let result = redact_text(&input, &replacement, rate, preserve_structure, &seed);

    if json_out {
        let obj = json!({
            "original_word_count": result.original_word_count,
            "replaced_count": result.replaced_count,
            "replacement_rate_actual": result.actual_rate(),
            "text": result.text,
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    } else {
        print!("{}", result.text);
    }

    Ok(())
}

fn read_input(path: Option<&Path>) -> Result<String, FnordError> {
    match path {
        Some(p) => Ok(std::fs::read_to_string(p)?),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

/// Result of a redaction operation.
pub struct RedactResult {
    pub text: String,
    pub original_word_count: usize,
    pub replaced_count: usize,
}

impl RedactResult {
    pub fn actual_rate(&self) -> f64 {
        if self.original_word_count == 0 {
            0.0
        } else {
            self.replaced_count as f64 / self.original_word_count as f64
        }
    }
}

/// Redact text by replacing a percentage of words with the replacement.
pub fn redact_text(
    input: &str,
    replacement: &str,
    rate: f64,
    preserve_structure: bool,
    seed: &str,
) -> RedactResult {
    let tokens = tokenize(input);
    let mut out = String::with_capacity(input.len());
    let mut word_idx: usize = 0;
    let mut word_count: usize = 0;
    let mut replaced: usize = 0;

    for tok in tokens {
        match tok {
            Token::NonWord(s) => out.push_str(&s),
            Token::Word(w) => {
                word_count += 1;

                let is_structural = is_structural_word(&w);
                let eligible = !(preserve_structure && is_structural);

                let decide = if eligible {
                    let h = hash_str(&format!("{seed}:{word_idx}"));
                    // Normalize hash to [0.0, 1.0)
                    let r = (h as f64) / (u64::MAX as f64);
                    r < rate
                } else {
                    false
                };

                if decide {
                    out.push_str(&apply_case_pattern(&w, replacement));
                    replaced += 1;
                } else {
                    out.push_str(&w);
                }

                word_idx += 1;
            }
        }
    }

    RedactResult {
        text: out,
        original_word_count: word_count,
        replaced_count: replaced,
    }
}

/// Tokenize input text into alternating Word and NonWord tokens. A word is a
/// maximal run of alphabetic characters or apostrophes (to keep contractions
/// whole). Everything else is NonWord.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut current = String::new();
    let mut in_word = false;

    for c in input.chars() {
        let is_wordy = c.is_alphabetic() || c == '\'';
        if is_wordy && !in_word {
            if !current.is_empty() {
                tokens.push(Token::NonWord(std::mem::take(&mut current)));
            }
            in_word = true;
            current.push(c);
        } else if !is_wordy && in_word {
            if !current.is_empty() {
                tokens.push(Token::Word(std::mem::take(&mut current)));
            }
            in_word = false;
            current.push(c);
        } else {
            current.push(c);
        }
    }

    if !current.is_empty() {
        if in_word {
            tokens.push(Token::Word(current));
        } else {
            tokens.push(Token::NonWord(current));
        }
    }

    tokens
}

fn is_structural_word(w: &str) -> bool {
    let lower = w.to_ascii_lowercase();
    STRUCTURAL_WORDS.iter().any(|s| *s == lower)
}

/// Apply the capitalization pattern of `original` to `replacement`.
/// ALL CAPS → all caps, Title → title, else → lower.
pub fn apply_case_pattern(original: &str, replacement: &str) -> String {
    let has_alpha = original.chars().any(|c| c.is_alphabetic());
    if !has_alpha {
        return replacement.to_string();
    }

    let all_upper = original
        .chars()
        .filter(|c| c.is_alphabetic())
        .all(|c| c.is_uppercase());
    if all_upper {
        return replacement.to_uppercase();
    }

    let first = original.chars().find(|c| c.is_alphabetic());
    let rest_lower = original
        .chars()
        .filter(|c| c.is_alphabetic())
        .skip(1)
        .all(|c| c.is_lowercase());

    if matches!(first, Some(c) if c.is_uppercase()) && rest_lower {
        // Title Case
        let mut chars = replacement.chars();
        match chars.next() {
            Some(first) => {
                let mut s: String = first.to_uppercase().collect();
                s.extend(chars.flat_map(|c| c.to_lowercase()));
                return s;
            }
            None => return String::new(),
        }
    }

    replacement.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_basic() {
        let toks = tokenize("The quick brown fox.");
        assert_eq!(
            toks,
            vec![
                Token::Word("The".to_string()),
                Token::NonWord(" ".to_string()),
                Token::Word("quick".to_string()),
                Token::NonWord(" ".to_string()),
                Token::Word("brown".to_string()),
                Token::NonWord(" ".to_string()),
                Token::Word("fox".to_string()),
                Token::NonWord(".".to_string()),
            ]
        );
    }

    #[test]
    fn test_apply_case_pattern() {
        assert_eq!(apply_case_pattern("HELLO", "fnord"), "FNORD");
        assert_eq!(apply_case_pattern("Hello", "fnord"), "Fnord");
        assert_eq!(apply_case_pattern("hello", "FNORD"), "fnord");
    }

    #[test]
    fn test_rate_zero_replaces_nothing() {
        let r = redact_text("one two three four five", "FNORD", 0.0, false, "seed");
        assert_eq!(r.text, "one two three four five");
        assert_eq!(r.replaced_count, 0);
    }

    #[test]
    fn test_rate_one_with_pure_chaos_replaces_all() {
        let r = redact_text(
            "The quick brown fox jumps over the lazy dog.",
            "FNORD",
            1.0,
            false,
            "seed",
        );
        // Every word should be replaced (case-preserved to fnord since input is lowercase).
        assert_eq!(r.replaced_count, r.original_word_count);
        assert!(r.text.contains("FNORD") || r.text.contains("Fnord") || r.text.contains("fnord"));
        // No original words should remain.
        for w in ["quick", "brown", "jumps", "over", "lazy"] {
            assert!(!r.text.contains(w), "word {w} was not replaced");
        }
    }

    #[test]
    fn test_structural_words_preserved() {
        // With preserve_structure=true and rate=1.0, structural words remain.
        let r = redact_text(
            "the quick brown fox is a hotdog",
            "FNORD",
            1.0,
            true,
            "seed",
        );
        assert!(r.text.contains("the"));
        assert!(r.text.contains("is"));
        assert!(r.text.contains("a"));
        // Non-structural should be replaced
        assert!(!r.text.contains("quick"));
        assert!(!r.text.contains("brown"));
    }

    #[test]
    fn test_same_seed_same_output() {
        let a = redact_text(
            "the quick brown fox jumps over the lazy dog",
            "FNORD",
            0.5,
            false,
            "alpha",
        );
        let b = redact_text(
            "the quick brown fox jumps over the lazy dog",
            "FNORD",
            0.5,
            false,
            "alpha",
        );
        assert_eq!(a.text, b.text);
    }

    #[test]
    fn test_different_seeds_differ() {
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let a = redact_text(text, "FNORD", 0.5, false, "alpha");
        let b = redact_text(text, "FNORD", 0.5, false, "bravo");
        // Very likely to differ; at minimum one of several seeds should.
        let c = redact_text(text, "FNORD", 0.5, false, "charlie");
        assert!(
            a.text != b.text || a.text != c.text,
            "different seeds produced identical output"
        );
    }

    #[test]
    fn test_capitalization_preserved_on_replacement() {
        // Use pure chaos + rate 1.0 so all words are replaced, then check
        // each replacement's case matches the original.
        let r = redact_text("HELLO World lower", "FNORD", 1.0, false, "seed");
        assert!(r.text.contains("FNORD"));
        assert!(r.text.contains("Fnord"));
        assert!(r.text.contains("fnord"));
    }
}
