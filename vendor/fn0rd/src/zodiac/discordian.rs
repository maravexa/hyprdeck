//! The Discordian zodiac — five signs aligned to the five Discordian
//! seasons, plus "The Void" for anyone unfortunate enough to be born on
//! St. Tib's Day.

use chrono::NaiveDate;

use crate::date::convert::to_discordian;
use crate::date::types::{DiscordianDate, Season};
use crate::subcommands::util::hash_str;
use crate::zodiac::{Sign, ZodiacSystem};

pub struct Discordian;

/// Per-sign metadata. Each entry is:
///   (sign_name, glyph, apostle, sacred_object, tagline, description,
///    [5 horoscope templates])
struct DiscSignData {
    name: &'static str,
    glyph: &'static str,
    apostle: &'static str,
    sacred: &'static str,
    tagline: &'static str,
    description: &'static str,
    horoscopes: [&'static str; 5],
}

const CHAO: DiscSignData = DiscSignData {
    name: "The Chao",
    glyph: "☯",
    apostle: "Hung Mung",
    sacred: "The Apple",
    tagline: "Ruled by beginnings, primordial confusion, and the sound of a gong.",
    description: "Born in the Season of Chaos, The Chao begins every sentence without finishing the last one. Ruled by Hung Mung, under the sacred apple, The Chao trusts the first instinct and regrets the second.",
    horoscopes: [
        "{name}, today the gong is still ringing. Follow the echo, not the source.",
        "{name}, the apple falls where it pleases. Do not catch it unless asked.",
        "{name}, five tons of flax arrive before lunch. Make room.",
        "{name}, your first instinct is the correct one. Your second is a bureaucrat in disguise.",
        "{name}, Hung Mung smiles. Say nothing, it's contagious.",
    ],
};

const DISCORD: DiscSignData = DiscSignData {
    name: "The Discord",
    glyph: "⚔",
    apostle: "Dr. Van Van Mojo",
    sacred: "The Cabbage",
    tagline: "Ruled by friction, useful disagreement, and the cabbage in the kitchen drawer.",
    description: "Born in the Season of Discord, The Discord argues with elevators and wins. Ruled by Dr. Van Van Mojo, under the sacred cabbage, The Discord knows that two things rubbing together make fire, noise, or dinner.",
    horoscopes: [
        "{name}, an argument approaches. Lose it on purpose; it pays better.",
        "{name}, the cabbage is speaking. It is wrong, but listen anyway.",
        "{name}, today you are right about something trivial. Do not gloat.",
        "{name}, Van Van Mojo is laughing at a joke you haven't told yet.",
        "{name}, friction is a resource. Bill for it.",
    ],
};

const CONFUSION: DiscSignData = DiscSignData {
    name: "The Confusion",
    glyph: "⚡",
    apostle: "Sri Syadasti",
    sacred: "The Fnord",
    tagline: "Ruled by paradox, indecision, and things that were never quite there.",
    description: "Born in the Season of Confusion, The Confusion reads the menu twice and still orders the wrong thing on purpose. Ruled by Sri Syadasti, under the sacred fnord, The Confusion knows that 'maybe' is a complete sentence.",
    horoscopes: [
        "{name}, today will become clearer, then immediately less clear. This is progress.",
        "{name}, do not look for the fnord. It is already looking back.",
        "{name}, Sri Syadasti shrugs. Consider shrugging back.",
        "{name}, the answer to the question is also the question. Try again later.",
        "{name}, you will misplace something important and find something better.",
    ],
};

const BUREAU: DiscSignData = DiscSignData {
    name: "The Bureau",
    glyph: "📜",
    apostle: "Zarathud",
    sacred: "The Hotdog",
    tagline: "Ruled by forms, formalism, and the sacred hotdog on a bun-shaped universe.",
    description: "Born in the Season of Bureaucracy, The Bureau has a stamp for that. Ruled by Zarathud, under the sacred hotdog, The Bureau understands that paperwork is a form of prayer and that the filing cabinet is eternal.",
    horoscopes: [
        "{name}, a form awaits. Fill it out in pencil. Erase everything.",
        "{name}, the hotdog is not a sandwich today. Tomorrow, maybe.",
        "{name}, Zarathud approves your application in principle and denies it in practice.",
        "{name}, order triumphs briefly. Enjoy the view before the stamp runs out of ink.",
        "{name}, today's paperwork is tomorrow's kindling.",
    ],
};

const AFTERMATH: DiscSignData = DiscSignData {
    name: "The Aftermath",
    glyph: "🍂",
    apostle: "The Elder Malaclypse",
    sacred: "The Chao",
    tagline: "Ruled by afterthoughts, debris, and the quiet satisfaction of a chao well thrown.",
    description: "Born in the Season of Aftermath, The Aftermath sweeps up after the party and keeps the good glassware. Ruled by The Elder Malaclypse, under the sacred chao, The Aftermath is the shape left behind when everyone else has stopped paying attention.",
    horoscopes: [
        "{name}, today you inherit someone else's mistake. Rename it.",
        "{name}, the Elder Malaclypse leaves you a broom and a blessing. Both are useful.",
        "{name}, the sacred chao rolls. Be where it lands, not where it started.",
        "{name}, a small thing ends. A smaller thing begins. Keep the receipt.",
        "{name}, nothing important happened today, which is itself important.",
    ],
};

impl ZodiacSystem for Discordian {
    fn sign_for(&self, date: NaiveDate) -> Sign {
        let disc = to_discordian(date);

        // St. Tib's Day: The Void
        if let DiscordianDate::StTibsDay { .. } = disc {
            return void_sign(date);
        }

        let season = match &disc {
            DiscordianDate::SeasonDay { season, .. } => *season,
            _ => Season::Chaos,
        };

        let data = season_data(season);
        let horoscope_idx =
            hash_str(&format!("{}:{}", date, data.name)) as usize % data.horoscopes.len();
        let horoscope = data.horoscopes[horoscope_idx].replace("{name}", "traveller");

        let extras = vec![
            ("apostle".to_string(), data.apostle.to_string()),
            ("sacred_object".to_string(), data.sacred.to_string()),
            ("horoscope".to_string(), horoscope),
        ];

        Sign {
            system: "discordian",
            system_label: "Discordian",
            name: data.name.to_string(),
            symbol: data.glyph.to_string(),
            tagline: data.tagline.to_string(),
            description: data.description.to_string(),
            extras,
        }
    }
}

fn season_data(season: Season) -> DiscSignData {
    match season {
        Season::Chaos => CHAO,
        Season::Discord => DISCORD,
        Season::Confusion => CONFUSION,
        Season::Bureaucracy => BUREAU,
        Season::Aftermath => AFTERMATH,
    }
}

fn void_sign(_date: NaiveDate) -> Sign {
    Sign {
        system: "discordian",
        system_label: "Discordian",
        name: "The Void".to_string(),
        symbol: "∅".to_string(),
        tagline: "Born outside all seasons, answerable to no zodiac.".to_string(),
        description:
            "Those born on St. Tib's Day are The Void — exempt from all signs, all predictions, all calendars. Every four years, reality briefly agrees. The rest of the time, it forgets."
                .to_string(),
        extras: vec![
            ("apostle".to_string(), "none".to_string()),
            ("sacred_object".to_string(), "the empty set".to_string()),
            (
                "horoscope".to_string(),
                "Today does not apply to you. Proceed with impunity.".to_string(),
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn chaos_day_1_is_the_chao() {
        // Jan 1 = Chaos 1
        let s = Discordian.sign_for(date(2025, 1, 1));
        assert_eq!(s.name, "The Chao");
    }

    #[test]
    fn st_tibs_is_the_void() {
        let s = Discordian.sign_for(date(2024, 2, 29));
        assert_eq!(s.name, "The Void");
    }

    #[test]
    fn aftermath_day_73_is_the_aftermath() {
        // Aftermath 73 = Dec 31 non-leap
        let s = Discordian.sign_for(date(2025, 12, 31));
        assert_eq!(s.name, "The Aftermath");
    }

    #[test]
    fn horoscope_replaces_name_placeholder() {
        let s = Discordian.sign_for(date(2025, 1, 1));
        let horoscope = s
            .extras
            .iter()
            .find(|(k, _)| k == "horoscope")
            .map(|(_, v)| v.clone())
            .unwrap();
        assert!(!horoscope.contains("{name}"));
    }
}
