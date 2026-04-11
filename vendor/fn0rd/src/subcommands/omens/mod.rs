pub mod fetch;
pub mod interpretations;

use chrono::{Local, NaiveDate};
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::OmensArgs;
use crate::config::Config;
use crate::date::convert::{parse_date_arg, to_discordian};
use crate::date::types::{DiscordianDate, Season};
use crate::error::FnordError;
use crate::subcommands::omens::interpretations::{
    directive_for_wind, interpret_condition, normalise_wind, wind_long_name, EXTREME_HEAT_OMEN,
    FREEZING_OMEN, STRONG_WIND_OMEN,
};
use crate::subcommands::util::{hash_str, sparkle};

/// Raw weather extracted either from wttr.in or from the generative model.
#[derive(Debug, Clone)]
pub struct WeatherReading {
    pub location: String,
    pub source: String,
    pub temp_c: f64,
    pub temp_f: f64,
    pub humidity: f64,
    pub wind_kmph: f64,
    pub wind_dir: String,
    pub description: String,
    #[allow(dead_code)]
    pub cloudcover: f64,
    pub precip_mm: f64,
}

/// Discordian-unit representation of a WeatherReading.
#[derive(Debug, Clone)]
pub struct DiscordianWeather {
    pub temp_fn: f64,
    pub temp_label: &'static str,
    pub wind_cu: f64,
    pub confusion_index: f64,
    pub precip_fu: f64,
}

pub fn run(
    args: &OmensArgs,
    config: &Config,
    json: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let date: NaiveDate = match &args.date {
        Some(s) => parse_date_arg(s)?,
        None => Local::now().date_naive(),
    };

    let location = args
        .location
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| config.weather.location.clone());

    let units = args
        .units
        .clone()
        .unwrap_or_else(|| config.weather.units.clone());

    // Decide between live and generative.
    let force_generative = args.generative || location.trim().is_empty();
    let (reading, generative) = if force_generative {
        (
            generative_weather(date, &effective_location(&location)),
            true,
        )
    } else {
        match fetch::fetch_weather(&location) {
            Ok(r) => (r, false),
            Err(_) => (generative_weather(date, &location), true),
        }
    };

    let disc_weather = to_discordian_units(&reading);

    if json {
        return print_json(&reading, &disc_weather, &units, date, generative);
    }

    render(
        &reading,
        &disc_weather,
        RenderOpts {
            units: &units,
            date,
            generative,
            raw: args.raw,
            no_color,
            no_unicode,
        },
    );
    Ok(())
}

fn effective_location(loc: &str) -> String {
    if loc.trim().is_empty() {
        "The Unmapped".to_string()
    } else {
        loc.to_string()
    }
}

struct RenderOpts<'a> {
    units: &'a str,
    date: NaiveDate,
    generative: bool,
    raw: bool,
    no_color: bool,
    no_unicode: bool,
}

/// Render the full omen report to stdout.
fn render(reading: &WeatherReading, disc: &DiscordianWeather, opts: RenderOpts<'_>) {
    let units = opts.units;
    let date = opts.date;
    let generative = opts.generative;
    let raw = opts.raw;
    let no_color = opts.no_color;
    let no_unicode = opts.no_unicode;
    let sp = sparkle(no_unicode);
    let disc_date = to_discordian(date);
    let date_label = match &disc_date {
        DiscordianDate::SeasonDay {
            season,
            day,
            weekday,
            ..
        } => format!(
            "{} {}, {}",
            season.to_string().to_uppercase(),
            day,
            weekday.to_string().to_uppercase()
        ),
        DiscordianDate::StTibsDay { .. } => "ST. TIB'S DAY".to_string(),
    };

    let heading = format!("  {sp} OMENS FOR {date_label} {sp}");
    println!();
    if no_color {
        println!("{heading}");
    } else {
        println!("{}", heading.bold().cyan());
    }
    let flag = if generative {
        " (generative)"
    } else {
        " (live)"
    };
    println!("  Location: {}{}", reading.location, flag);
    println!();

    let glyph = condition_glyph(&reading.description, no_unicode);
    let feel = temp_feel(reading.temp_f);

    if units == "discordian" {
        println!(
            "  {glyph}  {} — {:.1} Flax Units expected",
            reading.description, disc.precip_fu
        );
        println!(
            "  Temperature: {:.1}°Fn (feels like {})",
            disc.temp_fn, feel
        );
        println!("  Confusion Index: {:.0}%", disc.confusion_index);
        println!(
            "  Winds: {:.1} Cabbage Units from the {}",
            disc.wind_cu,
            wind_long_name(&reading.wind_dir)
        );
    } else {
        println!(
            "  {glyph}  {} — {:.1} mm precipitation expected",
            reading.description, reading.precip_mm
        );
        println!(
            "  Temperature: {:.1}°C ({:.1}°F)",
            reading.temp_c, reading.temp_f
        );
        println!("  Humidity: {:.0}%", reading.humidity);
        println!(
            "  Winds: {:.1} km/h from the {}",
            reading.wind_kmph,
            wind_long_name(&reading.wind_dir)
        );
    }

    println!();
    println!("  INTERPRETATION:");
    for line in interpretation_lines(reading) {
        if no_color {
            println!("  {line}");
        } else {
            println!("  {}", line.italic());
        }
    }

    println!();
    println!("  DIRECTIVE:");
    let directive = directive_for_wind(&reading.wind_dir);
    if no_color {
        println!("  {directive}");
    } else {
        println!("  {}", directive.italic());
    }

    println!();
    println!("  All Hail Eris. Carry an umbrella.");

    if raw {
        println!();
        println!("  --- raw (metric) ---");
        println!("  temp_C:        {:.1}", reading.temp_c);
        println!("  temp_F:        {:.1}", reading.temp_f);
        println!("  humidity:      {:.0}%", reading.humidity);
        println!("  wind_kmph:     {:.1}", reading.wind_kmph);
        println!("  wind_dir:      {}", reading.wind_dir);
        println!("  description:   {}", reading.description);
        println!("  precip_mm:     {:.2}", reading.precip_mm);
        println!("  source:        {}", reading.source);
    }
}

