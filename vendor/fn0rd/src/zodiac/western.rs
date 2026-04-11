//! Tropical (Western) zodiac. Signs are assigned by Gregorian month/day
//! using the standard cutoff dates.

use chrono::{Datelike, NaiveDate};

use crate::zodiac::{Sign, ZodiacSystem};

pub struct Western;

/// Static table: (name, symbol, tagline, description). Index matches
/// [`sign_index_for_date`] below.
const SIGNS: [(&str, &str, &str, &str); 12] = [
    (
        "Aries",
        "♈",
        "Born under the sign of ignition, momentum, and ill-considered commitments.",
        "Aries burns first and asks questions later. Ruled by Mars, the ram charges into every situation with conviction that is only occasionally warranted. Excellent at beginnings, terrible at follow-through.",
    ),
    (
        "Taurus",
        "♉",
        "Born under the sign of appetite, patience, and gently immovable opinions.",
        "Taurus will not be hurried. Taurus will eat what Taurus wants. Ruled by Venus, the bull values comfort, gravity, and the inertia of a well-planted hoof.",
    ),
    (
        "Gemini",
        "♊",
        "Born under the sign of contradiction, cleverness, and unfinished sentences.",
        "Gemini knows both sides of every argument and is still undecided. Ruled by Mercury, the twins trade ideas faster than anyone can verify them.",
    ),
    (
        "Cancer",
        "♋",
        "Born under the sign of tides, memory, and load-bearing feelings.",
        "Cancer remembers. Cancer carries every slight like a seashell. Ruled by the Moon, the crab builds fortresses out of soft things and makes them feel like home.",
    ),
    (
        "Leo",
        "♌",
        "Born under the sign of spotlight, pride, and unapologetic volume.",
        "Leo enters the room and the room notices. Ruled by the Sun, the lion performs even when no one is watching — especially when no one is watching.",
    ),
    (
        "Virgo",
        "♍",
        "Born under the sign of lists, revision, and polite suspicion.",
        "Virgo already noticed the typo. Ruled by Mercury, the virgin catalogs the world into tidy categories and is quietly disappointed when the world does not comply.",
    ),
    (
        "Libra",
        "♎",
        "Born under the sign of balance, indecision, and diplomatic ambiguity.",
        "Libra will not pick a restaurant. Ruled by Venus, the scales weigh every possibility until the opportunity has gently walked away.",
    ),
    (
        "Scorpio",
        "♏",
        "Born under the sign of intensity, mystery, and misplaced keys.",
        "Scorpio knows your secret and will not tell you which one. Ruled by Mars and Pluto, the scorpion stings last, remembers longest, and trusts no one with the good gossip.",
    ),
    (
        "Sagittarius",
        "♐",
        "Born under the sign of travel, opinions, and suspiciously strong convictions.",
        "Sagittarius has a theory and it is lovingly wrong. Ruled by Jupiter, the archer aims at truth and hits the neighbor's barn.",
    ),
    (
        "Capricorn",
        "♑",
        "Born under the sign of structure, patience, and long-term scheming.",
        "Capricorn has a ten-year plan and it is on track. Ruled by Saturn, the goat climbs slowly, never slips, and quietly outlasts everyone who mocked the climbing.",
    ),
    (
        "Aquarius",
        "♒",
        "Born under the sign of eccentricity, theory, and radio interference.",
        "Aquarius is two decades ahead of you and also slightly lost. Ruled by Saturn and Uranus, the water-bearer pours ideas nobody ordered into the nearest available bucket.",
    ),
    (
        "Pisces",
        "♓",
        "Born under the sign of dreams, oceans, and gracefully losing track.",
        "Pisces is not asleep, Pisces is listening. Ruled by Jupiter and Neptune, the fish swims in whatever direction feels true in this particular minute.",
    ),
];

impl ZodiacSystem for Western {
    fn sign_for(&self, date: NaiveDate) -> Sign {
        let idx = sign_index_for_date(date.month(), date.day());
        sign_from_index(idx)
    }
}

/// Convert a `(month, day)` pair to the index into [`SIGNS`].
pub fn sign_index_for_date(month: u32, day: u32) -> usize {
    match (month, day) {
        (3, 21..=31) | (4, 1..=19) => 0,   // Aries
        (4, 20..=30) | (5, 1..=20) => 1,   // Taurus
        (5, 21..=31) | (6, 1..=20) => 2,   // Gemini
        (6, 21..=30) | (7, 1..=22) => 3,   // Cancer
        (7, 23..=31) | (8, 1..=22) => 4,   // Leo
        (8, 23..=31) | (9, 1..=22) => 5,   // Virgo
        (9, 23..=30) | (10, 1..=22) => 6,  // Libra
        (10, 23..=31) | (11, 1..=21) => 7, // Scorpio
        (11, 22..=30) | (12, 1..=21) => 8, // Sagittarius
        (12, 22..=31) | (1, 1..=19) => 9,  // Capricorn
        (1, 20..=31) | (2, 1..=18) => 10,  // Aquarius
        (2, 19..=29) | (3, 1..=20) => 11,  // Pisces
        _ => 9,                            // defensive fallback: Capricorn
    }
}

pub fn sign_from_index(idx: usize) -> Sign {
    let (name, symbol, tagline, description) = SIGNS[idx % 12];
    Sign {
        system: "western",
        system_label: "Western",
        name: name.to_string(),
        symbol: symbol.to_string(),
        tagline: tagline.to_string(),
        description: description.to_string(),
        extras: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn jan_1_is_capricorn() {
        let s = Western.sign_for(date(2025, 1, 1));
        assert_eq!(s.name, "Capricorn");
    }

    #[test]
    fn jul_4_is_cancer() {
        let s = Western.sign_for(date(2025, 7, 4));
        assert_eq!(s.name, "Cancer");
    }

    #[test]
    fn feb_29_leap_is_pisces() {
        let s = Western.sign_for(date(2024, 2, 29));
        assert_eq!(s.name, "Pisces");
    }

    #[test]
    fn all_twelve_signs_are_reachable() {
        let mut seen = std::collections::HashSet::new();
        for doy in 1..=365 {
            let d = NaiveDate::from_yo_opt(2025, doy).unwrap();
            let s = Western.sign_for(d);
            seen.insert(s.name);
        }
        assert_eq!(seen.len(), 12);
    }
}
