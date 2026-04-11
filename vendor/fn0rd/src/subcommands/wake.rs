//! `fnord wake` — a morning dashboard composed of a large ASCII-art
//! Discordian date and up to four info panels (moon, omens, fortune,
//! holyday).

use chrono::Local;
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::WakeArgs;
use crate::config::Config;
use crate::date::convert::to_discordian;
use crate::date::types::{ordinal_suffix, DiscordianDate, Season};
use crate::error::FnordError;
use crate::holydays::defaults::builtin_holydays;
use crate::holydays::registry::HolydayRegistry;
use crate::moon::calc::{illumination_fraction, phase_angle, phase_name_for_angle, Body};
use crate::subcommands::fortune::{collect_fortunes, pick_weighted};
use crate::subcommands::omens::{generative_weather, interpretation_lines};
use crate::subcommands::util::{hash_str, sparkle};
use crate::wake::font::{render, FontStyle};

pub fn run(
    args: &WakeArgs,
    config: &Config,
    json_out: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let today = Local::now().date_naive();
    let disc = to_discordian(today);

    let style = match &args.font {
        Some(s) => FontStyle::parse(s),
        None => FontStyle::Standard,
    };

    let (line1, line2) = date_lines(&disc);

    // Moon panel data
    let moon_panel = if args.no_moon {
        None
    } else {
        let body = Body::resolve(&config.moon.body, today).unwrap_or(Body::Luna);
        let angle = phase_angle(body, today);
        let phase = phase_name_for_angle(angle);
        let illum = (illumination_fraction(angle) * 100.0).round() as i64;
        let glyph = phase.glyph(no_unicode);
        Some(MoonPanel {
            glyph: glyph.to_string(),
            phase_name: phase.label().to_string(),
            body_name: body.display_name().to_string(),
            illum_pct: illum,
        })
    };

    // Holyday panel data
    let registry = HolydayRegistry::build(builtin_holydays(), vec![], vec![]);
    let holydays = registry.lookup(&disc);
    let holyday_panel = holydays.first().map(|h| HolydayPanel {
        name: h.name.clone(),
        description: h.description.clone(),
        season_line: match &disc {
            DiscordianDate::SeasonDay { season, day, .. } => {
                Some(format!("Season of {season}, Day {day}"))
            }
            DiscordianDate::StTibsDay { .. } => None,
        },
    });

    // Omens panel data (generative)
    let omens_panel = if args.omens {
        let reading = generative_weather(today, "The Unmapped");
        let lines = interpretation_lines(&reading);
        let first = lines.first().copied().unwrap_or("The fnords are quiet.");
        Some(OmensPanel {
            description: reading.description.clone(),
            interpretation: first.to_string(),
            confusion_pct: reading.humidity.round() as i64,
            wind_dir: reading.wind_dir.clone(),
        })
    } else {
        None
    };

    // Fortune panel data
    let fortune_panel = if args.fortune {
        pick_a_fortune(config).map(|text| FortunePanel { text })
    } else {
        None
    };

    if json_out {
        return print_json(
            &disc,
            &line1,
            &line2,
            moon_panel.as_ref(),
            holyday_panel.as_ref(),
            omens_panel.as_ref(),
            fortune_panel.as_ref(),
        );
    }

    render_dashboard(
        style,
        no_unicode,
        no_color,
        &line1,
        &line2,
        moon_panel.as_ref(),
        holyday_panel.as_ref(),
        omens_panel.as_ref(),
        fortune_panel.as_ref(),
    );
    Ok(())
}

#[derive(Debug, Clone)]
struct MoonPanel {
    glyph: String,
    phase_name: String,
    body_name: String,
    illum_pct: i64,
}

#[derive(Debug, Clone)]
struct HolydayPanel {
    name: String,
    description: Option<String>,
    season_line: Option<String>,
}

#[derive(Debug, Clone)]
struct OmensPanel {
    description: String,
    interpretation: String,
    confusion_pct: i64,
    wind_dir: String,
}

#[derive(Debug, Clone)]
struct FortunePanel {
    text: String,
}

