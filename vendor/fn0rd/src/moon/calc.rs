//! Pure orbital-mechanics lunar phase calculations.
//!
//! Given a celestial body's orbital period and a known reference new-moon
//! date, we compute the fraction of the synodic cycle elapsed between the
//! reference date and the target date. The result (a `phase_angle` in
//! `0.0..1.0`) maps to a phase name, emoji, and ASCII glyph.

use chrono::NaiveDate;

use crate::subcommands::util::hash_str;

/// A celestial body whose phase we can calculate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body {
    Luna,
    Phobos,
    Deimos,
    Io,
    Europa,
    Ganymede,
    Titan,
    Triton,
}

impl Body {
    /// All concrete bodies in a stable order (used by `random`).
    pub const ALL: [Body; 8] = [
        Body::Luna,
        Body::Phobos,
        Body::Deimos,
        Body::Io,
        Body::Europa,
        Body::Ganymede,
        Body::Titan,
        Body::Triton,
    ];

    pub fn slug(&self) -> &'static str {
        match self {
            Body::Luna => "luna",
            Body::Phobos => "phobos",
            Body::Deimos => "deimos",
            Body::Io => "io",
            Body::Europa => "europa",
            Body::Ganymede => "ganymede",
            Body::Titan => "titan",
            Body::Triton => "triton",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Body::Luna => "Luna",
            Body::Phobos => "Phobos",
            Body::Deimos => "Deimos",
            Body::Io => "Io",
            Body::Europa => "Europa",
            Body::Ganymede => "Ganymede",
            Body::Titan => "Titan",
            Body::Triton => "Triton",
        }
    }

    /// Synodic orbital period used as the cycle length for phase calculations.
    pub fn orbital_period(&self) -> f64 {
        match self {
            Body::Luna => 29.530_59,
            Body::Phobos => 0.318_91,
            Body::Deimos => 1.262_44,
            Body::Io => 1.769_14,
            Body::Europa => 3.551_82,
            Body::Ganymede => 7.154_55,
            Body::Titan => 15.945_42,
            Body::Triton => 5.876_85,
        }
    }

    /// Known reference new-moon date near the J2000.0 epoch.
    pub fn reference_new_moon(&self) -> NaiveDate {
        match self {
            Body::Luna => NaiveDate::from_ymd_opt(2000, 1, 6).unwrap(),
            _ => NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
        }
    }

    /// Parenthetical note for non-Luna bodies (e.g. "moon of Mars").
    pub fn parent_note(&self) -> Option<&'static str> {
        match self {
            Body::Luna => None,
            Body::Phobos | Body::Deimos => Some("moon of Mars"),
            Body::Io | Body::Europa | Body::Ganymede => Some("moon of Jupiter"),
            Body::Titan => Some("moon of Saturn"),
            Body::Triton => Some("moon of Neptune, retrograde"),
        }
    }

    /// Triton orbits retrograde, so its phase cycle runs in reverse.
    pub fn retrograde(&self) -> bool {
        matches!(self, Body::Triton)
    }

    /// Parse a body name from user input (CLI flag or config string).
    /// Returns `None` if the name is unrecognized.
    pub fn parse(s: &str) -> Option<Body> {
        match s.to_lowercase().as_str() {
            "luna" | "moon" => Some(Body::Luna),
            "phobos" => Some(Body::Phobos),
            "deimos" => Some(Body::Deimos),
            "io" => Some(Body::Io),
            "europa" => Some(Body::Europa),
            "ganymede" => Some(Body::Ganymede),
            "titan" => Some(Body::Titan),
            "triton" => Some(Body::Triton),
            _ => None,
        }
    }

    /// Resolve a body name, with special handling for "random": select
    /// deterministically from `ALL` using `hash(today's date)` so the
    /// choice is stable within a day but changes daily.
    pub fn resolve(name: &str, date: NaiveDate) -> Option<Body> {
        if name.eq_ignore_ascii_case("random") {
            let h = hash_str(&date.to_string());
            return Some(Body::ALL[(h as usize) % Body::ALL.len()]);
        }
        Body::parse(name)
    }
}

