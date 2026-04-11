use std::io::{self, BufRead, Write};

use chrono::Local;
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::OracleArgs;
use crate::config::Config;
use crate::error::FnordError;
use crate::subcommands::util::{hash_str, pick, sparkle};
use crate::subcommands::wordlists::{ORACLE_CLOSINGS, ORACLE_MIDDLES, ORACLE_OPENINGS};

pub fn run(
    args: &OracleArgs,
    _config: &Config,
    json: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let question = match &args.question {
        Some(q) => q.clone(),
        None => prompt_for_question()?,
    };

    let normalized = question.to_lowercase();
    let normalized = normalized.trim();
    let base_seed = hash_str(normalized);

    let (seed, chaotic) = if args.chaos {
        let ts = Local::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        (hash_str(&format!("chaos:{ts}:{base_seed}")), true)
    } else {
        (base_seed, false)
    };

    let answer = generate_answer(&question, seed);

    if json {
        let obj = json!({
            "question": question,
            "answer": answer,
            "seed": format!("{seed:016x}"),
            "chaotic": chaotic,
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    render(
        &question,
        &answer,
        seed,
        chaotic,
        args.reveal_seed,
        no_color,
        no_unicode,
    );
    Ok(())
}

fn prompt_for_question() -> Result<String, FnordError> {
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    write!(handle, "Your question for the Oracle: ")?;
    handle.flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let trimmed = line.trim().to_string();
    if trimmed.is_empty() {
        return Err(FnordError::Parse(
            "the Oracle requires a question".to_string(),
        ));
    }
    Ok(trimmed)
}

/// Build an answer by combining opening + middle + closing phrases.
pub fn generate_answer(question: &str, seed: u64) -> String {
    let opening = pick(ORACLE_OPENINGS, seed);
    let middle_raw = pick(ORACLE_MIDDLES, seed / (ORACLE_OPENINGS.len() as u64).max(1));
    let closing = pick(
        ORACLE_CLOSINGS,
        seed / ((ORACLE_OPENINGS.len() * ORACLE_MIDDLES.len()) as u64).max(1),
    );

    let n = count_fives(question);
    let middle = middle_raw.replace("{n}", &n.to_string());

    format!("{opening} {middle} {closing}")
}

/// "Law of Fives" count: every '5' digit or 'f'/'F' letter in the question.
fn count_fives(q: &str) -> usize {
    q.chars()
        .filter(|c| *c == '5' || c.eq_ignore_ascii_case(&'f'))
        .count()
}

fn render(
    question: &str,
    answer: &str,
    seed: u64,
    chaotic: bool,
    reveal_seed: bool,
    no_color: bool,
    no_unicode: bool,
) {
    let sp = sparkle(no_unicode);
    let heading = format!("  {sp} THE ORACLE SPEAKS {sp}");
    println!();
    if no_color {
        println!("{heading}");
    } else {
        println!("{}", heading.bold().cyan());
    }
    println!();
    println!("  You asked: \"{question}\"");
    println!();
    if no_color {
        println!("  {answer}");
    } else {
        println!("  {}", answer.italic());
    }
    println!();
    if chaotic {
        println!("  The Oracle is feeling chaotic today.");
    }
    if reveal_seed {
        println!("  [Seed: {seed:016x} (raw: {seed}) — the Oracle is consistent]");
    } else {
        println!("  [Seed: {seed:016x} — the Oracle is consistent]");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_question_same_answer() {
        let seed1 = hash_str("is a hotdog a sandwich");
        let seed2 = hash_str("is a hotdog a sandwich");
        assert_eq!(seed1, seed2);
        let a1 = generate_answer("is a hotdog a sandwich", seed1);
        let a2 = generate_answer("is a hotdog a sandwich", seed2);
        assert_eq!(a1, a2);
    }

    #[test]
    fn answer_contains_components() {
        let seed = hash_str("what is the sound of five tons of flax");
        let a = generate_answer("what is the sound of five tons of flax", seed);
        assert!(!a.is_empty());
        // At least one opening, one closing — we don't know which, but one must match.
        let has_opening = ORACLE_OPENINGS.iter().any(|o| a.contains(o));
        let has_closing = ORACLE_CLOSINGS.iter().any(|c| a.contains(c));
        assert!(has_opening, "answer missing opening");
        assert!(has_closing, "answer missing closing");
    }

    #[test]
    fn count_fives_counts_5s_and_fs() {
        assert_eq!(count_fives(""), 0);
        assert_eq!(count_fives("fnord"), 1);
        assert_eq!(count_fives("FNORD"), 1);
        // "five tons of flax 55" contains: 'f' in 'five', 'f' in 'of',
        // 'f' in 'flax', then '5','5' = 5 total.
        assert_eq!(count_fives("five tons of flax 55"), 5);
    }

    #[test]
    fn chaos_flag_does_not_panic() {
        // We can't call run() directly without clap, so just simulate the
        // seed-mixing path and ensure generate_answer still works.
        let ts = Local::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        let seed = hash_str(&format!("chaos:{ts}:{}", hash_str("hi")));
        let a = generate_answer("hi", seed);
        assert!(!a.is_empty());
    }
}