fn print_json(
    reading: &WeatherReading,
    disc: &DiscordianWeather,
    units: &str,
    date: NaiveDate,
    generative: bool,
) -> Result<(), FnordError> {
    let directive = directive_for_wind(&reading.wind_dir);
    let lines: Vec<String> = interpretation_lines(reading)
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    let obj = json!({
        "date": date.to_string(),
        "discordian_date": to_discordian(date).to_string(),
        "location": reading.location,
        "source": reading.source,
        "generative": generative,
        "units": units,
        "raw": {
            "temp_c": reading.temp_c,
            "temp_f": reading.temp_f,
            "humidity": reading.humidity,
            "wind_kmph": reading.wind_kmph,
            "wind_dir": reading.wind_dir,
            "wind_dir_normalised": normalise_wind(&reading.wind_dir),
            "description": reading.description,
            "cloudcover": reading.cloudcover,
            "precip_mm": reading.precip_mm,
        },
        "discordian": {
            "temp_fn": disc.temp_fn,
            "temp_label": disc.temp_label,
            "wind_cu": disc.wind_cu,
            "confusion_index": disc.confusion_index,
            "precip_fu": disc.precip_fu,
        },
        "interpretation": lines,
        "directive": directive,
    });

    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    Ok(())
}

/// Build the list of interpretation lines: a base condition omen, plus
/// any temperature/wind extras that also apply.
pub fn interpretation_lines(reading: &WeatherReading) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    out.push(interpret_condition(&reading.description));
    if reading.temp_c >= 32.0 {
        out.push(EXTREME_HEAT_OMEN);
    }
    if reading.temp_c <= 0.0 {
        out.push(FREEZING_OMEN);
    }
    if reading.wind_kmph >= 40.0 {
        out.push(STRONG_WIND_OMEN);
    }
    out
}

fn condition_glyph(desc: &str, no_unicode: bool) -> &'static str {
    if no_unicode {
        return "*";
    }
    let lower = desc.to_lowercase();
    if lower.contains("thunder") {
        "⛈"
    } else if lower.contains("rain") || lower.contains("drizzle") {
        "🌧"
    } else if lower.contains("snow") {
        "❄"
    } else if lower.contains("fog") || lower.contains("mist") || lower.contains("haze") {
        "🌫"
    } else if lower.contains("clear") || lower.contains("sunny") || lower.contains("fair") {
        "☀"
    } else if lower.contains("cloud") || lower.contains("overcast") {
        "☁"
    } else {
        "✦"
    }
}

fn temp_feel(temp_f: f64) -> &'static str {
    if temp_f < 20.0 {
        "bureaucratic winter"
    } else if temp_f < 40.0 {
        "Greyface weather"
    } else if temp_f < 55.0 {
        "mild bureaucracy"
    } else if temp_f < 70.0 {
        "neither hot nor cold, which is itself suspicious"
    } else if temp_f < 85.0 {
        "the Goddess sunbathing"
    } else {
        "chaos on the stove"
    }
}

/// Convert a raw metric reading into Discordian units.
pub fn to_discordian_units(r: &WeatherReading) -> DiscordianWeather {
    let temp_fn = (r.temp_f - 32.0) * 0.3 + 23.0;
    let wind_cu = r.wind_kmph / 8.3;
    let precip_fu = r.precip_mm / 5.0;
    let confusion_index = r.humidity;
    let temp_label = temp_feel(r.temp_f);
    DiscordianWeather {
        temp_fn,
        temp_label,
        wind_cu,
        confusion_index,
        precip_fu,
    }
}