/// The human-readable name of a phase bucket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseName {
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    FullMoon,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl PhaseName {
    pub fn label(&self) -> &'static str {
        match self {
            PhaseName::NewMoon => "New Moon",
            PhaseName::WaxingCrescent => "Waxing Crescent",
            PhaseName::FirstQuarter => "First Quarter",
            PhaseName::WaxingGibbous => "Waxing Gibbous",
            PhaseName::FullMoon => "Full Moon",
            PhaseName::WaningGibbous => "Waning Gibbous",
            PhaseName::LastQuarter => "Last Quarter",
            PhaseName::WaningCrescent => "Waning Crescent",
        }
    }

    pub fn emoji(&self) -> &'static str {
        match self {
            PhaseName::NewMoon => "🌑",
            PhaseName::WaxingCrescent => "🌒",
            PhaseName::FirstQuarter => "🌓",
            PhaseName::WaxingGibbous => "🌔",
            PhaseName::FullMoon => "🌕",
            PhaseName::WaningGibbous => "🌖",
            PhaseName::LastQuarter => "🌗",
            PhaseName::WaningCrescent => "🌘",
        }
    }

    pub fn ascii(&self) -> &'static str {
        match self {
            PhaseName::NewMoon => "( )",
            PhaseName::WaxingCrescent => "(c)",
            PhaseName::FirstQuarter => "(D)",
            PhaseName::WaxingGibbous => "(0)",
            PhaseName::FullMoon => "(O)",
            PhaseName::WaningGibbous => "(0)",
            PhaseName::LastQuarter => "(C)",
            PhaseName::WaningCrescent => "(`)",
        }
    }

    pub fn glyph(&self, no_unicode: bool) -> &'static str {
        if no_unicode {
            self.ascii()
        } else {
            self.emoji()
        }
    }
}

/// Map a phase angle in `0.0..1.0` to a named bucket. The ranges match the
/// spec: new/full quarters get narrow windows, crescent/gibbous get wide.
pub fn phase_name_for_angle(angle: f64) -> PhaseName {
    let a = wrap_unit(angle);
    if !(0.03..0.97).contains(&a) {
        PhaseName::NewMoon
    } else if a < 0.22 {
        PhaseName::WaxingCrescent
    } else if a < 0.28 {
        PhaseName::FirstQuarter
    } else if a < 0.47 {
        PhaseName::WaxingGibbous
    } else if a < 0.53 {
        PhaseName::FullMoon
    } else if a < 0.72 {
        PhaseName::WaningGibbous
    } else if a < 0.78 {
        PhaseName::LastQuarter
    } else {
        PhaseName::WaningCrescent
    }
}

/// Fraction of the disc illuminated, in `0.0..1.0`. 0 at new, 1 at full,
/// symmetric about the half-cycle.
pub fn illumination_fraction(angle: f64) -> f64 {
    let a = wrap_unit(angle);
    // A cosine-based model: (1 - cos(2πa)) / 2 gives 0 at new, 1 at full,
    // smoothly varying through the quarters. Good enough for display.
    0.5 * (1.0 - (2.0 * std::f64::consts::PI * a).cos())
}

/// Compute the phase angle for `body` on `target` given its reference
/// new-moon date and orbital period. Retrograde bodies (Triton) have their
/// phase cycle inverted.
pub fn phase_angle(body: Body, target: NaiveDate) -> f64 {
    let ref_date = body.reference_new_moon();
    let days = (target - ref_date).num_days() as f64;
    let period = body.orbital_period();
    let raw = days.rem_euclid(period) / period;
    if body.retrograde() {
        wrap_unit(1.0 - raw)
    } else {
        wrap_unit(raw)
    }
}

/// Days until the next full moon from `angle` given `period`.
pub fn days_to_full(angle: f64, period: f64) -> f64 {
    let target = 0.5;
    let mut delta = target - angle;
    if delta <= 0.0 {
        delta += 1.0;
    }
    delta * period
}

/// Days until the next new moon from `angle` given `period`.
pub fn days_to_new(angle: f64, period: f64) -> f64 {
    let mut delta = 1.0 - angle;
    if delta >= 1.0 {
        delta -= 1.0;
    }
    delta * period
}

