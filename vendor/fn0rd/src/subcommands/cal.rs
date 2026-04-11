use chrono::Local;
use owo_colors::OwoColorize;

use crate::cli::CalArgs;
use crate::config::Config;
use crate::date::convert::to_discordian;
use crate::date::types::{DiscordianDate, Season, Weekday};
use crate::error::FnordError;
use crate::holydays::defaults::builtin_holydays;
use crate::holydays::registry::HolydayRegistry;

const WEEKDAY_ABBREVS: [&str; 5] = ["Swt", "Boo", "Pun", "PP", "SO"];
const COL_WIDTH: usize = 4; // " NNN" per cell

pub fn run(args: &CalArgs, _config: &Config, no_color: bool) -> Result<(), FnordError> {
    let today_naive = Local::now().date_naive();
    let today_disc = to_discordian(today_naive);

    let (today_year, today_season, today_day) = match &today_disc {
        DiscordianDate::SeasonDay {
            year, season, day, ..
        } => (*year, Some(*season), Some(*day)),
        DiscordianDate::StTibsDay { year } => (*year, None, None),
    };

    // Determine which year and seasons to render
    let target_year = args.year.unwrap_or(today_year);

    let registry = HolydayRegistry::build(builtin_holydays(), vec![], vec![]);

    if args.all {
        for season in Season::all() {
            render_season(
                season,
                target_year,
                today_year,
                today_season,
                today_day,
                &registry,
                no_color,
            );
            println!();
        }
    } else {
        let target_season = if let Some(name) = &args.season {
            parse_season(name)?
        } else {
            // Default to current season; if St. Tib's Day, use Chaos
            today_season.unwrap_or(Season::Chaos)
        };
        render_season(
            target_season,
            target_year,
            today_year,
            today_season,
            today_day,
            &registry,
            no_color,
        );
    }

    Ok(())
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

fn render_season(
    season: Season,
    target_year: i32,
    today_year: i32,
    today_season: Option<Season>,
    today_day: Option<u8>,
    registry: &HolydayRegistry,
    no_color: bool,
) {
    // Header
    let apostle = season.apostle();
    println!("  {} {}  (Apostle: {})", season, target_year, apostle);
    println!();

    // Weekday headers
    print!("  ");
    for abbrev in &WEEKDAY_ABBREVS {
        print!("{abbrev:>COL_WIDTH$}");
    }
    println!();

    // Separator
    println!("  {}", "-".repeat(COL_WIDTH * 5));

    // 73 days, 5 per week = 15 rows (last row has 3 days)
    let is_current_season = today_season == Some(season) && today_year == target_year;

    // Holyday legend tracking
    let mut holyday_legend: Vec<(u8, String, char)> = vec![];
    let symbols: Vec<char> = vec!['*', '+', '#', '@', '!', '~', '^', '&'];

    for week in 0..15_u8 {
        print!("  ");
        for wd in 0..5_u8 {
            let day = week * 5 + wd + 1;
            if day > 73 {
                print!("{:COL_WIDTH$}", "");
                continue;
            }

            // Check if this day is a holyday
            let symbol: Option<char> = {
                let fake_date = DiscordianDate::SeasonDay {
                    year: target_year,
                    season,
                    day,
                    weekday: Weekday::from_day_of_season(day),
                };
                let holydays = registry.lookup(&fake_date);
                if !holydays.is_empty() {
                    let existing = holyday_legend
                        .iter()
                        .find(|(d, _, _)| *d == day)
                        .map(|(_, _, s)| *s);
                    if let Some(s) = existing {
                        Some(s)
                    } else {
                        let sym = symbols.get(holyday_legend.len()).copied().unwrap_or('?');
                        holyday_legend.push((day, holydays[0].name.clone(), sym));
                        Some(sym)
                    }
                } else {
                    None
                }
            };

            let is_today = is_current_season && today_day == Some(day);
            let cell = format!("{day:>3}{}", symbol.unwrap_or(' '));

            if no_color {
                print!("{cell}");
            } else if is_today {
                print!("{}", cell.bold().underline());
            } else if symbol.is_some() {
                print!("{}", cell.yellow());
            } else {
                print!("{cell}");
            }
        }
        println!();
    }

    // Legend
    if !holyday_legend.is_empty() {
        println!();
        println!("  Holydays:");
        for (day, name, sym) in &holyday_legend {
            println!("    {sym} day {day}: {name}");
        }
    }
}
