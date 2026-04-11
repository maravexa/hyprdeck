//! `fnord log` — append timestamped entries to a Discordian grimoire.
//!
//! Supports plaintext, markdown, and org formats; three timestamp styles;
//! optional enrichment with fortune / omens; and a `--list N` reader.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;

use chrono::{Local, NaiveDate};
use serde_json::json;

use crate::cli::LogArgs;
use crate::config::Config;
use crate::date::convert::to_discordian;
use crate::date::types::{ordinal_suffix, DiscordianDate};
use crate::error::FnordError;
use crate::subcommands::fortune::{collect_fortunes, pick_weighted};
use crate::subcommands::omens::{generative_weather, interpretation_lines};
use crate::subcommands::util::{current_user, hash_str};

/// Plaintext entry separator — also used to split entries on read.
pub const PLAINTEXT_SEP: &str = "═══════════════════════════════════════════";

/// Entry format variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Plaintext,
    Markdown,
    Org,
}

impl LogFormat {
    pub fn parse(s: &str) -> Result<Self, FnordError> {
        match s.to_lowercase().as_str() {
            "plaintext" | "plain" | "text" => Ok(LogFormat::Plaintext),
            "markdown" | "md" => Ok(LogFormat::Markdown),
            "org" | "orgmode" => Ok(LogFormat::Org),
            other => Err(FnordError::Parse(format!("unknown log format: '{other}'"))),
        }
    }
}

/// Which timestamp components to include in the entry header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampStyle {
    Discordian,
    Iso8601,
    Both,
}

impl TimestampStyle {
    pub fn parse(s: &str) -> Result<Self, FnordError> {
        match s.to_lowercase().as_str() {
            "discordian" | "disc" => Ok(TimestampStyle::Discordian),
            "iso8601" | "iso" => Ok(TimestampStyle::Iso8601),
            "both" => Ok(TimestampStyle::Both),
            other => Err(FnordError::Parse(format!(
                "unknown timestamp style: '{other}'"
            ))),
        }
    }
}

pub fn run(
    args: &LogArgs,
    config: &Config,
    json_out: bool,
    _no_color: bool,
) -> Result<(), FnordError> {
    let format = match &args.format {
        Some(s) => LogFormat::parse(s)?,
        None => LogFormat::parse(&config.log.format)?,
    };

    let style = match &args.timestamp_style {
        Some(s) => TimestampStyle::parse(s)?,
        None => TimestampStyle::parse(&config.log.timestamp_style)?,
    };

    let path_str = args.file.clone().unwrap_or_else(|| config.log.path.clone());
    let path = expand_tilde(&path_str);

    if let Some(n) = args.list {
        return list_entries(&path, format, n.max(1), json_out);
    }

    // Writing an entry — ensure file exists.
    ensure_file(&path)?;

    let body = resolve_body(args, config)?;
    if body.trim().is_empty() {
        println!("No entry written.");
        return Ok(());
    }

    let now = Local::now();
    let disc = to_discordian(now.date_naive());
    let iso = now.format("%Y-%m-%d %H:%M:%S").to_string();

    let mut full_body = body;
    let append_fortune = args.fortune || config.log.append_fortune;
    let append_omens = args.omens || config.log.append_omens;
    if append_fortune {
        if let Some(f) = pick_a_fortune(config) {
            full_body.push_str("\n\n— ✦ —\n");
            full_body.push_str(&f);
        }
    }
    if append_omens {
        let omens_str = render_omens_line(now.date_naive());
        full_body.push_str("\n\n— ✦ —\n");
        full_body.push_str(&omens_str);
    }

    let rendered = render_entry(format, style, &disc, &iso, &full_body);

    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    f.write_all(rendered.as_bytes())?;

    Ok(())
}

fn resolve_body(args: &LogArgs, config: &Config) -> Result<String, FnordError> {
    if let Some(msg) = &args.message {
        return Ok(msg.clone());
    }
    // Launch editor.
    let editor = if !config.log.editor.is_empty() {
        config.log.editor.clone()
    } else {
        std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string())
    };

    let tmp = std::env::temp_dir().join(format!("fnord-log-{}.txt", std::process::id()));
    // Start empty — if the file is unchanged (empty) after the editor
    // exits we'll treat that as "nothing to write".
    fs::write(&tmp, b"")?;
    let status = StdCommand::new(&editor).arg(&tmp).status();
    match status {
        Ok(s) if s.success() => {}
        Ok(_) | Err(_) => {
            let _ = fs::remove_file(&tmp);
            return Ok(String::new());
        }
    }
    let body = fs::read_to_string(&tmp).unwrap_or_default();
    let _ = fs::remove_file(&tmp);
    Ok(body.trim_end().to_string())
}

