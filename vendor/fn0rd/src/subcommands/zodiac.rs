use chrono::{Local, NaiveDate};
use owo_colors::OwoColorize;
use serde_json::json;

use crate::cli::ZodiacArgs;
use crate::config::Config;
use crate::date::convert::parse_date_arg;
use crate::error::FnordError;
use crate::subcommands::util::sparkle;
use crate::zodiac::{parse_system, Sign};

pub fn run(
    args: &ZodiacArgs,
    config: &Config,
    json: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let date: NaiveDate = match &args.date {
        Some(s) => parse_date_arg(s)?,
        None => Local::now().date_naive(),
    };

    let system_name = args
        .system
        .clone()
        .unwrap_or_else(|| config.zodiac.system.clone());
    let system = parse_system(&system_name).ok_or_else(|| {
        FnordError::Parse(format!(
            "unknown zodiac system: '{system_name}' (expected western, vedic, chinese, discordian)"
        ))
    })?;

    let sign = system.sign_for(date);

    if json {
        return print_json(&sign, date);
    }

    render(&sign, date, args.full, no_color, no_unicode);
    Ok(())
}

fn render(sign: &Sign, date: NaiveDate, full: bool, no_color: bool, no_unicode: bool) {
    let sp = sparkle(no_unicode);
    let heading = format!("  {sp} {} ZODIAC {sp}", sign.system_label.to_uppercase());
    println!();
    if no_color {
        println!("{heading}");
    } else {
        println!("{}", heading.bold().cyan());
    }
    println!();

    let symbol = if no_unicode || sign.symbol.is_empty() {
        String::new()
    } else {
        format!(" {}", sign.symbol)
    };

    if no_color {
        println!("  {}: {}{symbol}", sign.system_label, sign.name);
    } else {
        println!(
            "  {}: {}{symbol}",
            sign.system_label,
            sign.name.bold().magenta()
        );
    }
    println!("  {}", sign.tagline);
    println!();

    // Extras (apostle, sacred object, element, etc.)
    let mut horoscope: Option<&String> = None;
    for (k, v) in &sign.extras {
        if k == "horoscope" {
            horoscope = Some(v);
            continue;
        }
        let pretty_k = pretty_key(k);
        println!("  {pretty_k}: {v}");
    }

    if let Some(h) = horoscope {
        println!();
        println!("  Today's alignment ({date}):");
        if no_color {
            println!("  {h}");
        } else {
            println!("  {}", h.italic());
        }
        println!();
        println!("  Hail Eris.");
    }

    if full {
        println!();
        println!("  {}", sign.description);
    }
}

fn pretty_key(k: &str) -> String {
    k.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_json(sign: &Sign, date: NaiveDate) -> Result<(), FnordError> {
    let extras_obj: serde_json::Map<String, serde_json::Value> = sign
        .extras
        .iter()
        .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
        .collect();

    let obj = json!({
        "system": sign.system,
        "system_label": sign.system_label,
        "sign": sign.name,
        "symbol": sign.symbol,
        "tagline": sign.tagline,
        "description": sign.description,
        "date": date.to_string(),
        "extras": extras_obj,
    });

    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_key_capitalises_and_spaces() {
        assert_eq!(pretty_key("sacred_object"), "Sacred Object");
        assert_eq!(pretty_key("apostle"), "Apostle");
    }
}
