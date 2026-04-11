//! `fnord pentabarf` — validate text against the Five Commandments of the
//! Pentabarf. Absurdist but internally consistent.

use std::collections::HashSet;
use std::io::{self, Read};
use std::path::Path;

use chrono::Local;
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::PentabarfArgs;
use crate::config::Config;
use crate::date::convert::to_discordian;
use crate::error::FnordError;
use crate::subcommands::util::sparkle;

const DEITIES_OTHER: &[&str] = &[
    "god", "gods", "goddess", "lord", "allah", "yahweh", "jehovah", "zeus", "odin", "thor",
    "vishnu", "buddha", "jesus", "christ", "jehovah",
];
const ERIS_NAMES: &[&str] = &["eris", "discordia", "kallisti"];

const ORDER_WORDS: &[&str] = &[
    "mandatory",
    "required",
    "prohibited",
    "forbidden",
    "must",
    "shall",
    "comply",
    "enforce",
    "regulate",
    "standardize",
    "normalize",
    "optimize",
    "streamline",
    "synergize",
];

const COMMANDS: &[&str] = &[
    "you must",
    "you should",
    "you will",
    "do this",
    "do that",
    "don't do",
    "do not do",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Compliant,
    PartiallyCompliant,
    Suspicious,
    Warning,
    Violation,
}

