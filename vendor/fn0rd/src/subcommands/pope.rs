use chrono::Local;
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::PopeArgs;
use crate::config::Config;
use crate::date::convert::to_discordian;
use crate::error::FnordError;
use crate::subcommands::util::{current_user, hash_str, hostname, pick, sparkle};
use crate::subcommands::wordlists::{
    PAPAL_DECREES, POPE_ADJECTIVES, POPE_HONORIFICS, POPE_NOUNS, SECT_ADJECTIVES, SECT_NOUNS,
};

pub fn run(
    args: &PopeArgs,
    config: &Config,
    json: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let user = current_user();
    let host = hostname();

    let seed = compute_seed(&user, &host, args.reroll);
    let title = resolve_title(&config.identity.pope_title, seed);
    let sect = resolve_sect(&config.identity.sect_name, seed, &host);

    if args.bull {
        return render_bull(&user, &title, &sect, seed, json, no_color, no_unicode);
    }

    if json {
        let obj = json!({
            "user": user,
            "hostname": host,
            "pope_title": title,
            "sect": sect,
            "short": args.short,
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    if args.short {
        println!("{user}: {title}");
        return Ok(());
    }

    render_declaration(&user, &title, &sect, no_color, no_unicode);
    Ok(())
}

fn compute_seed(user: &str, host: &str, reroll: bool) -> u64 {
    if reroll {
        let ts = Local::now().timestamp_nanos_opt().unwrap_or(0) as u64;
        hash_str(&format!("{user}@{host}:reroll:{ts}"))
    } else {
        hash_str(&format!("{user}@{host}"))
    }
}

/// Generate a papal title from a hash seed using three wordlists.
pub fn generate_title(seed: u64) -> String {
    let hon = pick(POPE_HONORIFICS, seed);
    let adj = pick(
        POPE_ADJECTIVES,
        seed / (POPE_HONORIFICS.len() as u64).max(1),
    );
    let noun = pick(
        POPE_NOUNS,
        seed / ((POPE_HONORIFICS.len() * POPE_ADJECTIVES.len()) as u64).max(1),
    );
    format!("{hon} of the {adj} {noun}")
}

/// Generate a sect name from a hash seed + hostname.
pub fn generate_sect(seed: u64, host: &str) -> String {
    // Mix the seed so the sect name is not just "first adjective/noun of the pope title".
    let sect_seed = hash_str(&format!("sect:{seed}"));
    let adj = pick(SECT_ADJECTIVES, sect_seed);
    let noun = pick(
        SECT_NOUNS,
        sect_seed / (SECT_ADJECTIVES.len() as u64).max(1),
    );
    format!("The {adj} {noun} of {host}")
}

fn resolve_title(config_title: &str, seed: u64) -> String {
    if config_title.is_empty() {
        generate_title(seed)
    } else {
        config_title.to_string()
    }
}

fn resolve_sect(config_sect: &str, seed: u64, host: &str) -> String {
    if config_sect.is_empty() {
        generate_sect(seed, host)
    } else {
        config_sect.to_string()
    }
}

fn render_declaration(user: &str, title: &str, sect: &str, no_color: bool, no_unicode: bool) {
    let sp = sparkle(no_unicode);
    let rule = "═════════════════════════════════════════════════";
    let heading = "PAPAL DECLARATION";

    if no_color {
        println!("{heading}");
        println!("{rule}");
    } else {
        println!("{}", heading.bold().yellow());
        println!("{}", rule.yellow());
    }
    println!();
    println!("By the authority vested in no one in particular,");
    println!();
    println!("{user} is hereby declared:");
    println!();
    if no_color {
        println!("  {title}");
    } else {
        println!("  {}", title.bold());
    }
    println!();
    println!("  Sect: {sect}");
    println!();
    println!("You are a genuine and authorized Pope of Discordia.");
    println!("You have ABSOLUTE AUTHORITY over no one but yourself.");
    println!("Fnord.");
    println!();
    println!("                    {sp} All Hail Eris {sp}");
    println!("                    {sp} All Hail Discordia {sp}");
}

fn render_bull(
    user: &str,
    title: &str,
    sect: &str,
    seed: u64,
    json_out: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let decrees = select_decrees(seed, 5);
    let today = Local::now().date_naive();
    let disc_date = to_discordian(today).to_string();

    if json_out {
        let obj = json!({
            "user": user,
            "pope_title": title,
            "sect": sect,
            "date": disc_date,
            "title_text": "BULLA DISCORDIANA",
            "decrees": decrees,
            "signed": "Eris, Goddess of Chaos (via automated papal bull generator)",
        });
        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return Ok(());
    }

    let border_top = "╔═══════════════════════════════════════════════════════════════════╗";
    let border_bot = "╚═══════════════════════════════════════════════════════════════════╝";
    let border_mid = "╠═══════════════════════════════════════════════════════════════════╣";
    let (t, b, m) = if no_unicode {
        (
            "+-------------------------------------------------------------------+",
            "+-------------------------------------------------------------------+",
            "+-------------------------------------------------------------------+",
        )
    } else {
        (border_top, border_bot, border_mid)
    };

    let sp = sparkle(no_unicode);

    // Top border: ╔ + 67 × ═ + ╗ = 69 chars wide. Our line is
    // "║ " + content + " ║", so content must be 65 chars wide.
    let interior_width = 65usize;
    let print_bordered = |line: &str| {
        let truncated: String = if line.chars().count() > interior_width {
            line.chars().take(interior_width).collect()
        } else {
            line.to_string()
        };
        let pad = interior_width.saturating_sub(truncated.chars().count());
        let side = if no_unicode { "|" } else { "║" };
        println!("{side} {}{} {side}", truncated, " ".repeat(pad));
    };

    if no_color {
        println!("{t}");
    } else {
        println!("{}", t.yellow());
    }
    print_bordered("");
    print_bordered("                      BULLA DISCORDIANA");
    print_bordered(&format!("                    {sp} {sp} {sp} {sp} {sp}"));
    print_bordered("");
    if no_color {
        println!("{m}");
    } else {
        println!("{}", m.yellow());
    }
    print_bordered("");
    print_bordered(&format!("Issued on: {disc_date}"));
    print_bordered("");
    print_bordered("Let it be known throughout the five seasons that the");
    print_bordered("undersigned, in a fit of divine bureaucratic whimsy,");
    print_bordered("does hereby affirm and proclaim:");
    print_bordered("");
    print_bordered(&format!("  {user}"));
    print_bordered("");
    print_bordered("is, was, and shall forever be:");
    print_bordered("");
    print_bordered(&format!("  {title}"));
    print_bordered("");
    print_bordered(&format!("  Sect: {sect}"));
    print_bordered("");
    if no_color {
        println!("{m}");
    } else {
        println!("{}", m.yellow());
    }
    print_bordered("");
    print_bordered("                   PAPAL DECREES");
    print_bordered("");
    for (i, decree) in decrees.iter().enumerate() {
        let prefix = format!("  {}. ", i + 1);
        // The "  N. " prefix takes 5 chars; wrap so prefix+text ≤ 65.
        let wrapped = wrap_text(decree, interior_width - 5);
        for (idx, line) in wrapped.iter().enumerate() {
            if idx == 0 {
                print_bordered(&format!("{prefix}{line}"));
            } else {
                print_bordered(&format!("     {line}"));
            }
        }
        print_bordered("");
    }
    if no_color {
        println!("{m}");
    } else {
        println!("{}", m.yellow());
    }
    print_bordered("");
    print_bordered("Signed,");
    print_bordered("");
    print_bordered("  Eris, Goddess of Chaos");
    print_bordered("  (via automated papal bull generator)");
    print_bordered("");
    if no_color {
        println!("{b}");
    } else {
        println!("{}", b.yellow());
    }

    Ok(())
}

/// Select `n` unique decrees deterministically from the decree list.
fn select_decrees(seed: u64, n: usize) -> Vec<&'static str> {
    let total = PAPAL_DECREES.len();
    let n = n.min(total);
    let mut indices: Vec<usize> = Vec::with_capacity(n);
    let mut s = seed;
    while indices.len() < n {
        s = hash_str(&format!("decree:{s}"));
        let idx = (s as usize) % total;
        if !indices.contains(&idx) {
            indices.push(idx);
        }
    }
    indices.into_iter().map(|i| PAPAL_DECREES[i]).collect()
}

/// Simple word-wrap for papal-bull decree lines.
fn wrap_text(s: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in s.split_whitespace() {
        if current.is_empty() {
            current.push_str(word);
        } else if current.chars().count() + 1 + word.chars().count() <= width {
            current.push(' ');
            current.push_str(word);
        } else {
            lines.push(std::mem::take(&mut current));
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_user_host_same_title() {
        let s1 = compute_seed("eris", "archbox", false);
        let s2 = compute_seed("eris", "archbox", false);
        assert_eq!(s1, s2);
        assert_eq!(generate_title(s1), generate_title(s2));
        assert_eq!(generate_sect(s1, "archbox"), generate_sect(s2, "archbox"));
    }

    #[test]
    fn different_users_different_titles() {
        // Search over a handful of users to make sure generate_title is
        // not a constant function. We don't require every pair to differ,
        // just that there exist two users with different outputs.
        let users = [
            "alice", "bob", "carol", "dave", "eve", "mallory", "peggy", "trent",
        ];
        let titles: Vec<String> = users
            .iter()
            .map(|u| generate_title(compute_seed(u, "host", false)))
            .collect();
        let unique: std::collections::HashSet<&String> = titles.iter().collect();
        assert!(unique.len() > 1, "expected at least 2 distinct titles");
    }

    #[test]
    fn config_override_used_when_set() {
        let seed = compute_seed("eris", "archbox", false);
        let t = resolve_title("Supreme Cabbage", seed);
        assert_eq!(t, "Supreme Cabbage");
        let s = resolve_sect("Custom Sect", seed, "archbox");
        assert_eq!(s, "Custom Sect");
    }

    #[test]
    fn empty_config_generates_title() {
        let seed = compute_seed("eris", "archbox", false);
        let t = resolve_title("", seed);
        assert!(t.contains("of the"));
    }

    #[test]
    fn reroll_produces_a_valid_title() {
        let seed = compute_seed("eris", "archbox", true);
        let title = generate_title(seed);
        assert!(title.contains("of the"));
        assert!(!title.is_empty());
    }

    #[test]
    fn decree_selection_is_deterministic_and_unique() {
        let d1 = select_decrees(12345, 5);
        let d2 = select_decrees(12345, 5);
        assert_eq!(d1, d2);
        assert_eq!(d1.len(), 5);
        let set: std::collections::HashSet<&&str> = d1.iter().collect();
        assert_eq!(set.len(), 5, "decrees should be unique");
    }
}
