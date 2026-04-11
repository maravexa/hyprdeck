//! `fnord hotdog` — a Discordian `file` command. Determines whether each
//! given file IS or IS NOT a hotdog, with metaphysical justification.

use std::path::PathBuf;

use serde_json::json;

use crate::cli::HotdogArgs;
use crate::config::Config;
use crate::error::FnordError;
use crate::subcommands::util::{hash_str, pick};

const HOTDOG_JUSTIFICATIONS: &[&str] = &[
    "Its essence transcends categorization. It is a hotdog.",
    "The Sacred Chao has reviewed this file and found it satisfactory.",
    "All things, at the quantum level, are hotdogs. This one is especially so.",
    "The Law of Fives confirms: this file is dense with hotdog energy.",
    "Eris blessed this file on the day it was created. Hotdog.",
    "Its extension reveals nothing. Emptiness is hotdog.",
    "The binary oracle has spoken: hotdog.",
    "This file contains multitudes. Hotdogs contain multitudes. QED.",
    "Even a cursory glance reveals the unmistakable aura of hotdog.",
    "Its very name hums with the frequency of hotdog.",
    "The Pineal Gland tingles. This can only mean hotdog.",
];

const NOT_HOTDOG_JUSTIFICATIONS: &[&str] = &[
    "This file has opinions. Hotdogs do not have opinions. Not a hotdog.",
    "Structured data is the enemy of chaos. Not a hotdog.",
    "Greyface created this format. Not a hotdog.",
    "The Law of Fives finds no resonance here. Not a hotdog.",
    "Its very name suggests order. Not a hotdog.",
    "Shebangs are the mark of Cain. Not a hotdog.",
    "The Sacred Chao rejects this file's architecture. Not a hotdog.",
    "The bureaucracy runs strong in this one. Not a hotdog.",
    "This file is suspiciously well-formed. Not a hotdog.",
    "Eris wept upon encountering this file. Not a hotdog.",
    "The primes conspire against this file. It is primly not a hotdog.",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Hotdog,
    NotHotdog,
}

impl Verdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Hotdog => "hotdog",
            Verdict::NotHotdog => "not hotdog",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Classification {
    pub file: String,
    pub verdict: Verdict,
    pub evidence: Vec<String>,
    pub justification: String,
}

