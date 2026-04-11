use chrono::Local;
use serde_json::json;
use std::path::PathBuf;

use crate::cli::{HolydayAction, HolydayArgs};
use crate::config::Config;
use crate::date::convert::{parse_date_arg, to_discordian};
use crate::date::types::{DiscordianDate, Season};
use crate::error::FnordError;
use crate::holydays::defaults::builtin_holydays;
use crate::holydays::registry::HolydayRegistry;
use crate::holydays::types::{Holyday, HolydayKey, HolydaySource};

pub fn run(
    args: &HolydayArgs,
    _config: &Config,
    _json: bool,
    _no_color: bool,
) -> Result<(), FnordError> {
    let registry = HolydayRegistry::build(builtin_holydays(), vec![], load_personal()?);

    match &args.action {
        None | Some(HolydayAction::List(_)) => {
            let list_args = match &args.action {
                Some(HolydayAction::List(a)) => a,
                _ => &crate::cli::HolydayListArgs::default(),
            };
            run_list(&registry, list_args)
        }
        Some(HolydayAction::Show(a)) => run_show(&registry, a),
        Some(HolydayAction::Add(a)) => run_add(a),
        Some(HolydayAction::Remove(a)) => run_remove(a),
    }
}

// ─── list ─────────────────────────────────────────────────────────────────────