/// Deterministically generate a weather reading for `date` and `location`.
/// Same (date, location) pair always yields identical output.
pub fn generative_weather(date: NaiveDate, location: &str) -> WeatherReading {
    let seed = hash_str(&format!("omens:{date}:{location}"));
    let mut s = seed;

    let disc = to_discordian(date);
    let season = match &disc {
        DiscordianDate::SeasonDay { season, .. } => *season,
        DiscordianDate::StTibsDay { .. } => Season::Chaos,
    };

    // Temperature bounds (°C) per season.
    let (temp_min_c, temp_max_c) = match season {
        Season::Chaos => (-5.0, 10.0),      // Jan–Mar: cold
        Season::Discord => (5.0, 22.0),     // Mar–May: spring
        Season::Confusion => (15.0, 32.0),  // May–Aug: summer
        Season::Bureaucracy => (8.0, 25.0), // Aug–Oct: autumn
        Season::Aftermath => (-3.0, 15.0),  // Oct–Dec: cold
    };

    let temp_c = temp_min_c + sample_unit(&mut s) * (temp_max_c - temp_min_c);
    let temp_f = temp_c * 9.0 / 5.0 + 32.0;

    let conditions = [
        "Clear",
        "Partly Cloudy",
        "Overcast",
        "Drizzle",
        "Rain",
        "Thunderstorm",
        "Fog",
        "Haze",
        "Suspicious Clarity",
        "Unnatural Stillness",
        "Discordant Winds",
        "Sacred Precipitation",
    ];
    let desc_idx = sample_index(&mut s, conditions.len());
    let description = conditions[desc_idx].to_string();

    let wind_kmph = sample_unit(&mut s) * 60.0;
    let wind_dirs = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let wind_dir = wind_dirs[sample_index(&mut s, wind_dirs.len())].to_string();

    let humidity = 20.0 + sample_unit(&mut s) * 75.0;

    // Precipitation only if description implies it.
    let precip_mm = if matches!(
        description.as_str(),
        "Drizzle" | "Rain" | "Thunderstorm" | "Sacred Precipitation"
    ) {
        sample_unit(&mut s) * 12.0
    } else {
        0.0
    };

    WeatherReading {
        location: location.to_string(),
        source: "generative".to_string(),
        temp_c,
        temp_f,
        humidity,
        wind_kmph,
        wind_dir,
        description,
        cloudcover: sample_unit(&mut s) * 100.0,
        precip_mm,
    }
}

/// Step the seed and return a uniform sample in [0.0, 1.0).
fn sample_unit(s: &mut u64) -> f64 {
    *s = hash_str(&format!("{s}"));
    ((*s as u32) as f64) / (u32::MAX as f64)
}

/// Step the seed and return an index in [0, n).
fn sample_index(s: &mut u64, n: usize) -> usize {
    *s = hash_str(&format!("{s}"));
    (*s as usize) % n.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generative_is_deterministic() {
        let d = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let w1 = generative_weather(d, "Portland");
        let w2 = generative_weather(d, "Portland");
        assert_eq!(w1.description, w2.description);
        assert!((w1.temp_c - w2.temp_c).abs() < 1e-9);
        assert!((w1.wind_kmph - w2.wind_kmph).abs() < 1e-9);
        assert_eq!(w1.wind_dir, w2.wind_dir);
        assert!((w1.humidity - w2.humidity).abs() < 1e-9);
    }

    #[test]
    fn generative_varies_across_dates() {
        let mut descriptions = std::collections::HashSet::new();
        for doy in 1..=10 {
            let d = NaiveDate::from_yo_opt(2025, doy * 30).unwrap();
            let w = generative_weather(d, "Nowhere");
            descriptions.insert(w.description);
        }
        assert!(
            descriptions.len() > 1,
            "expected generative to vary across dates, got {:?}",
            descriptions
        );
    }

    #[test]
    fn discordian_conversion_32f_is_23_fn() {
        let r = WeatherReading {
            location: "".to_string(),
            source: "test".to_string(),
            temp_c: 0.0,
            temp_f: 32.0,
            humidity: 50.0,
            wind_kmph: 8.3,
            wind_dir: "N".to_string(),
            description: "Clear".to_string(),
            cloudcover: 0.0,
            precip_mm: 5.0,
        };
        let d = to_discordian_units(&r);
        assert!((d.temp_fn - 23.0).abs() < 1e-6);
        assert!((d.wind_cu - 1.0).abs() < 1e-6);
        assert!((d.precip_fu - 1.0).abs() < 1e-6);
        assert!((d.confusion_index - 50.0).abs() < 1e-9);
    }

    #[test]
    fn fetch_with_empty_location_errors() {
        assert!(fetch::fetch_weather("").is_err());
    }

    #[test]
    fn all_weather_conditions_have_omen() {
        let sample = [
            "Clear",
            "Partly Cloudy",
            "Overcast",
            "Drizzle",
            "Rain",
            "Thunderstorm",
            "Fog",
            "Haze",
            "Suspicious Clarity",
            "Unnatural Stillness",
            "Discordant Winds",
            "Sacred Precipitation",
            "Snow",
            "Mist",
        ];
        for s in &sample {
            assert!(!interpret_condition(s).is_empty(), "no omen for '{s}'");
        }
    }
}
