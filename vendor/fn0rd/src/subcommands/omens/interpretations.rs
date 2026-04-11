//! Static tables that translate weather conditions and wind directions
//! into Discordian omen text.
//!
//! - `interpret_condition` accepts a free-form wttr.in condition string
//!   and returns the first matching omen, or a generic fallback.
//! - `directive_for_wind` maps a 16-point compass abbreviation down to one
//!   of the eight cardinal directions and returns a directive line.

/// Condition keyword → omen interpretation. First match wins.
/// The keyword is matched case-insensitively against the weather
/// description.
pub const CONDITION_OMENS: &[(&str, &str)] = &[
    ("thunderstorm", "Eris is bowling. Stay indoors or don't."),
    ("thunder", "The sky is arguing with itself. It will lose."),
    (
        "lightning",
        "The fnords have been electrified. Ground yourself.",
    ),
    (
        "heavy rain",
        "Sacred precipitation cleanses the bureaucracy. Temporarily.",
    ),
    (
        "torrential",
        "The Goddess is wringing out the week. Find a porch.",
    ),
    (
        "rain",
        "Rain falls. Bureaucrats complain. Balance restored.",
    ),
    ("drizzle", "A reluctant rain. The sky is hedging."),
    ("sleet", "The sky cannot commit. Dress for all outcomes."),
    (
        "snow",
        "Order attempts to blanket chaos in white. It will melt.",
    ),
    (
        "blizzard",
        "Entropy in powdered form. Do not drive toward it.",
    ),
    ("hail", "The sky is throwing things. Take the hint."),
    ("fog", "The fnords are thick today. Visibility: spiritual."),
    (
        "mist",
        "The world has been lightly redacted. Read between the drops.",
    ),
    (
        "haze",
        "A gentle obscuring. You are not less seen; you are less seeable.",
    ),
    (
        "smoke",
        "Something is burning. Probably paperwork. Open a window.",
    ),
    ("overcast", "The sky is indecisive. Join the club."),
    ("partly cloudy", "The universe is undecided. Join the club."),
    ("mostly cloudy", "Cloud is in charge today. Bow politely."),
    ("cloudy", "Cloud is watching. Be interesting."),
    ("clear", "The Goddess smiles. Be suspicious."),
    (
        "sunny",
        "The sun performs its one trick with great sincerity. Applaud.",
    ),
    ("fair", "Neither good nor bad. Probably a trap."),
    (
        "suspicious clarity",
        "It is too clear. Something is about to happen. Something is always about to happen.",
    ),
    (
        "unnatural stillness",
        "The fnords are holding their breath. Pretend not to notice.",
    ),
    (
        "discordant winds",
        "The winds have made a committee and the committee is fighting.",
    ),
    (
        "sacred precipitation",
        "It is raining on purpose. Catch some if you can.",
    ),
];

/// Generic fallbacks triggered when no keyword matches.
pub const FALLBACK_OMEN: &str = "The weather refuses to be interpreted. This, too, is information.";

/// Temperature-band omens, used in addition to condition omens.
pub const EXTREME_HEAT_OMEN: &str = "The Sacred Chao is overheating. Offer it a cold beverage.";
pub const FREEZING_OMEN: &str = "Greyface has won, temporarily. Wear a coat.";
pub const STRONG_WIND_OMEN: &str = "The winds carry messages. They are probably wrong.";

/// 8-point wind directive table.
pub const WIND_DIRECTIVES: &[(&str, &str)] = &[
    ("N", "Avoid paperwork today."),
    (
        "NE",
        "A bureaucrat approaches from the northeast. Prepare fnords.",
    ),
    ("E", "Something unexpected arrives. Welcome it."),
    ("SE", "Discord flows from the southeast. Embrace it."),
    ("S", "The Goddess points south. Follow or don't."),
    ("SW", "Confusion comes. It was already here."),
    ("W", "Look west. Then look east. Then stop looking."),
    ("NW", "The northwest wind carries five tons of flax. Duck."),
];

/// Return the first omen matching the weather description, or the
/// fallback string. Matching is case-insensitive and substring-based.
pub fn interpret_condition(desc: &str) -> &'static str {
    let lower = desc.to_lowercase();
    for (kw, omen) in CONDITION_OMENS {
        if lower.contains(kw) {
            return omen;
        }
    }
    FALLBACK_OMEN
}

/// Normalize a 16-point wind direction (e.g. "NNE", "ENE") to one of
/// N/NE/E/SE/S/SW/W/NW by dropping the tertiary component.
pub fn normalise_wind(dir: &str) -> &'static str {
    let d = dir.trim().to_uppercase();
    match d.as_str() {
        "N" | "NNE" | "NNW" => "N",
        "NE" | "ENE" => "NE",
        "E" | "ESE" => "E",
        "SE" | "SSE" => "SE",
        "S" | "SSW" => "S",
        "SW" | "WSW" => "SW",
        "W" | "WNW" => "W",
        "NW" => "NW",
        _ => "N",
    }
}

/// Return the directive for a wind direction, matching by normalised
/// cardinal abbreviation. Always returns a non-empty string.
pub fn directive_for_wind(dir: &str) -> &'static str {
    let norm = normalise_wind(dir);
    for (d, text) in WIND_DIRECTIVES {
        if *d == norm {
            return text;
        }
    }
    WIND_DIRECTIVES[0].1
}

/// Human-readable long name for a cardinal abbreviation.
pub fn wind_long_name(dir: &str) -> &'static str {
    match normalise_wind(dir) {
        "N" => "North",
        "NE" => "Northeast",
        "E" => "East",
        "SE" => "Southeast",
        "S" => "South",
        "SW" => "Southwest",
        "W" => "West",
        "NW" => "Northwest",
        _ => "North",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_condition_omens_non_empty() {
        assert!(CONDITION_OMENS.len() >= 20);
        for (k, omen) in CONDITION_OMENS {
            assert!(!k.is_empty());
            assert!(!omen.is_empty());
        }
    }

    #[test]
    fn all_wind_directives_non_empty() {
        for (d, text) in WIND_DIRECTIVES {
            assert!(!d.is_empty());
            assert!(!text.is_empty());
        }
        assert_eq!(WIND_DIRECTIVES.len(), 8);
    }

    #[test]
    fn all_conditions_map_to_non_empty_omen() {
        for (k, _) in CONDITION_OMENS {
            let o = interpret_condition(k);
            assert!(!o.is_empty());
        }
    }

    #[test]
    fn all_wind_directions_map_to_directive() {
        for d in &["N", "NE", "E", "SE", "S", "SW", "W", "NW"] {
            let t = directive_for_wind(d);
            assert!(!t.is_empty());
        }
    }

    #[test]
    fn interpret_unknown_returns_fallback() {
        assert_eq!(
            interpret_condition("existential vibration of the hotdog meridian"),
            FALLBACK_OMEN
        );
    }

    #[test]
    fn interpret_is_case_insensitive() {
        assert_eq!(
            interpret_condition("Clear sky"),
            "The Goddess smiles. Be suspicious."
        );
    }

    #[test]
    fn normalise_wind_handles_16_point() {
        assert_eq!(normalise_wind("NNE"), "N");
        assert_eq!(normalise_wind("ENE"), "NE");
        assert_eq!(normalise_wind("WSW"), "SW");
    }
}
