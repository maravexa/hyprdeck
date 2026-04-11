use chrono::{Duration, Local, NaiveDate};
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::MoonArgs;
use crate::config::Config;
use crate::date::convert::{parse_date_arg, to_discordian};
use crate::date::types::{DiscordianDate, Season};
use crate::error::FnordError;
use crate::moon::calc::{
    days_to_full, days_to_new, illumination_fraction, phase_angle, phase_name_for_angle, Body,
    PhaseName,
};

pub fn run(
    args: &MoonArgs,
    config: &Config,
    json: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let target_date = match &args.date {
        Some(s) => parse_date_arg(s)?,
        None => Local::now().date_naive(),
    };

    // Determine the body: --body flag > config.moon.body.
    let body_name = args
        .body
        .clone()
        .unwrap_or_else(|| config.moon.body.clone());
    let body = Body::resolve(&body_name, target_date)
        .ok_or_else(|| FnordError::Parse(format!("unknown body: '{body_name}'")))?;

    let angle = phase_angle(body, target_date);
    let phase = phase_name_for_angle(angle);
    let illum = illumination_fraction(angle);
    let illum_pct = (illum * 100.0).round() as i64;

    if args.season {
        return render_season(body, target_date, no_color, no_unicode);
    }

    if json {
        return print_json(body, target_date, angle, phase, illum_pct);
    }

    render_default(
        body,
        target_date,
        angle,
        phase,
        illum_pct,
        no_color,
        no_unicode,
    );

    if args.next {
        render_next(body, target_date, angle, no_color);
    }

    Ok(())
}

fn render_default(
    body: Body,
    target_date: NaiveDate,
    _angle: f64,
    phase: PhaseName,
    illum_pct: i64,
    no_color: bool,
    no_unicode: bool,
) {
    let glyph = phase.glyph(no_unicode);
    let label = phase.label();
    let disc = to_discordian(target_date);

    println!();
    if no_color {
        println!("  {glyph}  {label}");
    } else {
        println!("  {glyph}  {}", label.bold());
    }

    match body.parent_note() {
        Some(note) => {
            println!(
                "  {} ({}) — {}% illuminated",
                body.display_name(),
                note,
                illum_pct
            );
            let period = body.orbital_period();
            if period < 1.0 {
                let hours = period * 24.0;
                println!("  Orbital period: {period:.3} days (~{hours:.1} hours)");
            } else {
                println!("  Orbital period: {period:.3} days");
            }
        }
        None => {
            println!("  {} — {}% illuminated", body.display_name(), illum_pct);
        }
    }

    println!("  {}", short_discordian(&disc));
}

fn render_next(body: Body, target_date: NaiveDate, angle: f64, _no_color: bool) {
    let period = body.orbital_period();
    let d_full = days_to_full(angle, period);
    let d_new = days_to_new(angle, period);

    let full_date = target_date + Duration::days(d_full.round() as i64);
    let new_date = target_date + Duration::days(d_new.round() as i64);

    let full_disc = to_discordian(full_date);
    let new_disc = to_discordian(new_date);

    println!(
        "  Next full moon: in {:.1} days ({})",
        d_full,
        short_discordian_terse(&full_disc)
    );
    println!(
        "  Next new moon:  in {:.1} days ({})",
        d_new,
        short_discordian_terse(&new_disc)
    );
}

fn render_season(
    body: Body,
    target_date: NaiveDate,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let disc = to_discordian(target_date);
    let season = match &disc {
        DiscordianDate::SeasonDay { season, .. } => *season,
        DiscordianDate::StTibsDay { .. } => Season::Chaos,
    };

    let header = format!("  {} — Season of {}", body.display_name(), season);
    println!();
    if no_color {
        println!("{header}");
    } else {
        println!("{}", header.bold().cyan());
    }
    println!();

    // Find the first day of the season in Gregorian space by walking back
    // from target_date until we hit day 1 of the same season (or any day of
    // an adjacent season — whichever comes first).
    let first_of_season = gregorian_for_season_day(target_date, season, 1);

    // Render one row per 5 days (Discordian week), each row covering
    // days (start, start+5, start+10, ...) of the season, for 73 days total.
    for week_start in (1u8..=73).step_by(5) {
        let mut cells: Vec<String> = Vec::with_capacity(5);
        for d in week_start..week_start.saturating_add(5) {
            if d > 73 {
                break;
            }
            let g = first_of_season + Duration::days((d - 1) as i64);
            let angle = phase_angle(body, g);
            let phase = phase_name_for_angle(angle);
            let glyph = phase.glyph(no_unicode);
            cells.push(format!("{d:>2} {glyph} {:<16}", phase.label()));
        }
        println!("  {}", cells.join("  "));
    }

    Ok(())
}

/// Find the Gregorian date that corresponds to day `target_day` of
/// `target_season` in the same Discordian year as `reference`.
fn gregorian_for_season_day(
    reference: NaiveDate,
    target_season: Season,
    target_day: u8,
) -> NaiveDate {
    // Walk the calendar within ±400 days of `reference` — enough to cover a
    // full year in either direction. This is a simple, correct approach that
    // handles leap years and season boundaries without re-deriving the map.
    for offset in -400i64..=400 {
        let cand = reference + Duration::days(offset);
        if let DiscordianDate::SeasonDay { season, day, .. } = to_discordian(cand) {
            if season == target_season && day == target_day {
                return cand;
            }
        }
    }
    reference
}

fn short_discordian(d: &DiscordianDate) -> String {
    match d {
        DiscordianDate::SeasonDay {
            season,
            day,
            weekday,
            ..
        } => format!(
            "Season of {season}, {weekday} the {day}{}",
            ordinal_suf(*day)
        ),
        DiscordianDate::StTibsDay { .. } => "St. Tib's Day".to_string(),
    }
}

fn short_discordian_terse(d: &DiscordianDate) -> String {
    match d {
        DiscordianDate::SeasonDay {
            season,
            day,
            weekday,
            ..
        } => format!("{season} {day}, {weekday}"),
        DiscordianDate::StTibsDay { .. } => "St. Tib's Day".to_string(),
    }
}

fn ordinal_suf(n: u8) -> &'static str {
    crate::date::types::ordinal_suffix(n)
}

fn print_json(
    body: Body,
    target_date: NaiveDate,
    angle: f64,
    phase: PhaseName,
    illum_pct: i64,
) -> Result<(), FnordError> {
    let disc = to_discordian(target_date);
    let disc_str = disc.to_string();
    let period = body.orbital_period();
    let d_full = days_to_full(angle, period);
    let d_new = days_to_new(angle, period);

    let obj = json!({
        "body": body.slug(),
        "body_display": body.display_name(),
        "phase_name": phase.label(),
        "phase_angle": angle,
        "illumination_pct": illum_pct,
        "emoji": phase.emoji(),
        "discordian_date": disc_str,
        "gregorian_date": target_date.to_string(),
        "orbital_period_days": period,
        "next_full_days": d_full,
        "next_new_days": d_new,
    });

    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_discordian_terse_formats_seasonday() {
        let d = DiscordianDate::SeasonDay {
            year: 3191,
            season: Season::Chaos,
            day: 5,
            weekday: crate::date::types::Weekday::SettingOrange,
        };
        let s = short_discordian_terse(&d);
        assert!(s.contains("Chaos"));
        assert!(s.contains("5"));
        assert!(s.contains("Setting Orange"));
    }

    #[test]
    fn short_discordian_terse_handles_st_tibs() {
        let d = DiscordianDate::StTibsDay { year: 3190 };
        assert_eq!(short_discordian_terse(&d), "St. Tib's Day");
    }
}
