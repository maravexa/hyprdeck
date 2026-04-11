//! `fnord chaos` — a Discordian `shuf`. Shuffles lines, words, or characters
//! of its input. Eris blesses all randomization.

use std::io::{self, IsTerminal, Read};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

use crate::cli::ChaosArgs;
use crate::config::Config;
use crate::error::FnordError;
use crate::subcommands::util::hash_str;

pub fn run(args: &ChaosArgs, _config: &Config, json_out: bool) -> Result<(), FnordError> {
    let input = read_input(args.file.as_deref())?;

    let seed_str = match args.seed.as_deref() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                / 60;
            ts.to_string()
        }
    };

    let mode = if args.chars {
        Mode::Chars
    } else if args.words {
        Mode::Words
    } else {
        Mode::Lines
    };

    let input_line_count = count_lines(&input);
    let (output, output_line_count) = shuffle(&input, mode, &seed_str);

    if json_out {
        let obj = json!({
            "mode": mode.as_str(),
            "seed": seed_str,
            "input_line_count": input_line_count,
            "output_line_count": output_line_count,
            "text": output,
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    print!("{output}");
    if !output.ends_with('\n') {
        println!();
    }

    // Discordian flavor on stderr only if stderr is a TTY.
    if io::stderr().is_terminal() {
        eprintln!("# shuffled by fnord chaos — all hail eris");
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum Mode {
    Lines,
    Words,
    Chars,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Lines => "lines",
            Mode::Words => "words",
            Mode::Chars => "chars",
        }
    }
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

fn count_lines(content: &str) -> usize {
    if content.is_empty() {
        return 0;
    }
    let n = content.matches('\n').count();
    if content.ends_with('\n') {
        n
    } else {
        n + 1
    }
}

/// Deterministic Fisher-Yates shuffle using FNV-1a-seeded LCG.
pub fn shuffle_slice<T>(items: &mut [T], seed: &str) {
    let mut state = hash_str(seed);
    if items.len() < 2 {
        return;
    }
    for i in (1..items.len()).rev() {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let j = (state >> 32) as usize % (i + 1);
        items.swap(i, j);
    }
}

/// Shuffle the input and return (output_text, output_line_count).
pub fn shuffle(input: &str, mode: Mode, seed: &str) -> (String, usize) {
    match mode {
        Mode::Lines => {
            let mut lines: Vec<&str> = input.lines().collect();
            shuffle_slice(&mut lines, seed);
            let mut out = lines.join("\n");
            if input.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            let count = lines.len();
            (out, count)
        }
        Mode::Words => {
            let mut out_lines: Vec<String> = Vec::new();
            for (i, line) in input.lines().enumerate() {
                let mut words: Vec<&str> = line.split_whitespace().collect();
                let line_seed = format!("{seed}:line:{i}");
                shuffle_slice(&mut words, &line_seed);
                out_lines.push(words.join(" "));
            }
            let mut out = out_lines.join("\n");
            if input.ends_with('\n') && !out.is_empty() {
                out.push('\n');
            }
            (out, out_lines.len())
        }
        Mode::Chars => {
            // Preserve newlines; shuffle non-newline chars globally.
            let chars: Vec<char> = input.chars().collect();
            let positions: Vec<usize> = chars
                .iter()
                .enumerate()
                .filter_map(|(i, c)| if *c != '\n' { Some(i) } else { None })
                .collect();
            let mut to_shuffle: Vec<char> = positions.iter().map(|&i| chars[i]).collect();
            shuffle_slice(&mut to_shuffle, seed);

            let mut out_chars = chars.clone();
            for (k, &pos) in positions.iter().enumerate() {
                out_chars[pos] = to_shuffle[k];
            }
            let out: String = out_chars.into_iter().collect();
            let lc = count_lines(&out);
            (out, lc)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_line_shuffle_preserves_content() {
        let input = "one\ntwo\nthree\nfour\nfive\n";
        let (out, _) = shuffle(input, Mode::Lines, "seed");
        let in_set: HashSet<&str> = input.lines().collect();
        let out_set: HashSet<&str> = out.lines().collect();
        assert_eq!(in_set, out_set);
    }

    #[test]
    fn test_word_shuffle_preserves_words_per_line() {
        let input = "alpha bravo charlie delta echo foxtrot golf hotel\n";
        let (out, _) = shuffle(input, Mode::Words, "seed");
        let in_words: HashSet<&str> = input.split_whitespace().collect();
        let out_words: HashSet<&str> = out.split_whitespace().collect();
        assert_eq!(in_words, out_words);
    }

    #[test]
    fn test_same_seed_same_shuffle() {
        let input = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\n";
        let (a, _) = shuffle(input, Mode::Lines, "same");
        let (b, _) = shuffle(input, Mode::Lines, "same");
        assert_eq!(a, b);
    }

    #[test]
    fn test_different_seeds_differ() {
        let input = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\n";
        let mut outputs: HashSet<String> = HashSet::new();
        for i in 0..10 {
            let seed = format!("s{i}");
            let (out, _) = shuffle(input, Mode::Lines, &seed);
            outputs.insert(out);
        }
        // With 16! possible orderings, 10 different seeds should never all collide.
        assert!(outputs.len() > 1, "all seeds produced identical shuffles");
    }

    #[test]
    fn test_char_shuffle_preserves_newlines() {
        let input = "abc\ndef\n";
        let (out, _) = shuffle(input, Mode::Chars, "seed");
        // Newlines preserved at same positions.
        assert_eq!(out.chars().nth(3), Some('\n'));
        assert_eq!(out.chars().nth(7), Some('\n'));
    }
}