pub fn run(args: &HotdogArgs, _config: &Config, json_out: bool) -> Result<(), FnordError> {
    if args.files.is_empty() {
        return Err(FnordError::Parse(
            "hotdog: must specify at least one file".to_string(),
        ));
    }

    let mut classifications: Vec<Classification> = Vec::new();
    for f in &args.files {
        classifications.push(classify(f));
    }

    if json_out {
        let arr: Vec<serde_json::Value> = classifications
            .iter()
            .map(|c| {
                json!({
                    "file": c.file,
                    "verdict": c.verdict.as_str(),
                    "evidence_factors": c.evidence,
                    "justification": c.justification,
                })
            })
            .collect();
        let hotdog_count = classifications
            .iter()
            .filter(|c| c.verdict == Verdict::Hotdog)
            .count();
        let not = classifications.len() - hotdog_count;
        let pct = if classifications.is_empty() {
            0.0
        } else {
            100.0 * hotdog_count as f64 / classifications.len() as f64
        };
        let obj = json!({
            "classifications": arr,
            "summary": {
                "hotdog_count": hotdog_count,
                "non_hotdog_count": not,
                "hotdog_percent": pct,
            }
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    if args.brief {
        for c in &classifications {
            println!("{}: {}", c.file, c.verdict.as_str());
        }
        return Ok(());
    }

    for (i, c) in classifications.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let verdict_text = match c.verdict {
            Verdict::Hotdog => "IS a hotdog",
            Verdict::NotHotdog => "is NOT a hotdog",
        };
        println!("  {}: {}", c.file, verdict_text);
        if !args.no_justify {
            println!("  Justification: {}", c.justification);
        }
    }
    println!();
    println!("  ─────────────────────────────");
    let hotdog_count = classifications
        .iter()
        .filter(|c| c.verdict == Verdict::Hotdog)
        .count();
    let not = classifications.len() - hotdog_count;
    println!(
        "  Verdict: {} hotdog{}, {} non-hotdog{}",
        hotdog_count,
        if hotdog_count == 1 { "" } else { "s" },
        not,
        if not == 1 { "" } else { "s" }
    );
    let pct = 100.0 * hotdog_count as f64 / classifications.len() as f64;
    println!("  The universe is {pct:.1}% hotdog today.");

    Ok(())
}

/// Classify a single file.
pub fn classify(path: &PathBuf) -> Classification {
    let mut hotdog_evidence: Vec<String> = Vec::new();
    let mut not_hotdog_evidence: Vec<String> = Vec::new();

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    // Extension check
    let ext = path.extension().map(|s| s.to_string_lossy().to_lowercase());
    match ext.as_deref() {
        Some("txt") | Some("md") | Some("log") => {}
        Some("rs") | Some("py") | Some("js") | Some("go") => {
            not_hotdog_evidence.push(format!(
                "extension .{} is too structured",
                ext.as_deref().unwrap_or("")
            ));
        }
        Some("toml") | Some("yaml") | Some("yml") | Some("json") => {
            not_hotdog_evidence.push("extension is bureaucratic".to_string());
        }
        Some("jpg") | Some("jpeg") | Some("png") | Some("gif") => {
            hotdog_evidence.push("images contain multitudes".to_string());
        }
        Some("mp3") | Some("wav") | Some("ogg") => {
            hotdog_evidence.push("music is chaotic".to_string());
        }
        Some("exe") | Some("bin") => {
            not_hotdog_evidence.push("binary order".to_string());
        }
        Some("sh") | Some("bash") => {
            // borderline — count as weak hotdog
            hotdog_evidence.push("shell chaos potential exists".to_string());
        }
        None => {
            hotdog_evidence.push("no extension — undefined is hotdog".to_string());
        }
        Some(_) => {}
    }

    // File size law of fives
    if let Ok(meta) = std::fs::metadata(path) {
        let size = meta.len();
        if size > 0 && size % 5 == 0 {
            hotdog_evidence.push(format!("size {size} is divisible by five"));
        }
        if size.to_string().contains('5') {
            hotdog_evidence.push(format!("size {size} contains a five"));
        }
        if is_prime(size) {
            not_hotdog_evidence.push(format!("size {size} is prime — suspiciously orderly"));
        }

        // First byte
        if let Ok(bytes) = std::fs::read(path) {
            if let Some(&first) = bytes.first() {
                if bytes.len() >= 2 && &bytes[..2] == b"#!" {
                    not_hotdog_evidence.push("shebang line: file has opinions".to_string());
                } else if first == b'{' || first == b'[' {
                    not_hotdog_evidence.push("starts with structured-data bracket".to_string());
                } else if first == b'<' {
                    hotdog_evidence.push("starts with markup — confusing = hotdog".to_string());
                } else if !(32..=126).contains(&first) && first != b'\n' && first != b'\t' {
                    hotdog_evidence
                        .push("binary content — beyond comprehension = hotdog".to_string());
                }
            }
        }
    }

    // Filename-based evidence
    let name_lower = file_name.to_lowercase();
    for kw in ["hotdog", "chao", "eris", "fnord", "discord"] {
        if name_lower.contains(kw) {
            hotdog_evidence.push(format!("filename contains '{kw}'"));
        }
    }
    for kw in ["config", "schema", "spec", "test", "lint"] {
        if name_lower.contains(kw) {
            not_hotdog_evidence.push(format!("filename contains '{kw}'"));
        }
    }

    // Verdict: ties go to hotdog.
    let verdict = if hotdog_evidence.len() >= not_hotdog_evidence.len() {
        Verdict::Hotdog
    } else {
        Verdict::NotHotdog
    };

    let seed = hash_str(&format!("{}:{}", file_name, verdict.as_str()));
    let justification = match verdict {
        Verdict::Hotdog => pick(HOTDOG_JUSTIFICATIONS, seed).to_string(),
        Verdict::NotHotdog => pick(NOT_HOTDOG_JUSTIFICATIONS, seed).to_string(),
    };

    let mut all_evidence = hotdog_evidence;
    all_evidence.extend(not_hotdog_evidence);
    if all_evidence.is_empty() {
        all_evidence.push("no distinguishing features".to_string());
    }

    Classification {
        file: path.display().to_string(),
        verdict,
        evidence: all_evidence,
        justification,
    }
}

fn is_prime(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n < 4 {
        return true;
    }
    if n % 2 == 0 {
        return false;
    }
    let mut i: u64 = 3;
    while i.saturating_mul(i) <= n {
        if n % i == 0 {
            return false;
        }
        i += 2;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn classify_name(name: &str) -> Verdict {
        let mut f = tempfile::Builder::new()
            .prefix(name)
            .suffix("")
            .tempfile()
            .unwrap();
        writeln!(f, "hello").unwrap();
        classify(&f.path().to_path_buf()).verdict
    }

    #[test]
    fn test_toml_not_hotdog() {
        let mut f = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
        writeln!(f, "[section]\nkey = \"value\"").unwrap();
        let c = classify(&f.path().to_path_buf());
        assert_eq!(c.verdict, Verdict::NotHotdog);
    }

    #[test]
    fn test_no_extension_is_hotdog() {
        // tempfile::Builder with no suffix gives a random name, possibly no ext.
        // Create a manual temp file with no extension.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("somefile");
        std::fs::write(&path, b"content").unwrap();
        let c = classify(&path);
        assert_eq!(c.verdict, Verdict::Hotdog);
    }

    #[test]
    fn test_filename_contains_fnord_is_hotdog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("my_fnord_file.txt");
        std::fs::write(&path, b"content").unwrap();
        let c = classify(&path);
        assert_eq!(c.verdict, Verdict::Hotdog);
    }

    #[test]
    fn test_filename_contains_config_is_not_hotdog() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.rs");
        std::fs::write(&path, b"fn main() {}").unwrap();
        let c = classify(&path);
        assert_eq!(c.verdict, Verdict::NotHotdog);
    }
}
