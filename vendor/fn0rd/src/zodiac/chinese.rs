//! Chinese zodiac. Signs cycle every 12 years; elements every 10 (one
//! element per pair of consecutive years). Reference year for Rat is 1900.

use chrono::{Datelike, NaiveDate};

use crate::zodiac::{Sign, ZodiacSystem};

pub struct Chinese;

const ANIMALS: [(&str, &str, &str); 12] = [
    (
        "Rat",
        "🐀",
        "Resourceful, quick-witted, and always one step ahead of the trap.",
    ),
    ("Ox", "🐂", "Steady, patient, and suspicious of shortcuts."),
    (
        "Tiger",
        "🐅",
        "Brave, impulsive, and prone to dramatic entrances.",
    ),
    (
        "Rabbit",
        "🐇",
        "Gentle, diplomatic, and slightly elsewhere.",
    ),
    (
        "Dragon",
        "🐉",
        "Charismatic, confident, and accidentally in charge.",
    ),
    (
        "Snake",
        "🐍",
        "Wise, inscrutable, and owes you no explanations.",
    ),
    (
        "Horse",
        "🐎",
        "Free-spirited, restless, and bad at standing still.",
    ),
    (
        "Goat",
        "🐐",
        "Creative, tender-hearted, and quietly stubborn.",
    ),
    (
        "Monkey",
        "🐒",
        "Clever, mischievous, and a terrible influence.",
    ),
    (
        "Rooster",
        "🐓",
        "Observant, punctual, and loud on principle.",
    ),
    ("Dog", "🐕", "Loyal, honest, and deeply worried about you."),
    ("Pig", "🐖", "Generous, earnest, and in favor of snacks."),
];

const ELEMENTS: [&str; 5] = ["Metal", "Water", "Wood", "Fire", "Earth"];

impl ZodiacSystem for Chinese {
    fn sign_for(&self, date: NaiveDate) -> Sign {
        let year = date.year();
        let (animal, element) = animal_and_element(year);
        let (name, symbol, desc) = ANIMALS[animal];
        let element_name = ELEMENTS[element];

        Sign {
            system: "chinese",
            system_label: "Chinese",
            name: name.to_string(),
            symbol: symbol.to_string(),
            tagline: format!("Year of the {element_name} {name}."),
            description: format!("{desc} The {element_name} aspect lends its colour to the year."),
            extras: vec![
                ("element".to_string(), element_name.to_string()),
                ("year".to_string(), year.to_string()),
            ],
        }
    }
}

/// Compute (animal_index, element_index) for a given year.
/// Uses 1900 (Metal Rat in many modern tables) as the reference. We apply
/// `(year - 1900) % 10 / 2` for element, yielding 5 elements.
pub fn animal_and_element(year: i32) -> (usize, usize) {
    let diff = year - 1900;
    let animal = diff.rem_euclid(12) as usize;
    let element = (diff.rem_euclid(10) / 2) as usize;
    (animal, element)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn year_1900_is_rat() {
        let s = Chinese.sign_for(date(1900, 6, 15));
        assert_eq!(s.name, "Rat");
    }

    #[test]
    fn year_2000_is_dragon() {
        let s = Chinese.sign_for(date(2000, 6, 15));
        assert_eq!(s.name, "Dragon");
    }

    #[test]
    fn year_1984_is_rat_cycle_repeats() {
        let s = Chinese.sign_for(date(1984, 6, 15));
        assert_eq!(s.name, "Rat");
    }

    #[test]
    fn negative_year_offsets_do_not_panic() {
        let s = Chinese.sign_for(date(1800, 6, 15));
        assert!(!s.name.is_empty());
    }

    #[test]
    fn element_rotates_every_two_years() {
        let (_, e1) = animal_and_element(1900);
        let (_, e2) = animal_and_element(1901);
        let (_, e3) = animal_and_element(1902);
        assert_eq!(e1, e2);
        assert_ne!(e1, e3);
    }
}