fn date_lines(disc: &DiscordianDate) -> (String, String) {
    match disc {
        DiscordianDate::SeasonDay {
            year,
            season,
            day,
            weekday,
        } => {
            let suf = ordinal_suffix(*day);
            (
                format!("{weekday}, the {day}{suf}"),
                format!("of {season}, YOLD {year}"),
            )
        }
        DiscordianDate::StTibsDay { year } => ("St. Tib's Day".to_string(), format!("YOLD {year}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_dashboard(
    style: FontStyle,
    no_unicode: bool,
    no_color: bool,
    line1: &str,
    line2: &str,
    moon: Option<&MoonPanel>,
    holyday: Option<&HolydayPanel>,
    omens: Option<&OmensPanel>,
    fortune: Option<&FortunePanel>,
) {
    // ASCII-art date.
    println!();
    for row in render(line1, style, no_unicode) {
        if no_color {
            println!(" {row}");
        } else {
            println!(" {}", row.yellow());
        }
    }
    println!();
    for row in render(line2, style, no_unicode) {
        if no_color {
            println!(" {row}");
        } else {
            println!(" {}", row.yellow());
        }
    }
    println!();

    let divider = if no_unicode {
        "  ========================================"
    } else {
        "  ════════════════════════════════════════"
    };

    let mut printed_any = false;
    if let Some(m) = moon {
        if !printed_any {
            println!("{divider}");
        }
        println!(
            "  {}  {} — {} — {}% illuminated",
            m.glyph, m.phase_name, m.body_name, m.illum_pct
        );
        printed_any = true;
    }

    if let Some(h) = holyday {
        println!("{divider}");
        let star = if no_unicode { "*" } else { "★" };
        if no_color {
            println!("  {star}  TODAY: {}", h.name);
        } else {
            println!("  {}  TODAY: {}", star, h.name.bold().cyan());
        }
        if let Some(line) = &h.season_line {
            println!("     {line}");
        }
        if let Some(desc) = &h.description {
            println!("     {desc}");
        }
        printed_any = true;
    }

    if let Some(o) = omens {
        println!("{divider}");
        let glyph = if no_unicode { "*" } else { "🌧" };
        println!("  {glyph}  {} — {}", o.description, o.interpretation);
        println!(
            "      Confusion Index: {}% — Winds from the {}",
            o.confusion_pct, o.wind_dir
        );
        printed_any = true;
    }

    if let Some(f) = fortune {
        println!("{divider}");
        let sp = sparkle(no_unicode);
        println!("  {sp}  {}", f.text);
        printed_any = true;
    }

    if printed_any {
        println!("{divider}");
    }

    println!();
    let sp = sparkle(no_unicode);
    let footer = format!("                    {sp} All Hail Eris {sp}");
    if no_color {
        println!("{footer}");
    } else {
        println!("{}", footer.bold().magenta());
    }
    println!();
}

fn pick_a_fortune(config: &Config) -> Option<String> {
    let pool = collect_fortunes(config, false).ok()?;
    if pool.is_empty() {
        return None;
    }
    let refs: Vec<&crate::subcommands::fortune::Fortune> = pool.iter().collect();
    let today = Local::now().date_naive();
    let seed = hash_str(&format!("wake-fortune:{today}"));
    let current_season = match to_discordian(today) {
        DiscordianDate::SeasonDay { season, .. } => Some(season),
        DiscordianDate::StTibsDay { .. } => Some(Season::Chaos),
    };
    let f = pick_weighted(&refs, seed, true, false, current_season, None);
    Some(f.text.clone())
}

#[allow(clippy::too_many_arguments)]
fn print_json(
    disc: &DiscordianDate,
    line1: &str,
    line2: &str,
    moon: Option<&MoonPanel>,
    holyday: Option<&HolydayPanel>,
    omens: Option<&OmensPanel>,
    fortune: Option<&FortunePanel>,
) -> Result<(), FnordError> {
    let obj = json!({
        "discordian_date": disc.to_string(),
        "date_lines": {
            "line1": line1,
            "line2": line2,
        },
        "moon": moon.map(|m| json!({
            "phase_name": m.phase_name,
            "body_name": m.body_name,
            "illumination_pct": m.illum_pct,
        })),
        "holyday": holyday.map(|h| json!({
            "name": h.name,
            "description": h.description,
            "season_line": h.season_line,
        })),
        "omens": omens.map(|o| json!({
            "description": o.description,
            "interpretation": o.interpretation,
            "confusion_pct": o.confusion_pct,
            "wind_dir": o.wind_dir,
        })),
        "fortune": fortune.map(|f| json!({
            "text": f.text,
        })),
    });
    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    Ok(())
}
