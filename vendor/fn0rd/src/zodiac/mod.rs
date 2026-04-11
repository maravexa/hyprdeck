//! Zodiac sign lookups across four systems: Western, Vedic, Chinese,
//! and Discordian. Each system implements `ZodiacSystem` and returns
//! a `Sign` for a given Gregorian date.

pub mod chinese;
pub mod discordian;
pub mod vedic;
pub mod western;

use chrono::NaiveDate;

/// A fully-rendered zodiac sign, ready for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sign {
    /// System slug (e.g. "western", "discordian").
    pub system: &'static str,
    /// Human-friendly system label (e.g. "Western").
    pub system_label: &'static str,
    /// Sign name ("Scorpio", "The Discord", "Rat").
    pub name: String,
    /// Short symbol or emoji; may be empty for systems without a glyph.
    pub symbol: String,
    /// One-line tagline for the sign.
    pub tagline: String,
    /// Extended description shown with `--full`.
    pub description: String,
    /// System-specific extras rendered under the sign (key-value).
    pub extras: Vec<(String, String)>,
}

pub trait ZodiacSystem {
    fn sign_for(&self, date: NaiveDate) -> Sign;
}

/// Resolve a system name from user input. Returns `None` if unrecognized.
pub fn parse_system(name: &str) -> Option<Box<dyn ZodiacSystem>> {
    match name.to_lowercase().as_str() {
        "western" => Some(Box::new(western::Western)),
        "vedic" => Some(Box::new(vedic::Vedic)),
        "chinese" => Some(Box::new(chinese::Chinese)),
        "discordian" => Some(Box::new(discordian::Discordian)),
        _ => None,
    }
}