fn run_list(
    registry: &HolydayRegistry,
    args: &crate::cli::HolydayListArgs,
) -> Result<(), FnordError> {
    let all_builtins = builtin_holydays();
    let personal = load_personal()?;
    let combined_registry = HolydayRegistry::build(all_builtins.clone(), vec![], personal.clone());

    // Build merged list: builtins + personal extras
    let mut items: Vec<(HolydayKey, &Holyday, HolydaySource)> = vec![];

    // We iterate through all known keys from builtins + personal
    let mut seen_keys = std::collections::HashSet::new();

    for h in &all_builtins {
        seen_keys.insert(h.key.clone());
    }
    for h in &personal {
        seen_keys.insert(h.key.clone());
    }

    // Gather from registry (which has merged overrides)
    for key in &seen_keys {
        let fake_date = key_to_date(key);
        let results = combined_registry.lookup(&fake_date);
        if let Some(h) = results.first() {
            items.push((key.clone(), h, h.source.clone()));
        }
    }

    // Filter by season if requested
    if let Some(season_filter) = &args.season {
        let season = parse_season(season_filter)?;
        items.retain(
            |(key, _, _)| matches!(key, HolydayKey::SeasonDay { season: s, .. } if *s == season),
        );
    }

    if args.json {
        let arr: Vec<serde_json::Value> = items
            .iter()
            .map(|(key, h, src)| {
                json!({
                    "key": key_to_string(key),
                    "name": h.name,
                    "description": h.description,
                    "source": source_tag(src),
                    "recurring": h.recurring,
                    "year": h.year,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap());
        return Ok(());
    }

    // Group by season for display
    println!();
    println!("  DISCORDIAN HOLY DAYS");
    println!("  {}", "═".repeat(54));

    for season in Season::all() {
        let season_items: Vec<_> = items
            .iter()
            .filter(|(key, _, _)| {
                matches!(key, HolydayKey::SeasonDay { season: s, .. } if *s == season)
            })
            .collect();

        if season_items.is_empty() {
            continue;
        }

        println!();
        println!("  SEASON OF {}", season.to_string().to_uppercase());
        println!("  {}", "─".repeat(53));

        let mut sorted = season_items;
        sorted.sort_by_key(|(key, _, _)| {
            if let HolydayKey::SeasonDay { day, .. } = key {
                *day
            } else {
                0
            }
        });

        for (key, h, src) in sorted {
            let key_str = key_to_string(key);
            let src_tag = if args.show_source {
                format!("  [{}]", source_tag(src))
            } else {
                String::new()
            };
            let desc = h.description.as_deref().unwrap_or("");
            println!("  {:<12}  {:<20}  {:<38}{}", key_str, h.name, desc, src_tag);
        }
    }

    // St. Tib's Day
    let tibs: Vec<_> = items
        .iter()
        .filter(|(key, _, _)| matches!(key, HolydayKey::StTibs))
        .collect();
    if !tibs.is_empty() {
        println!();
        println!("  ST. TIB'S DAY  (leap years only)");
        println!("  {}", "─".repeat(53));
        for (key, h, src) in tibs {
            let src_tag = if args.show_source {
                format!("  [{}]", source_tag(src))
            } else {
                String::new()
            };
            let desc = h.description.as_deref().unwrap_or("");
            println!(
                "  {:<12}  {:<20}  {:<38}{}",
                key_to_string(key),
                h.name,
                desc,
                src_tag
            );
        }
    }

    println!();
    let default_count = items
        .iter()
        .filter(|(_, _, s)| matches!(s, HolydaySource::Default))
        .count();
    let cabal_count = items
        .iter()
        .filter(|(_, _, s)| matches!(s, HolydaySource::Cabal))
        .count();
    let personal_count = items
        .iter()
        .filter(|(_, _, s)| matches!(s, HolydaySource::Personal))
        .count();
    let total = items.len();
    print!("  {}", "─".repeat(29));
    println!();
    print!(
        "  Total: {total} holyday{}",
        if total == 1 { "" } else { "s" }
    );
    if default_count > 0 {
        print!(" ({default_count} default");
        if cabal_count > 0 {
            print!(", {cabal_count} cabal");
        }
        if personal_count > 0 {
            print!(", {personal_count} personal");
        }
        print!(")");
    }
    println!();
    println!();

    let _ = registry;
    Ok(())
}

// ─── show ─────────────────────────────────────────────────────────────────────

fn run_show(
    registry: &HolydayRegistry,
    args: &crate::cli::HolydayShowArgs,
) -> Result<(), FnordError> {
    let naive_date = match &args.date {
        Some(s) => parse_date_arg(s)?,
        None => Local::now().date_naive(),
    };
    let disc = to_discordian(naive_date);
    let holydays = registry.lookup(&disc);

    println!();
    println!("  {disc}");
    println!();

    if holydays.is_empty() {
        println!("  No holydays today.");

        // Find next holyday
        if let Some((days_until, next_disc, next_h)) = find_next_holyday(&disc, registry) {
            println!(
                "  Next holyday: {} in {} day{} ({})",
                next_h.name,
                days_until,
                if days_until == 1 { "" } else { "s" },
                next_disc
            );
        }
    } else {
        for h in &holydays {
            println!("  ★ TODAY: {} ★", h.name);
            if let Some(desc) = &h.description {
                println!("  {desc}");
            }
            if let Some(greeting) = &h.greeting {
                println!();
                println!("  {greeting}");
            }
        }
        println!();
        println!("  Hail Eris. Today is sacred. Act accordingly (or don't).");
    }

    println!();
    Ok(())
}

// ─── add ──────────────────────────────────────────────────────────────────────

fn run_add(args: &crate::cli::HolydayAddArgs) -> Result<(), FnordError> {
    // Validate
    let key = HolydayKey::parse(&args.key)?;

    if args.once && args.year.is_none() {
        return Err(FnordError::Parse(
            "--year is required when --once is set".to_string(),
        ));
    }

    let path = personal_holyday_path();

    // Load existing
    let mut file_content = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };

    // Check for existing entry with same key
    let key_str = key_to_string(&key);
    if file_content.contains(&format!("date = \"{key_str}\"")) {
        // Prompt for confirmation
        eprint!("  A holyday already exists for {key_str}. Overwrite? [y/N] ");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap_or_default();
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("  Cancelled.");
            return Ok(());
        }
        // Remove existing entry by stripping the block
        file_content = remove_entry_from_toml(&file_content, &key_str);
    }

    // Append new entry
    if !path.parent().map(|p| p.exists()).unwrap_or(true) {
        std::fs::create_dir_all(path.parent().unwrap())?;
    }

    let recurring = !args.once;
    let mut entry = format!(
        "\n[[holyday]]\nname = {:?}\ndate = {:?}\n",
        args.name, key_str
    );
    if let Some(desc) = &args.description {
        entry.push_str(&format!("description = {:?}\n", desc));
    }
    entry.push_str(&format!("recurring = {recurring}\n"));
    if let Some(year) = args.year {
        entry.push_str(&format!("year = {year}\n"));
    }

    file_content.push_str(&entry);
    std::fs::write(&path, &file_content)?;

    println!();
    println!("  ✦ Holyday added to {}", path.display());
    println!("  {}: {:?}", key_str, args.name);
    println!("  Eris has been notified. She doesn't care, but she's been notified.");
    println!();
    Ok(())
}

// ─── remove ───────────────────────────────────────────────────────────────────

fn run_remove(args: &crate::cli::HolydayRemoveArgs) -> Result<(), FnordError> {
    let key = HolydayKey::parse(&args.key)?;
    let key_str = key_to_string(&key);

    let path = personal_holyday_path();
    if !path.exists() {
        return Err(FnordError::Parse(format!(
            "no personal holyday file found; cannot remove '{key_str}'"
        )));
    }

    let content = std::fs::read_to_string(&path)?;

    // Check it exists in personal file
    let personal = load_personal()?;
    let entry = personal.iter().find(|h| key_to_string(&h.key) == key_str);
    let holyday_name = match entry {
        Some(h) => h.name.clone(),
        None => {
            return Err(FnordError::Parse(format!(
                "holyday '{key_str}' not found in personal file (cannot remove default or cabal holydays)"
            )));
        }
    };

    // Prompt for confirmation
    eprint!("  Remove holyday {key_str} {:?}? [y/N] ", holyday_name);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap_or_default();
    if !input.trim().eq_ignore_ascii_case("y") {
        println!("  Cancelled.");
        return Ok(());
    }

    let new_content = remove_entry_from_toml(&content, &key_str);
    std::fs::write(&path, &new_content)?;

    println!();
    println!("  ✦ Holyday removed: {key_str} {:?}", holyday_name);
    println!("  The universe has been updated. Or has it?");
    println!();
    Ok(())
}

// ─── helpers ──────────────────────────────────────────────────────────────────

fn personal_holyday_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("eris")
        .join("holydays")
        .join("personal.toml")
}