impl Status {
    pub fn points(&self) -> u8 {
        match self {
            Status::Compliant => 2,
            Status::PartiallyCompliant => 1,
            Status::Suspicious => 1,
            Status::Warning => 1,
            Status::Violation => 0,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Status::Compliant => "COMPLIANT",
            Status::PartiallyCompliant => "PARTIALLY COMPLIANT",
            Status::Suspicious => "SUSPICIOUS",
            Status::Warning => "WARNING",
            Status::Violation => "VIOLATION",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandmentResult {
    pub number: u8,
    pub title: &'static str,
    pub short: &'static str,
    pub status: Status,
    pub explanation: String,
}

pub fn run(
    args: &PentabarfArgs,
    _config: &Config,
    json_out: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let (label, content) = read_source(args.file.as_deref())?;

    let results = assess(&content);
    let total: u8 = results.iter().map(|r| r.status.points()).sum();
    let verdict = verdict_for(total);

    if json_out {
        return emit_json(&label, &content, &results, total, verdict);
    }

    let lines = content.lines().count();
    let word_count = content.split_whitespace().count();
    let sp = sparkle(no_unicode);

    let heading = format!("  {sp} PENTABARF COMPLIANCE REPORT {sp}");
    if no_color {
        println!("{heading}");
    } else {
        println!("{}", heading.bold().magenta());
    }
    println!();
    println!("  Text: {label} ({lines} lines, {word_count} words)");

    // Discordian date
    let naive = Local::now().date_naive();
    let disc = to_discordian(naive);
    println!("  Assessed: {disc}");
    println!();

    for r in &results {
        let roman = match r.number {
            1 => "I.  ",
            2 => "II. ",
            3 => "III.",
            4 => "IV. ",
            5 => "V.  ",
            _ => "?.  ",
        };
        let title = format!("{} {}", roman, r.title);
        // Pad title to fixed width, then dots, then status
        let dot_line = format!("{title:.<38}");
        let status_colored: String = if no_color {
            r.status.label().to_string()
        } else {
            match r.status {
                Status::Compliant => r.status.label().green().to_string(),
                Status::PartiallyCompliant => r.status.label().yellow().to_string(),
                Status::Suspicious => r.status.label().yellow().to_string(),
                Status::Warning => r.status.label().yellow().to_string(),
                Status::Violation => r.status.label().red().to_string(),
            }
        };
        println!("  {dot_line} {status_colored}");
        println!("       {}", r.explanation);
        println!();
    }

    println!("  ────────────────────────────────────");
    println!("  SCORE: {total}/10 — {verdict}");
    println!();
    println!("  Fnord.");

    if args.strict {
        let code = (10u8.saturating_sub(total)) as i32;
        std::process::exit(code);
    }

    Ok(())
}

fn read_source(path: Option<&Path>) -> Result<(String, String), FnordError> {
    match path {
        Some(p) => {
            let content = std::fs::read_to_string(p)?;
            Ok((p.display().to_string(), content))
        }
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(("<stdin>".to_string(), buf))
        }
    }
}

/// Run the five commandment checks against the given text.
pub fn assess(text: &str) -> Vec<CommandmentResult> {
    vec![
        check_i(text),
        check_ii(text),
        check_iii(text),
        check_iv(text),
        check_v(text),
    ]
}

fn text_lower(text: &str) -> String {
    text.to_lowercase()
}

fn contains_any_word(hay: &str, words: &[&str]) -> bool {
    // Tokenize hay into alphabetic words.
    let tokens: HashSet<String> = hay
        .split(|c: char| !c.is_alphabetic())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    words.iter().any(|w| tokens.contains(*w))
}

/// I — There is no Goddess but Goddess and She is Your Goddess.
pub fn check_i(text: &str) -> CommandmentResult {
    let lower = text_lower(text);
    let mentions_eris = contains_any_word(&lower, ERIS_NAMES);
    let mentions_other = contains_any_word(&lower, DEITIES_OTHER);

    let (status, explanation) = if mentions_eris {
        (
            Status::Compliant,
            "Eris is acknowledged. The Goddess is pleased.".to_string(),
        )
    } else if mentions_other {
        (
            Status::PartiallyCompliant,
            "Other gods noted, Eris not acknowledged.".to_string(),
        )
    } else {
        (
            Status::Compliant,
            "No deities mentioned. Compliance by omission.".to_string(),
        )
    };

    CommandmentResult {
        number: 1,
        title: "Goddess Clause ..................",
        short: "goddess",
        status,
        explanation,
    }
}

/// II — As She is Discordant, so art Thou. Detect simple negation contradictions.
pub fn check_ii(text: &str) -> CommandmentResult {
    let contradictions = count_contradictions(text);
    let (status, explanation) = if contradictions > 0 {
        (
            Status::Compliant,
            format!("{contradictions} contradictions detected. Eris approves."),
        )
    } else {
        (
            Status::Suspicious,
            "No contradictions found. Dangerously orderly.".to_string(),
        )
    };
    CommandmentResult {
        number: 2,
        title: "Discord Clause ..................",
        short: "discord",
        status,
        explanation,
    }
}

/// Count simple word+not pairs: for each word that appears with "not" or "n't"
/// nearby (within 3 tokens), and also appears without that negation elsewhere,
/// count as 1 contradiction.
pub fn count_contradictions(text: &str) -> usize {
    let lower = text.to_lowercase();
    let tokens: Vec<&str> = lower.split_whitespace().collect();
    if tokens.len() < 2 {
        return 0;
    }

    // Collect (word, negated) appearances.
    let mut negated_words: HashSet<String> = HashSet::new();
    let mut bare_words: HashSet<String> = HashSet::new();
    for (i, tok) in tokens.iter().enumerate() {
        // Strip punctuation.
        let w: String = tok.chars().filter(|c| c.is_alphabetic()).collect();
        if w.is_empty() {
            continue;
        }

        // Check for negation within previous 3 tokens.
        let negated = tokens
            .iter()
            .take(i)
            .skip(i.saturating_sub(3))
            .any(|prev| *prev == "not" || *prev == "no" || prev.ends_with("n't"));
        if negated {
            negated_words.insert(w);
        } else {
            bare_words.insert(w);
        }
    }

    negated_words.intersection(&bare_words).count()
}

/// III — Discord is Holy. Vocabulary diversity.
pub fn check_iii(text: &str) -> CommandmentResult {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|s| {
            s.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|s| !s.is_empty())
        .collect();
    let total = words.len();
    let unique: HashSet<&str> = words.iter().map(|s| s.as_str()).collect();
    let ratio = if total == 0 {
        0.0
    } else {
        unique.len() as f64 / total as f64
    };

    let (status, explanation) = if ratio > 0.7 {
        (
            Status::Compliant,
            format!("Vocabulary diversity: {ratio:.2} — rich linguistic chaos."),
        )
    } else if ratio >= 0.4 {
        (
            Status::Compliant,
            format!("Vocabulary diversity: {ratio:.2} — compliant."),
        )
    } else {
        (
            Status::Warning,
            format!("Vocabulary diversity: {ratio:.2} — repetitive. Order creeping in."),
        )
    };

    CommandmentResult {
        number: 3,
        title: "Holy Discord Clause .............",
        short: "holy-discord",
        status,
        explanation,
    }
}

/// IV — Each Soul is Inviolate. No second-person commands.
pub fn check_iv(text: &str) -> CommandmentResult {
    let lower = text.to_lowercase();
    let found: Vec<&str> = COMMANDS
        .iter()
        .copied()
        .filter(|p| lower.contains(*p))
        .collect();
    let (status, explanation) = if found.is_empty() {
        (
            Status::Compliant,
            "No commands detected. Souls remain inviolate.".to_string(),
        )
    } else {
        let list = found
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(", ");
        (
            Status::Violation,
            format!(
                "{} commands detected: {list}. Souls are being violated.",
                found.len()
            ),
        )
    };
    CommandmentResult {
        number: 4,
        title: "Inviolate Soul Clause ...........",
        short: "soul",
        status,
        explanation,
    }
}

/// V — Thou Shalt Not Make the Sacred Chao Sad. Count order-words.
pub fn check_v(text: &str) -> CommandmentResult {
    let lower = text.to_lowercase();
    let tokens: HashSet<String> = lower
        .split(|c: char| !c.is_alphabetic())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    let hits: Vec<&str> = ORDER_WORDS
        .iter()
        .copied()
        .filter(|w| tokens.contains(*w))
        .collect();
    let (status, explanation) = if hits.len() > 3 {
        (
            Status::Violation,
            format!(
                "{} order-words detected: {}. The Sacred Chao weeps.",
                hits.len(),
                hits.iter()
                    .take(8)
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    } else if !hits.is_empty() {
        (
            Status::Warning,
            format!(
                "{} order-words detected: {}. The Sacred Chao is mildly sad.",
                hits.len(),
                hits.iter()
                    .map(|s| format!("\"{s}\""))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        )
    } else {
        (
            Status::Compliant,
            "No order-words detected. The Sacred Chao is content.".to_string(),
        )
    };
    CommandmentResult {
        number: 5,
        title: "Sacred Chao Clause ..............",
        short: "chao",
        status,
        explanation,
    }
}

pub fn verdict_for(score: u8) -> &'static str {
    match score {
        10 => "Perfect Chaos — Eris smiles upon this text",
        8 | 9 => "Adequately Discordian — minor heresies noted",
        6 | 7 => "Questionable — Greyface energy detected",
        4 | 5 => "Troubling — this text leans toward Order",
        _ => "HERESY — Greyface wrote this. Burn it.",
    }
}

fn emit_json(
    label: &str,
    content: &str,
    results: &[CommandmentResult],
    total: u8,
    verdict: &str,
) -> Result<(), FnordError> {
    let cmds: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            json!({
                "number": r.number,
                "clause": r.short,
                "status": r.status.label(),
                "points": r.status.points(),
                "explanation": r.explanation,
            })
        })
        .collect();
    let obj = json!({
        "file": label,
        "lines": content.lines().count(),
        "words": content.split_whitespace().count(),
        "commandments": cmds,
        "score": total,
        "verdict": verdict,
    });
    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commandment_i_eris() {
        let r = check_i("Hail Eris, the goddess of chaos.");
        assert_eq!(r.status, Status::Compliant);
    }

    #[test]
    fn test_commandment_i_other_deities() {
        let r = check_i("Zeus and Odin walked into a bar.");
        assert_eq!(r.status, Status::PartiallyCompliant);
    }

    #[test]
    fn test_commandment_ii_no_contradictions() {
        let r = check_ii("Everything is fine. All is well. The day is good.");
        assert_eq!(r.status, Status::Suspicious);
    }

    #[test]
    fn test_commandment_ii_with_contradiction() {
        let r = check_ii("The cat is happy. The cat is not happy today.");
        assert_eq!(r.status, Status::Compliant);
    }

    #[test]
    fn test_commandment_iii_high_diversity() {
        let r = check_iii("alpha bravo charlie delta echo foxtrot golf hotel india juliet");
        assert_eq!(r.status, Status::Compliant);
    }

    #[test]
    fn test_commandment_iv_violation() {
        let r = check_iv("You must file form 23-B before sunrise.");
        assert_eq!(r.status, Status::Violation);
    }

    #[test]
    fn test_commandment_iv_compliant() {
        let r = check_iv("The cabbage is wise.");
        assert_eq!(r.status, Status::Compliant);
    }

    #[test]
    fn test_commandment_v_violation() {
        let r = check_v("Citizens must comply with required regulations and shall not deviate.");
        assert_eq!(r.status, Status::Violation);
    }

    #[test]
    fn test_commandment_v_compliant() {
        let r = check_v("The apple rolls where it will.");
        assert_eq!(r.status, Status::Compliant);
    }

    #[test]
    fn test_perfect_score_verdict() {
        assert_eq!(
            verdict_for(10),
            "Perfect Chaos — Eris smiles upon this text"
        );
    }

    #[test]
    fn test_zero_score_verdict() {
        assert!(verdict_for(0).contains("HERESY"));
    }

    #[test]
    fn test_perfect_score_is_achievable() {
        // Must mention Eris (I), have contradictions (II), high diversity (III),
        // no commands (IV), no order-words (V).
        let text = "Eris laughed. She was joyful. She was not joyful. \
                    Apples rolled, cabbages danced, hotdogs whispered, \
                    fnords flickered, pineal glands tingled mysteriously.";
        let results = assess(text);
        let total: u8 = results.iter().map(|r| r.status.points()).sum();
        assert_eq!(total, 10, "results: {results:?}");
    }
}
