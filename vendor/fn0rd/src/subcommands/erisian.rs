//! `fnord erisian` — a Discordian `diff`. Compares two files and reports
//! their differences as a theological dispute between Order (file A) and
//! Chaos (file B).

use owo_colors::OwoColorize;
use serde_json::json;
use similar::{ChangeTag, TextDiff};

use crate::cli::ErisianArgs;
use crate::config::Config;
use crate::error::FnordError;
use crate::subcommands::util::sparkle;

#[derive(Debug, Clone)]
pub struct DiffSummary {
    pub agreements: usize,
    pub order_lines: usize,
    pub chaos_lines: usize,
    pub change_ratio: f64,
    pub disputes: Vec<Dispute>,
}

#[derive(Debug, Clone)]
pub struct Dispute {
    pub line_number: usize,
    pub order_lines: Vec<String>,
    pub chaos_lines: Vec<String>,
}

pub fn run(
    args: &ErisianArgs,
    _config: &Config,
    json_out: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let a = std::fs::read_to_string(&args.file_a)?;
    let b = std::fs::read_to_string(&args.file_b)?;

    let summary = compute_diff(&a, &b);
    let verdict = verdict_for(summary.change_ratio);

    if json_out {
        let disputes: Vec<serde_json::Value> = summary
            .disputes
            .iter()
            .map(|d| {
                json!({
                    "line": d.line_number,
                    "order": d.order_lines,
                    "chaos": d.chaos_lines,
                })
            })
            .collect();
        let obj = json!({
            "file_a": args.file_a.display().to_string(),
            "file_b": args.file_b.display().to_string(),
            "agreements": summary.agreements,
            "order_lines": summary.order_lines,
            "chaos_lines": summary.chaos_lines,
            "change_ratio": summary.change_ratio,
            "verdict": verdict,
            "disputes": disputes,
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    let sp = sparkle(no_unicode);
    let heading = format!("  {sp} ERISIAN THEOLOGICAL DIFF {sp}");
    if no_color {
        println!("{heading}");
    } else {
        println!("{}", heading.bold().magenta());
    }
    println!(
        "  ORDER  ({}) vs CHAOS ({})",
        args.file_a.display(),
        args.file_b.display()
    );
    println!("  ═══════════════════════════════════════════");
    println!();

    if !args.summary {
        render_diff(&a, &b, args.context, no_color);
        println!();
    }

    println!("  ─────────────────────────────────────");
    println!("  THEOLOGICAL SUMMARY:");
    println!();
    println!(
        "  Lines of agreement:  {:>3}  (the Truce of Greyface)",
        summary.agreements
    );
    println!(
        "  ORDER's assertions:  {:>3}  (attempts at structure)",
        summary.order_lines
    );
    println!(
        "  CHAOS's proclamations: {:>3}  (glorious divergence)",
        summary.chaos_lines
    );
    println!();
    let verdict_head = if summary.chaos_lines > summary.order_lines {
        format!(
            "VERDICT: Chaos holds the theological high ground ({} > {}).",
            summary.chaos_lines, summary.order_lines
        )
    } else if summary.order_lines > summary.chaos_lines {
        format!(
            "VERDICT: Order has the upper hand ({} > {}). Suspicious.",
            summary.order_lines, summary.chaos_lines
        )
    } else {
        "VERDICT: A perfect stalemate. The Sacred Chao spins uneasily.".to_string()
    };
    println!("  {verdict_head}");
    println!("           {verdict}");

    Ok(())
}

pub fn compute_diff(a: &str, b: &str) -> DiffSummary {
    let diff = TextDiff::from_lines(a, b);

    let mut agreements = 0usize;
    let mut order_lines = 0usize;
    let mut chaos_lines = 0usize;

    // Collect disputes: group consecutive delete/insert blocks.
    let mut disputes: Vec<Dispute> = Vec::new();
    let mut current: Option<Dispute> = None;
    let mut line_in_a: usize = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                agreements += 1;
                if let Some(d) = current.take() {
                    disputes.push(d);
                }
                line_in_a += 1;
            }
            ChangeTag::Delete => {
                order_lines += 1;
                let text = change.value().trim_end_matches('\n').to_string();
                let d = current.get_or_insert(Dispute {
                    line_number: line_in_a + 1,
                    order_lines: Vec::new(),
                    chaos_lines: Vec::new(),
                });
                d.order_lines.push(text);
                line_in_a += 1;
            }
            ChangeTag::Insert => {
                chaos_lines += 1;
                let text = change.value().trim_end_matches('\n').to_string();
                let d = current.get_or_insert(Dispute {
                    line_number: line_in_a + 1,
                    order_lines: Vec::new(),
                    chaos_lines: Vec::new(),
                });
                d.chaos_lines.push(text);
            }
        }
    }
    if let Some(d) = current.take() {
        disputes.push(d);
    }

    let total = agreements + order_lines + chaos_lines;
    let changed = order_lines + chaos_lines;
    let change_ratio = if total == 0 {
        0.0
    } else {
        changed as f64 / total as f64
    };

    DiffSummary {
        agreements,
        order_lines,
        chaos_lines,
        change_ratio,
        disputes,
    }
}