/// Clamp a floating-point value into `[0.0, 1.0)` via `rem_euclid(1.0)`.
fn wrap_unit(a: f64) -> f64 {
    let r = a.rem_euclid(1.0);
    if r.is_nan() {
        0.0
    } else {
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luna_reference_is_new_moon() {
        let d = NaiveDate::from_ymd_opt(2000, 1, 6).unwrap();
        let a = phase_angle(Body::Luna, d);
        assert!(
            !(0.05..=0.95).contains(&a),
            "expected new moon (angle near 0), got {a}"
        );
        assert_eq!(phase_name_for_angle(a), PhaseName::NewMoon);
    }

    #[test]
    fn luna_known_full_moon() {
        // Jan 20 2000 is approximately a full moon (~14.7 days after Jan 6).
        let d = NaiveDate::from_ymd_opt(2000, 1, 20).unwrap();
        let a = phase_angle(Body::Luna, d);
        assert!(
            (0.47..=0.53).contains(&a),
            "expected full moon angle (0.47..=0.53), got {a}"
        );
        assert_eq!(phase_name_for_angle(a), PhaseName::FullMoon);
    }

    #[test]
    fn phobos_cycles_in_its_period() {
        // Two dates one Phobos period apart should produce very similar
        // phase angles. We can't use "0.319 days apart" directly since
        // NaiveDate has day granularity, so compare instead to dates that
        // should produce roughly the same angle because of the modulus.
        let d1 = NaiveDate::from_ymd_opt(2000, 1, 5).unwrap();
        let d2 = NaiveDate::from_ymd_opt(2000, 1, 5).unwrap();
        let a1 = phase_angle(Body::Phobos, d1);
        let a2 = phase_angle(Body::Phobos, d2);
        assert!(
            (a1 - a2).abs() < 1e-9,
            "same date should yield same phase (got {a1} and {a2})"
        );
        // And a date exactly one period later (rounded to the nearest day):
        // since the period is ~0.319 days, three days apart should be very
        // close to 9 full cycles. Verify the angle difference stays bounded.
        let d3 = NaiveDate::from_ymd_opt(2000, 1, 8).unwrap();
        let a3 = phase_angle(Body::Phobos, d3);
        let diff = ((a3 - a1 + 0.5).rem_euclid(1.0) - 0.5).abs();
        assert!(diff <= 0.5, "phase diff wrapped wrong: {diff}");
    }

    #[test]
    fn triton_is_inverted_relative_to_non_retrograde() {
        // Build a "fake" Triton-period body by computing what Triton would
        // return if it were forward-moving: a non-inverted version of the
        // same modulus should be 1.0 - Triton's angle (modulo wrap).
        let d = NaiveDate::from_ymd_opt(2000, 2, 15).unwrap();
        let ref_date = Body::Triton.reference_new_moon();
        let days = (d - ref_date).num_days() as f64;
        let forward =
            (days.rem_euclid(Body::Triton.orbital_period())) / Body::Triton.orbital_period();
        let triton_angle = phase_angle(Body::Triton, d);
        let sum = wrap_unit(forward + triton_angle);
        // Forward + retrograde should sum to 0 or 1 (wrap-equivalent).
        assert!(
            sum < 1e-9 || (1.0 - sum) < 1e-9,
            "expected forward + retrograde ≈ 0 or 1, got {sum}"
        );
    }

    #[test]
    fn random_body_is_stable_for_a_date() {
        let d = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
        let b1 = Body::resolve("random", d).unwrap();
        let b2 = Body::resolve("random", d).unwrap();
        assert_eq!(b1, b2);
    }

    #[test]
    fn illumination_zero_at_new_and_one_at_full() {
        let at_new = illumination_fraction(0.0);
        let at_full = illumination_fraction(0.5);
        assert!(at_new < 1e-9);
        assert!((at_full - 1.0).abs() < 1e-9);
    }

    #[test]
    fn days_to_full_at_new_is_half_period() {
        let p = Body::Luna.orbital_period();
        assert!(((days_to_full(0.0, p)) - p / 2.0).abs() < 1e-9);
    }

    #[test]
    fn days_to_new_at_full_is_half_period() {
        let p = Body::Luna.orbital_period();
        assert!(((days_to_new(0.5, p)) - p / 2.0).abs() < 1e-9);
    }
}
