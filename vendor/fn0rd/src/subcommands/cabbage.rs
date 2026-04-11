//! `fnord cabbage` — a Discordian `wc`. Counts content in Cabbages,
//! Discord Units, Ergs of Confusion, Fnord Density, and Law of Fives Index.

use std::collections::HashSet;
use std::io::{self, Read};

use serde_json::json;

use crate::cli::CabbageArgs;
use crate::config::Config;
use crate::error::FnordError;

#[derive(Debug, Clone, Default)]
pub struct CabbageMetrics {
    pub cabbages: usize,      // lines
    pub discord_units: usize, // words
    pub ergs: usize,          // bytes
    pub unique_words: usize,  // for density
    pub lof_index: usize,     // count of '5' and 'f'/'F'
}

impl CabbageMetrics {
    pub fn fnord_density(&self) -> f64 {
        if self.discord_units == 0 {
            0.0
        } else {
            self.unique_words as f64 / self.discord_units as f64
        }
    }
}

pub fn run(args: &CabbageArgs, _config: &Config, json_out: bool) -> Result<(), FnordError> {
    // Gather (label, content) pairs.
    let mut entries: Vec<(String, String)> = Vec::new();
    if args.files.is_empty() {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        entries.push(("-".to_string(), buf));
    } else {
        for f in &args.files {
            let content = std::fs::read_to_string(f)?;
            entries.push((display_name(f), content));
        }
    }

    let per_file: Vec<(String, CabbageMetrics)> = entries
        .into_iter()
        .map(|(name, content)| {
            let m = compute_metrics(&content);
            (name, m)
        })
        .collect();

    // Individual pipeable flags: emit only a single number.
    if args.cabbages || args.discord_units || args.ergs {
        let mut total: usize = 0;
        for (_, m) in &per_file {
            if args.cabbages {
                total += m.cabbages;
            } else if args.discord_units {
                total += m.discord_units;
            } else if args.ergs {
                total += m.ergs;
            }
        }
        println!("{total}");
        return Ok(());
    }

    if json_out {
        return emit_json(&per_file);
    }

    if per_file.len() == 1 {
        print_single(&per_file[0].1);
    } else {
        print_table(&per_file);
    }

    Ok(())
}

fn display_name(p: &std::path::Path) -> String {
    p.display().to_string()
}