fn pick_a_fortune(config: &Config) -> Option<String> {
    let pool = collect_fortunes(config, false).ok()?;
    if pool.is_empty() {
        return None;
    }
    let refs: Vec<&crate::subcommands::fortune::Fortune> = pool.iter().collect();
    let seed = hash_str(&format!(
        "log-fortune:{}",
        Local::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let f = pick_weighted(&refs, seed, false, false, None, None);
    Some(f.text.clone())
}

fn render_omens_line(date: NaiveDate) -> String {
    let reading = generative_weather(date, "The Unmapped");
    let lines = interpretation_lines(&reading);
    let interp = lines.first().copied().unwrap_or("The fnords are quiet.");
    format!("OMENS: {} — {interp}", reading.description)
}

/// Build the full entry text including its header and trailing separator.
pub fn render_entry(
    format: LogFormat,
    style: TimestampStyle,
    disc: &DiscordianDate,
    iso: &str,
    body: &str,
) -> String {
    let disc_line = short_disc(disc);
    match format {
        LogFormat::Plaintext => {
            let mut out = String::new();
            out.push_str(PLAINTEXT_SEP);
            out.push('\n');
            match style {
                TimestampStyle::Discordian => {
                    out.push_str(&disc_line);
                    out.push('\n');
                }
                TimestampStyle::Iso8601 => {
                    out.push_str(iso);
                    out.push('\n');
                }
                TimestampStyle::Both => {
                    out.push_str(&disc_line);
                    out.push('\n');
                    out.push_str(iso);
                    out.push('\n');
                }
            }
            out.push_str(PLAINTEXT_SEP);
            out.push_str("\n\n");
            out.push_str(body.trim_end());
            out.push_str("\n\n");
            out
        }
        LogFormat::Markdown => {
            let mut out = String::new();
            out.push_str("## ");
            out.push_str(&disc_line);
            out.push('\n');
            match style {
                TimestampStyle::Discordian => {}
                TimestampStyle::Iso8601 => {
                    out.push_str(&format!("*{iso}*\n"));
                }
                TimestampStyle::Both => {
                    out.push_str(&format!("*{iso}*\n"));
                }
            }
            out.push('\n');
            out.push_str(body.trim_end());
            out.push_str("\n\n---\n\n");
            out
        }
        LogFormat::Org => {
            let mut out = String::new();
            out.push_str("* ");
            out.push_str(&disc_line);
            out.push('\n');
            out.push_str("  :PROPERTIES:\n");
            match style {
                TimestampStyle::Discordian => {
                    out.push_str(&format!("  :DISCORDIAN: {disc_line}\n"));
                }
                TimestampStyle::Iso8601 => {
                    out.push_str(&format!("  :GREGORIAN: {iso}\n"));
                }
                TimestampStyle::Both => {
                    out.push_str(&format!("  :GREGORIAN: {iso}\n"));
                }
            }
            out.push_str("  :END:\n\n");
            out.push_str(body.trim_end());
            out.push_str("\n\n");
            out
        }
    }
}

/// Display helper: "Pungenday, the 23rd of Confusion, YOLD 3192".
pub fn short_disc(d: &DiscordianDate) -> String {
    match d {
        DiscordianDate::SeasonDay {
            year,
            season,
            day,
            weekday,
        } => {
            let suf = ordinal_suffix(*day);
            format!("{weekday}, the {day}{suf} of {season}, YOLD {year}")
        }
        DiscordianDate::StTibsDay { year } => format!("St. Tib's Day, YOLD {year}"),
    }
}

/// Resolve a leading `~` in `p` to the user's home directory.
pub fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    if p == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(p)
}

/// Create the grimoire file (and its parent directories) if missing,
/// seeding it with a Discordian header comment.
pub fn ensure_file(path: &Path) -> Result<(), FnordError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let user = current_user();
    let disc = to_discordian(Local::now().date_naive());
    let header = format!(
        "# Grimoire of {user}\n# Initiated: {}\n# All Hail Eris\n#\n\n",
        short_disc(&disc)
    );
    fs::write(path, header)?;
    Ok(())
}

/// Parsed grimoire entry used for --list.
#[derive(Debug, Clone)]
pub struct GrimoireEntry {
    pub header: String,
    pub body: String,
}

/// Split a grimoire file into `GrimoireEntry`s in file-order (oldest first).
pub fn parse_entries(content: &str, format: LogFormat) -> Vec<GrimoireEntry> {
    match format {
        LogFormat::Plaintext => parse_plaintext(content),
        LogFormat::Markdown => parse_markdown(content),
        LogFormat::Org => parse_org(content),
    }
}