fn load_personal() -> Result<Vec<Holyday>, FnordError> {
    let path = personal_holyday_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)?;
    let file: crate::holydays::types::PersonalHolydayFile =
        toml::from_str(&content).map_err(|e| FnordError::Parse(e.to_string()))?;
    let mut holydays = vec![];
    for entry in file.holyday {
        match entry.into_holyday(HolydaySource::Personal) {
            Ok(h) => holydays.push(h),
            Err(e) => eprintln!("fnord: warning: skipping malformed personal holyday: {e}"),
        }
    }
    Ok(holydays)
}

fn key_to_string(key: &HolydayKey) -> String {
    match key {
        HolydayKey::StTibs => "st-tibs".to_string(),
        HolydayKey::SeasonDay { season, day } => {
            format!("{}-{}", season.to_string().to_lowercase(), day)
        }
    }
}

fn key_to_date(key: &HolydayKey) -> DiscordianDate {
    match key {
        HolydayKey::StTibs => DiscordianDate::StTibsDay { year: 0 },
        HolydayKey::SeasonDay { season, day } => DiscordianDate::SeasonDay {
            year: 0,
            season: *season,
            day: *day,
            weekday: crate::date::types::Weekday::from_day_of_season(*day),
        },
    }
}

fn source_tag(src: &HolydaySource) -> &'static str {
    match src {
        HolydaySource::Default => "default",
        HolydaySource::Cabal => "cabal",
        HolydaySource::Personal => "personal",
    }
}

fn parse_season(s: &str) -> Result<Season, FnordError> {
    match s.to_lowercase().as_str() {
        "chaos" => Ok(Season::Chaos),
        "discord" => Ok(Season::Discord),
        "confusion" => Ok(Season::Confusion),
        "bureaucracy" => Ok(Season::Bureaucracy),
        "aftermath" => Ok(Season::Aftermath),
        other => Err(FnordError::Parse(format!("unknown season: '{other}'"))),
    }
}

/// Naive TOML block remover: strip the [[holyday]] block matching the given date key.
fn remove_entry_from_toml(content: &str, key_str: &str) -> String {
    let needle = format!("date = \"{key_str}\"");
    let mut result = String::with_capacity(content.len());
    let mut lines = content.lines().peekable();
    let mut in_block = false;

    while let Some(line) = lines.next() {
        if line.trim() == "[[holyday]]" {
            // Peek ahead to see if this block contains our key
            // We need to collect the block and decide
            let mut block_lines: Vec<&str> = vec![line];
            while let Some(&next) = lines.peek() {
                if next.trim().starts_with("[[") && next.trim() != "[[holyday]]" {
                    break;
                }
                if next.trim() == "[[holyday]]" {
                    break;
                }
                if next.trim().is_empty() && block_lines.len() > 1 {
                    block_lines.push(lines.next().unwrap());
                    break;
                }
                block_lines.push(lines.next().unwrap());
            }
            let block_str = block_lines.join("\n");
            if !block_str.contains(&needle) {
                result.push_str(&block_str);
                result.push('\n');
            }
            in_block = false;
        } else if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}

/// Find the next holyday after the given date (scans up to 366 days ahead).
fn find_next_holyday<'a>(
    current: &DiscordianDate,
    registry: &'a HolydayRegistry,
) -> Option<(u32, DiscordianDate, &'a crate::holydays::types::Holyday)> {
    use crate::date::convert::to_discordian;

    // Convert current Discordian date back to approximate Gregorian for iteration
    // We'll just iterate forward from today
    let today = Local::now().date_naive();
    for days_ahead in 1u32..=366 {
        let candidate = today + chrono::Duration::days(days_ahead as i64);
        let disc = to_discordian(candidate);
        let holydays = registry.lookup(&disc);
        if !holydays.is_empty() {
            return Some((days_ahead, disc, holydays[0]));
        }
    }
    let _ = current;
    None
}
