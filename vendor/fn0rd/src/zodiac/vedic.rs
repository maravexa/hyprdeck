//! Sidereal (Vedic) zodiac. Computed from the tropical position minus
//! an ayanamsa offset of 23.85 degrees, which in practice shifts the
//! result back by approximately 23 days.

use chrono::{Datelike, Duration, NaiveDate};

use crate::zodiac::western::{sign_from_index, sign_index_for_date};
use crate::zodiac::{Sign, ZodiacSystem};

pub struct Vedic;

/// Number of days to shift the tropical date backward to approximate
/// the sidereal position. 23.85° / (360° / 365.25d) ≈ 24.2 days; round to 24.
const AYANAMSA_DAY_SHIFT: i64 = 24;

impl ZodiacSystem for Vedic {
    fn sign_for(&self, date: NaiveDate) -> Sign {
        let shifted = date - Duration::days(AYANAMSA_DAY_SHIFT);
        let idx = sign_index_for_date(shifted.month(), shifted.day());
        let mut s = sign_from_index(idx);
        s.system = "vedic";
        s.system_label = "Vedic";
        s.tagline = format!("{} (sidereal / Vedic)", s.tagline);
        s.extras
            .push(("ayanamsa".to_string(), "23.85°".to_string()));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn vedic_shifts_western_by_roughly_23_days() {
        // Apr 15 tropical = Aries; apr 15 - 24 = Mar 22 tropical = still Aries.
        // Let's pick a date where the shift crosses a boundary:
        // Apr 13 tropical = Aries; Apr 13 - 24 = Mar 20 = Pisces.
        let s = Vedic.sign_for(date(2025, 4, 13));
        assert_eq!(s.system, "vedic");
        assert_eq!(s.name, "Pisces");
    }
}