fn parse_plaintext(content: &str) -> Vec<GrimoireEntry> {
    let mut out: Vec<GrimoireEntry> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == PLAINTEXT_SEP {
            // Header is the lines between this separator and the next.
            let mut header_lines: Vec<&str> = Vec::new();
            i += 1;
            while i < lines.len() && lines[i].trim() != PLAINTEXT_SEP {
                header_lines.push(lines[i]);
                i += 1;
            }
            // Skip the closing separator.
            if i < lines.len() {
                i += 1;
            }
            // Body is everything until the next opening separator.
            let mut body_lines: Vec<&str> = Vec::new();
            while i < lines.len() && lines[i].trim() != PLAINTEXT_SEP {
                body_lines.push(lines[i]);
                i += 1;
            }
            // Trim leading/trailing blank lines from the body.
            let header = header_lines.join("\n").trim().to_string();
            let body = trim_blank_edges(&body_lines).join("\n");
            if !header.is_empty() {
                out.push(GrimoireEntry { header, body });
            }
        } else {
            i += 1;
        }
    }
    out
}

fn parse_markdown(content: &str) -> Vec<GrimoireEntry> {
    let mut out: Vec<GrimoireEntry> = Vec::new();
    let mut header: Option<String> = None;
    let mut body: Vec<&str> = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            if let Some(h) = header.take() {
                let b = trim_blank_edges(&body)
                    .into_iter()
                    .take_while(|l| l.trim() != "---")
                    .collect::<Vec<_>>()
                    .join("\n");
                out.push(GrimoireEntry { header: h, body: b });
            }
            header = Some(rest.trim().to_string());
            body.clear();
        } else if header.is_some() {
            body.push(line);
        }
    }
    if let Some(h) = header.take() {
        let b = trim_blank_edges(&body)
            .into_iter()
            .take_while(|l| l.trim() != "---")
            .collect::<Vec<_>>()
            .join("\n");
        out.push(GrimoireEntry { header: h, body: b });
    }
    out
}

fn parse_org(content: &str) -> Vec<GrimoireEntry> {
    let mut out: Vec<GrimoireEntry> = Vec::new();
    let mut header: Option<String> = None;
    let mut body: Vec<&str> = Vec::new();
    let mut in_props = false;
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("* ") {
            if let Some(h) = header.take() {
                let b = trim_blank_edges(&body).join("\n");
                out.push(GrimoireEntry { header: h, body: b });
            }
            header = Some(rest.trim().to_string());
            body.clear();
            in_props = false;
        } else if header.is_some() {
            let trimmed = line.trim();
            if trimmed == ":PROPERTIES:" {
                in_props = true;
                continue;
            }
            if trimmed == ":END:" {
                in_props = false;
                continue;
            }
            if in_props {
                continue;
            }
            body.push(line);
        }
    }
    if let Some(h) = header.take() {
        let b = trim_blank_edges(&body).join("\n");
        out.push(GrimoireEntry { header: h, body: b });
    }
    out
}

fn trim_blank_edges<'a>(lines: &[&'a str]) -> Vec<&'a str> {
    let mut start = 0;
    while start < lines.len() && lines[start].trim().is_empty() {
        start += 1;
    }
    let mut end = lines.len();
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }
    lines[start..end].to_vec()
}