fn render_diff(a: &str, b: &str, context: usize, no_color: bool) {
    let diff = TextDiff::from_lines(a, b);

    // Grouping the changes with context
    let grouped = diff.grouped_ops(context);
    if grouped.is_empty() {
        println!("  (the texts agree on all things)");
        return;
    }

    for group in grouped {
        for op in group {
            for change in diff.iter_changes(&op) {
                let text = change.value().trim_end_matches('\n');
                match change.tag() {
                    ChangeTag::Equal => {
                        if no_color {
                            println!("    {text}");
                        } else {
                            println!("    {}", text.dimmed());
                        }
                    }
                    ChangeTag::Delete => {
                        let tag = "[ORDER]";
                        if no_color {
                            println!("  {tag} {text}");
                        } else {
                            println!("  {} {}", tag.cyan().bold(), text.cyan());
                        }
                    }
                    ChangeTag::Insert => {
                        let tag = "[CHAOS]";
                        if no_color {
                            println!("  {tag} {text}");
                        } else {
                            println!("  {} {}", tag.magenta().bold(), text.magenta());
                        }
                    }
                }
            }
        }
        println!();
    }
}

pub fn verdict_for(change_ratio: f64) -> &'static str {
    let pct = change_ratio * 100.0;
    if pct == 0.0 {
        "These texts are identical. This is either perfect harmony or suspicious."
    } else if pct <= 10.0 {
        "Minor theological differences. Schism unlikely."
    } else if pct <= 30.0 {
        "Significant disputes. A council of one Pope should be convened."
    } else if pct <= 60.0 {
        "Major schism. Both sides are probably right. And wrong."
    } else if pct <= 90.0 {
        "These texts barely agree. Eris wrote one of them."
    } else {
        "These texts share nothing. One is a hotdog. We're not sure which."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_files_have_no_disputes() {
        let a = "alpha\nbravo\ncharlie\n";
        let b = "alpha\nbravo\ncharlie\n";
        let d = compute_diff(a, b);
        assert_eq!(d.order_lines, 0);
        assert_eq!(d.chaos_lines, 0);
        assert_eq!(d.change_ratio, 0.0);
        assert!(verdict_for(d.change_ratio).contains("identical"));
    }

    #[test]
    fn test_completely_different_files() {
        let a = "one\ntwo\nthree\n";
        let b = "four\nfive\nsix\n";
        let d = compute_diff(a, b);
        assert!(d.order_lines > 0);
        assert!(d.chaos_lines > 0);
        assert!(d.change_ratio > 0.9);
        assert!(verdict_for(d.change_ratio).contains("share nothing"));
    }

    #[test]
    fn test_partial_change() {
        let a = "alpha\nbravo\ncharlie\ndelta\n";
        let b = "alpha\nbravo\nCHARLIE\ndelta\n";
        let d = compute_diff(a, b);
        // 3 agreements, 1 delete, 1 insert
        assert_eq!(d.agreements, 3);
        assert_eq!(d.order_lines, 1);
        assert_eq!(d.chaos_lines, 1);
    }

    #[test]
    fn test_changed_line_count_is_order_plus_chaos() {
        let a = "a\nb\nc\nd\ne\n";
        let b = "a\nZ\nc\nY\ne\n";
        let d = compute_diff(a, b);
        assert_eq!(d.order_lines + d.chaos_lines, 4);
    }
}
