use std::io::{self, IsTerminal, Read};

use chrono::Local;
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::KoanArgs;
use crate::config::Config;
use crate::error::FnordError;
use crate::subcommands::util::{hash_str, pick, sparkle};
use crate::subcommands::wordlists::{
    KOAN_QUESTIONS, KOAN_REFLECTIONS, KOAN_RESPONSES, KOAN_SETUPS,
};

const MAX_INPUT_LEN: usize = 280;

/// A fully-rendered koan.
#[derive(Debug, Clone)]
pub struct Koan {
    pub setup: String,
    pub question: String,
    pub response: String,
    pub reflection: Option<String>,
}

pub fn run(
    args: &KoanArgs,
    _config: &Config,
    json: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let count = args.count.max(1);

    // Collect piped input if stdin is not a terminal.
    let piped = read_piped_input()?;

    // Base seed: either user-supplied --seed or the current time.
    let base_seed = match &args.seed {
        Some(s) => hash_str(s),
        None => Local::now().timestamp_nanos_opt().unwrap_or(0) as u64,
    };

    let mut koans: Vec<Koan> = Vec::with_capacity(count);
    for i in 0..count {
        let seed = hash_str(&format!("{base_seed}:{i}"));
        koans.push(generate_koan(seed, piped.as_deref()));
    }

    if json {
        let arr: Vec<serde_json::Value> = koans
            .iter()
            .map(|k| {
                json!({
                    "setup": k.setup,
                    "question": k.question,
                    "response": k.response,
                    "reflection": k.reflection,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap());
        return Ok(());
    }

    for (i, k) in koans.iter().enumerate() {
        if i > 0 {
            println!();
        }
        render_koan(k, no_color, no_unicode);
    }

    Ok(())
}

/// Read piped input from stdin if it is not a terminal. Input is
/// truncated to `MAX_INPUT_LEN` characters.
fn read_piped_input() -> Result<Option<String>, FnordError> {
    let stdin = io::stdin();
    if stdin.is_terminal() {
        return Ok(None);
    }
    let mut buf = String::new();
    stdin.lock().read_to_string(&mut buf)?;
    let trimmed = buf.trim().to_string();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Ok(Some(truncate_chars(&trimmed, MAX_INPUT_LEN)))
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

/// Build a single koan from a deterministic seed, optionally
/// incorporating a reflection on piped input.
pub fn generate_koan(seed: u64, piped: Option<&str>) -> Koan {
    let setup = pick(KOAN_SETUPS, seed).to_string();
    let question = pick(KOAN_QUESTIONS, seed / (KOAN_SETUPS.len() as u64).max(1)).to_string();
    let response = pick(
        KOAN_RESPONSES,
        seed / ((KOAN_SETUPS.len() * KOAN_QUESTIONS.len()) as u64).max(1),
    )
    .to_string();

    let reflection = piped.map(|input| {
        let tmpl = pick(KOAN_REFLECTIONS, hash_str(&format!("reflect:{seed}")));
        tmpl.replace("{input}", input)
    });

    Koan {
        setup,
        question,
        response,
        reflection,
    }
}

fn render_koan(k: &Koan, no_color: bool, no_unicode: bool) {
    let sp = sparkle(no_unicode);
    let heading = format!("  {sp} KOAN {sp}");
    if no_color {
        println!("{heading}");
    } else {
        println!("{}", heading.bold().magenta());
    }
    println!();
    println!("  {} {}", k.setup, k.question);
    println!();
    if no_color {
        println!("  {}", k.response);
    } else {
        println!("  {}", k.response.italic());
    }
    if let Some(r) = &k.reflection {
        println!();
        println!("  {r}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_produces_same_koan() {
        let k1 = generate_koan(42, None);
        let k2 = generate_koan(42, None);
        assert_eq!(k1.setup, k2.setup);
        assert_eq!(k1.question, k2.question);
        assert_eq!(k1.response, k2.response);
    }

    #[test]
    fn parts_are_non_empty() {
        let k = generate_koan(12345, None);
        assert!(!k.setup.is_empty());
        assert!(!k.question.is_empty());
        assert!(!k.response.is_empty());
        assert!(k.reflection.is_none());
    }

    #[test]
    fn piped_input_appends_reflection() {
        let k = generate_koan(99, Some("chaos reigns"));
        assert!(k.reflection.is_some());
        let r = k.reflection.unwrap();
        assert!(r.contains("chaos reigns"));
    }

    #[test]
    fn different_seeds_can_produce_different_koans() {
        // Across many seeds, at least two should differ somewhere.
        let koans: Vec<Koan> = (0..10)
            .map(|i| generate_koan(hash_str(&format!("{i}")), None))
            .collect();
        let set: std::collections::HashSet<&String> = koans.iter().map(|k| &k.setup).collect();
        let set2: std::collections::HashSet<&String> = koans.iter().map(|k| &k.question).collect();
        let set3: std::collections::HashSet<&String> = koans.iter().map(|k| &k.response).collect();
        assert!(set.len() > 1 || set2.len() > 1 || set3.len() > 1);
    }

    #[test]
    fn truncate_chars_limits_length() {
        let long = "a".repeat(500);
        let t = truncate_chars(&long, MAX_INPUT_LEN);
        assert_eq!(t.chars().count(), MAX_INPUT_LEN);
    }
}