fn list_entries(
    path: &Path,
    format: LogFormat,
    n: usize,
    json_out: bool,
) -> Result<(), FnordError> {
    if !path.exists() {
        if json_out {
            println!("[]");
        } else {
            println!("(no grimoire at {})", path.display());
        }
        return Ok(());
    }
    let content = fs::read_to_string(path)?;
    let entries = parse_entries(&content, format);
    let mut rev: Vec<&GrimoireEntry> = entries.iter().rev().collect();
    rev.truncate(n);

    if json_out {
        let arr: Vec<serde_json::Value> = rev
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let idx = entries.len() - i; // 1-based original index, newest first
                let (disc_date, greg_date) = split_header(&e.header);
                json!({
                    "index": idx,
                    "discordian_date": disc_date,
                    "gregorian_date": greg_date,
                    "body": e.body,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap());
        return Ok(());
    }

    for (i, e) in rev.iter().enumerate() {
        let idx = entries.len() - i;
        let body_preview = truncate_lines(&e.body, 2);
        println!("[{idx}] {}", e.header);
        for line in body_preview.lines() {
            println!("    {line}");
        }
        println!();
    }
    Ok(())
}

/// Split a combined-style header into (discordian, gregorian) when possible.
fn split_header(header: &str) -> (String, Option<String>) {
    let lines: Vec<&str> = header.lines().collect();
    if lines.len() >= 2 {
        (
            lines[0].trim().to_string(),
            Some(lines[1].trim().to_string()),
        )
    } else {
        (header.trim().to_string(), None)
    }
}

fn truncate_lines(body: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() <= max_lines {
        body.to_string()
    } else {
        let mut out: Vec<&str> = lines.iter().take(max_lines).copied().collect();
        out.push("...");
        out.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::types::{Season, Weekday};
    use tempfile::tempdir;

    fn sample_disc() -> DiscordianDate {
        DiscordianDate::SeasonDay {
            year: 3192,
            season: Season::Confusion,
            day: 23,
            weekday: Weekday::Pungenday,
        }
    }

    #[test]
    fn plaintext_entry_has_header_and_separator() {
        let rendered = render_entry(
            LogFormat::Plaintext,
            TimestampStyle::Both,
            &sample_disc(),
            "2026-04-09 14:32:07",
            "Hello grimoire.",
        );
        assert!(rendered.contains(PLAINTEXT_SEP));
        assert!(rendered.contains("Pungenday, the 23rd of Confusion, YOLD 3192"));
        assert!(rendered.contains("2026-04-09 14:32:07"));
        assert!(rendered.contains("Hello grimoire."));
    }

    #[test]
    fn markdown_entry_uses_h2_headers() {
        let rendered = render_entry(
            LogFormat::Markdown,
            TimestampStyle::Both,
            &sample_disc(),
            "2026-04-09 14:32:07",
            "note.",
        );
        assert!(rendered.starts_with("## "));
        assert!(rendered.contains("---"));
    }

    #[test]
    fn org_entry_uses_star_headers() {
        let rendered = render_entry(
            LogFormat::Org,
            TimestampStyle::Iso8601,
            &sample_disc(),
            "2026-04-09 14:32:07",
            "note.",
        );
        assert!(rendered.starts_with("* "));
        assert!(rendered.contains(":PROPERTIES:"));
        assert!(rendered.contains(":GREGORIAN:"));
    }

    #[test]
    fn plaintext_list_parses_last_n_entries() {
        let a = render_entry(
            LogFormat::Plaintext,
            TimestampStyle::Both,
            &sample_disc(),
            "2026-04-09 00:00:00",
            "first entry body",
        );
        let b = render_entry(
            LogFormat::Plaintext,
            TimestampStyle::Both,
            &sample_disc(),
            "2026-04-10 00:00:00",
            "second entry body",
        );
        let combined = format!("{a}{b}");
        let entries = parse_entries(&combined, LogFormat::Plaintext);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].body.contains("first entry body"));
        assert!(entries[1].body.contains("second entry body"));
    }

    #[test]
    fn markdown_list_parses_entries() {
        let a = render_entry(
            LogFormat::Markdown,
            TimestampStyle::Both,
            &sample_disc(),
            "2026-04-09 00:00:00",
            "one",
        );
        let b = render_entry(
            LogFormat::Markdown,
            TimestampStyle::Both,
            &sample_disc(),
            "2026-04-10 00:00:00",
            "two",
        );
        let combined = format!("{a}{b}");
        let entries = parse_entries(&combined, LogFormat::Markdown);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].body.contains("one"));
        assert!(entries[1].body.contains("two"));
    }

    #[test]
    fn org_list_parses_entries() {
        let a = render_entry(
            LogFormat::Org,
            TimestampStyle::Iso8601,
            &sample_disc(),
            "2026-04-09 00:00:00",
            "first org",
        );
        let b = render_entry(
            LogFormat::Org,
            TimestampStyle::Iso8601,
            &sample_disc(),
            "2026-04-10 00:00:00",
            "second org",
        );
        let combined = format!("{a}{b}");
        let entries = parse_entries(&combined, LogFormat::Org);
        assert_eq!(entries.len(), 2);
        assert!(entries[0].body.contains("first org"));
        assert!(entries[1].body.contains("second org"));
    }

    #[test]
    fn ensure_file_creates_grimoire_with_header() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("sub/dir/grimoire");
        ensure_file(&path).unwrap();
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# Grimoire of "));
        assert!(content.contains("All Hail Eris"));
    }

    #[test]
    fn expand_tilde_resolves_home() {
        let p = expand_tilde("~/foo/bar");
        let home = dirs::home_dir().unwrap();
        assert_eq!(p, home.join("foo/bar"));
    }

    #[test]
    fn expand_tilde_passthrough_absolute() {
        let p = expand_tilde("/tmp/foo");
        assert_eq!(p, PathBuf::from("/tmp/foo"));
    }
}