pub fn compute_metrics(content: &str) -> CabbageMetrics {
    let cabbages = count_lines(content);
    let words: Vec<String> = content
        .split_whitespace()
        .map(|s| {
            s.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .filter(|s| !s.is_empty())
        .collect();
    let discord_units = words.len();
    let ergs = content.len();
    let unique: HashSet<&str> = words.iter().map(|s| s.as_str()).collect();
    let unique_words = unique.len();

    let mut lof_index = 0usize;
    for c in content.chars() {
        if c == '5' || c == 'f' || c == 'F' {
            lof_index += 1;
        }
    }

    CabbageMetrics {
        cabbages,
        discord_units,
        ergs,
        unique_words,
        lof_index,
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

pub fn assessment(density: f64) -> &'static str {
    let pct = density * 100.0;
    if pct < 30.0 {
        "Suspiciously orderly. Greyface wrote this."
    } else if pct < 50.0 {
        "Some entropy detected. Keep going."
    } else if pct < 70.0 {
        "Adequately chaotic. The Sacred Chao nods."
    } else if pct < 85.0 {
        "A moderately chaotic document. Eris approves."
    } else {
        "Pure chaos. The Law of Fives is strong here."
    }
}

fn print_single(m: &CabbageMetrics) {
    let density = m.fnord_density() * 100.0;
    println!("  {} Cabbages", format_int(m.cabbages));
    println!("  {} Discord Units", format_int(m.discord_units));
    println!("  {} Ergs of Confusion", format_int(m.ergs));
    println!("  {:.1}% Fnord Density", density);
    println!("  {} LoF Index", format_int(m.lof_index));
    println!();
    println!("  ASSESSMENT: {}", assessment(m.fnord_density()));
}

fn print_table(per_file: &[(String, CabbageMetrics)]) {
    println!(
        "  {:>8}   {:>13}   {:>17}   {:>13}   {:>9}   File",
        "Cabbages", "Discord Units", "Ergs of Confusion", "Fnord Density", "LoF Index"
    );
    println!(
        "  {:─>8}   {:─>13}   {:─>17}   {:─>13}   {:─>9}   ────",
        "", "", "", "", ""
    );

    let mut total = CabbageMetrics::default();
    let mut total_unique_approx: usize = 0;
    for (name, m) in per_file {
        let density = m.fnord_density() * 100.0;
        println!(
            "  {:>8}   {:>13}   {:>17}   {:>12.1}%   {:>9}   {}",
            format_int(m.cabbages),
            format_int(m.discord_units),
            format_int(m.ergs),
            density,
            format_int(m.lof_index),
            name
        );
        total.cabbages += m.cabbages;
        total.discord_units += m.discord_units;
        total.ergs += m.ergs;
        total.lof_index += m.lof_index;
        total_unique_approx += m.unique_words;
    }

    // Total density is a reasonable average (unique approx / total words).
    total.unique_words = total_unique_approx;
    let total_density = total.fnord_density() * 100.0;

    println!(
        "  {:─>8}   {:─>13}   {:─>17}   {:─>13}   {:─>9}",
        "", "", "", "", ""
    );
    println!(
        "  {:>8}   {:>13}   {:>17}   {:>12.1}%   {:>9}   total",
        format_int(total.cabbages),
        format_int(total.discord_units),
        format_int(total.ergs),
        total_density,
        format_int(total.lof_index),
    );
}

fn format_int(n: usize) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    let len = bytes.len();
    for (i, c) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

fn emit_json(per_file: &[(String, CabbageMetrics)]) -> Result<(), FnordError> {
    let files: Vec<serde_json::Value> = per_file
        .iter()
        .map(|(name, m)| {
            json!({
                "file": name,
                "cabbages": m.cabbages,
                "discord_units": m.discord_units,
                "ergs_of_confusion": m.ergs,
                "fnord_density": m.fnord_density(),
                "lof_index": m.lof_index,
                "assessment": assessment(m.fnord_density()),
            })
        })
        .collect();

    let mut total = CabbageMetrics::default();
    let mut total_unique: usize = 0;
    for (_, m) in per_file {
        total.cabbages += m.cabbages;
        total.discord_units += m.discord_units;
        total.ergs += m.ergs;
        total.lof_index += m.lof_index;
        total_unique += m.unique_words;
    }
    total.unique_words = total_unique;

    let out = json!({
        "files": files,
        "totals": {
            "cabbages": total.cabbages,
            "discord_units": total.discord_units,
            "ergs_of_confusion": total.ergs,
            "fnord_density": total.fnord_density(),
            "lof_index": total.lof_index,
        }
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_count() {
        let m = compute_metrics("one\ntwo\nthree\n");
        assert_eq!(m.cabbages, 3);
    }

    #[test]
    fn test_line_count_no_trailing_newline() {
        let m = compute_metrics("one\ntwo\nthree");
        assert_eq!(m.cabbages, 3);
    }

    #[test]
    fn test_word_count() {
        let m = compute_metrics("the quick brown fox");
        assert_eq!(m.discord_units, 4);
    }

    #[test]
    fn test_byte_count() {
        let m = compute_metrics("hello");
        assert_eq!(m.ergs, 5);
    }

    #[test]
    fn test_fnord_density_all_unique() {
        let m = compute_metrics("alpha bravo charlie delta");
        assert!((m.fnord_density() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_fnord_density_all_identical() {
        let m = compute_metrics("word word word word word word word word word word");
        // 1 unique / 10 total = 0.1
        assert!((m.fnord_density() - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_lof_index_counts_fives_and_fs() {
        let m = compute_metrics("f55 fFa 555");
        // 'f', '5', '5', 'f', 'F', '5', '5', '5' = 8
        assert_eq!(m.lof_index, 8);
    }

    #[test]
    fn test_assessment_thresholds() {
        assert_eq!(
            assessment(0.0),
            "Suspiciously orderly. Greyface wrote this."
        );
        assert_eq!(assessment(0.40), "Some entropy detected. Keep going.");
        assert_eq!(
            assessment(0.60),
            "Adequately chaotic. The Sacred Chao nods."
        );
        assert_eq!(
            assessment(0.75),
            "A moderately chaotic document. Eris approves."
        );
        assert_eq!(
            assessment(0.95),
            "Pure chaos. The Law of Fives is strong here."
        );
    }
}
