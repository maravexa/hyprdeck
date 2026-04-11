//! `fnord law` — a Discordian `grep`. Searches for patterns and then
//! applies the Law of Fives to the match count.

use std::io::{self, Read};

use serde_json::json;

use crate::cli::LawArgs;
use crate::config::Config;
use crate::error::FnordError;

#[derive(Debug, Clone)]
pub struct Match {
    pub file: String,
    pub line_number: usize,
    pub line: String,
}

pub fn run(args: &LawArgs, _config: &Config, json_out: bool) -> Result<(), FnordError> {
    let mut sources: Vec<(String, String)> = Vec::new();
    if args.files.is_empty() {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        sources.push(("-".to_string(), buf));
    } else {
        for f in &args.files {
            let content = std::fs::read_to_string(f)?;
            sources.push((display_name(f), content));
        }
    }

    let matches = search_all(
        &sources,
        &args.pattern,
        args.ignore_case,
        args.word,
        args.invert,
    );
    let file_count = sources.len();

    if json_out {
        let law = apply_law_of_fives(matches.len());
        let arr: Vec<serde_json::Value> = matches
            .iter()
            .map(|m| {
                json!({
                    "file": m.file,
                    "line": m.line_number,
                    "text": m.line,
                })
            })
            .collect();
        let obj = json!({
            "pattern": args.pattern,
            "match_count": matches.len(),
            "matches": arr,
            "law_analysis": law,
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    for m in &matches {
        println!("{}:{}: {}", m.file, m.line_number, m.line);
    }

    if !args.no_law {
        println!();
        println!(
            "  {} matches found in {} file{}.",
            matches.len(),
            file_count,
            if file_count == 1 { "" } else { "s" }
        );
        println!();
        println!("  LAW OF FIVES ANALYSIS:");
        println!("  {}", apply_law_of_fives(matches.len()));
        let n_str = matches.len().to_string();
        let fives_in_count = n_str
            .chars()
            .filter(|c| *c == '5' || *c == 'f' || *c == 'F')
            .count();
        println!(
            "  The digit/letter five appears {} times in \"{}\". {}",
            fives_in_count,
            n_str,
            oracle_hint(fives_in_count)
        );
    }

    Ok(())
}

fn display_name(p: &std::path::Path) -> String {
    p.display().to_string()
}

fn oracle_hint(n: usize) -> &'static str {
    if n == 0 {
        "The Oracle is consulting."
    } else {
        "The Law resonates within the number itself."
    }
}

/// Run the search across all sources and return match records in order.
pub fn search_all(
    sources: &[(String, String)],
    pattern: &str,
    ignore_case: bool,
    whole_word: bool,
    invert: bool,
) -> Vec<Match> {
    let needle = if ignore_case {
        pattern.to_lowercase()
    } else {
        pattern.to_string()
    };

    let mut out = Vec::new();
    for (name, content) in sources {
        for (idx, line) in content.lines().enumerate() {
            let hay = if ignore_case {
                line.to_lowercase()
            } else {
                line.to_string()
            };

            let found = if whole_word {
                contains_whole_word(&hay, &needle)
            } else {
                hay.contains(&needle)
            };

            let matched = found ^ invert;
            if matched {
                out.push(Match {
                    file: name.clone(),
                    line_number: idx + 1,
                    line: line.to_string(),
                });
            }
        }
    }
    out
}

fn contains_whole_word(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let mut start = 0usize;
    while let Some(pos) = hay[start..].find(needle) {
        let abs = start + pos;
        let before_ok = abs == 0
            || !hay[..abs]
                .chars()
                .next_back()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);
        let after_idx = abs + needle.len();
        let after_ok = after_idx >= hay.len()
            || !hay[after_idx..]
                .chars()
                .next()
                .map(|c| c.is_alphanumeric() || c == '_')
                .unwrap_or(false);
        if before_ok && after_ok {
            return true;
        }
        start = abs + needle.len();
        if start >= hay.len() {
            break;
        }
    }
    false
}

/// Apply the Law of Fives to `n`.
pub fn apply_law_of_fives(n: usize) -> String {
    if n == 0 {
        "Zero matches. Zero is five minus five. The Law holds.".to_string()
    } else if n == 5 {
        "Five matches. The Law of Fives is self-evident.".to_string()
    } else if n % 5 == 0 {
        "Divisible by five. The Law of Fives is satisfied.".to_string()
    } else {
        let nearest_five = (((n as f64) / 5.0).round() * 5.0) as usize;
        let dist = nearest_five.abs_diff(n);
        format!("{n} matches. {n} is {dist} away from {nearest_five}. The Law of Fives remains true in spirit.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> Vec<(String, String)> {
        vec![(
            "test.txt".to_string(),
            "the quick brown fox\nThe Quick Brown Fox\nhello world\nfox in boxes\n".to_string(),
        )]
    }

    #[test]
    fn test_basic_match() {
        let ms = search_all(&sources(), "quick", false, false, false);
        assert_eq!(ms.len(), 1);
        assert_eq!(ms[0].line_number, 1);
    }

    #[test]
    fn test_ignore_case() {
        let ms = search_all(&sources(), "QUICK", true, false, false);
        assert_eq!(ms.len(), 2);
    }

    #[test]
    fn test_invert() {
        let ms = search_all(&sources(), "fox", false, false, true);
        // Case-sensitive: lines 1 and 4 contain "fox"; lines 2 ("Fox") and 3 do not.
        assert_eq!(ms.len(), 2);
        assert_eq!(ms[0].line_number, 2);
        assert_eq!(ms[1].line_number, 3);
    }

    #[test]
    fn test_whole_word() {
        let ms = search_all(&sources(), "fox", false, true, false);
        // "fox" appears as a whole word on lines 1 and 4. Line 2's "Fox" is
        // case-sensitive miss; line 4's "boxes" does not contain "fox".
        assert_eq!(ms.len(), 2);
    }

    #[test]
    fn test_law_of_fives_zero() {
        assert!(apply_law_of_fives(0).contains("Zero is five minus five"));
    }

    #[test]
    fn test_law_of_fives_five() {
        assert!(apply_law_of_fives(5).contains("self-evident"));
    }

    #[test]
    fn test_law_of_fives_divisible() {
        assert!(apply_law_of_fives(10).contains("Divisible by five"));
    }

    #[test]
    fn test_law_of_fives_distance() {
        let s = apply_law_of_fives(3);
        assert!(s.contains("3 is 2 away from 5"));
    }
}
