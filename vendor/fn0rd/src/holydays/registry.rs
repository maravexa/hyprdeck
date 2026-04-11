use std::collections::HashMap;

use crate::date::types::DiscordianDate;
use crate::holydays::types::{Holyday, HolydayKey, HolydaySource};

pub struct HolydayRegistry {
    /// Recurring holydays, keyed by HolydayKey. Later insertions (higher precedence) overwrite.
    recurring: HashMap<HolydayKey, Holyday>,
    /// One-time holydays keyed by (year, HolydayKey).
    one_time: HashMap<(i32, HolydayKey), Holyday>,
}

impl HolydayRegistry {
    pub fn build(defaults: Vec<Holyday>, cabal: Vec<Holyday>, personal: Vec<Holyday>) -> Self {
        let mut recurring: HashMap<HolydayKey, Holyday> = HashMap::new();
        let mut one_time: HashMap<(i32, HolydayKey), Holyday> = HashMap::new();

        for holyday in defaults.into_iter().chain(cabal).chain(personal) {
            if holyday.recurring {
                recurring.insert(holyday.key.clone(), holyday);
            } else if let Some(year) = holyday.year {
                one_time.insert((year, holyday.key.clone()), holyday);
            }
            // Non-recurring without year: ignore (malformed)
        }

        HolydayRegistry {
            recurring,
            one_time,
        }
    }

    pub fn lookup(&self, date: &DiscordianDate) -> Vec<&Holyday> {
        let key = date.holyday_key();
        let year = date.year();
        let mut results = vec![];

        // One-time entries take priority and are checked first
        if let Some(h) = self.one_time.get(&(year, key.clone())) {
            results.push(h);
            return results;
        }

        if let Some(h) = self.recurring.get(&key) {
            results.push(h);
        }

        results
    }

    #[allow(dead_code)]
    pub fn winning_source(&self, key: &HolydayKey) -> Option<HolydaySource> {
        self.recurring.get(key).map(|h| h.source.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::date::types::{Season, Weekday};
    use crate::holydays::defaults::builtin_holydays;
    use crate::holydays::types::HolydaySource;

    fn make_holyday(
        key: HolydayKey,
        name: &str,
        source: HolydaySource,
        recurring: bool,
        year: Option<i32>,
    ) -> Holyday {
        Holyday {
            key,
            name: name.to_string(),
            description: None,
            greeting: None,
            apostle: None,
            recurring,
            year,
            source,
        }
    }

    fn chaos5_date(year: i32) -> DiscordianDate {
        DiscordianDate::SeasonDay {
            year,
            season: Season::Chaos,
            day: 5,
            weekday: Weekday::SettingOrange,
        }
    }

    #[test]
    fn test_builtin_lookup_mungday() {
        let registry = HolydayRegistry::build(builtin_holydays(), vec![], vec![]);
        let date = chaos5_date(3191);
        let results = registry.lookup(&date);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Mungday");
    }

    #[test]
    fn test_personal_overrides_default() {
        let key = HolydayKey::SeasonDay {
            season: Season::Chaos,
            day: 5,
        };
        let personal = vec![make_holyday(
            key,
            "My Custom Chaos 5",
            HolydaySource::Personal,
            true,
            None,
        )];
        let registry = HolydayRegistry::build(builtin_holydays(), vec![], personal);
        let date = chaos5_date(3191);
        let results = registry.lookup(&date);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "My Custom Chaos 5");
        assert_eq!(results[0].source, HolydaySource::Personal);
    }

    #[test]
    fn test_cabal_overrides_default() {
        let key = HolydayKey::SeasonDay {
            season: Season::Chaos,
            day: 5,
        };
        let cabal = vec![make_holyday(
            key,
            "Cabal Chaos 5",
            HolydaySource::Cabal,
            true,
            None,
        )];
        let registry = HolydayRegistry::build(builtin_holydays(), cabal, vec![]);
        let date = chaos5_date(3191);
        let results = registry.lookup(&date);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Cabal Chaos 5");
    }

    #[test]
    fn test_personal_overrides_cabal() {
        let key = HolydayKey::SeasonDay {
            season: Season::Chaos,
            day: 5,
        };
        let cabal = vec![make_holyday(
            key.clone(),
            "Cabal Version",
            HolydaySource::Cabal,
            true,
            None,
        )];
        let personal = vec![make_holyday(
            key,
            "Personal Version",
            HolydaySource::Personal,
            true,
            None,
        )];
        let registry = HolydayRegistry::build(builtin_holydays(), cabal, personal);
        let date = chaos5_date(3191);
        let results = registry.lookup(&date);
        assert_eq!(results[0].name, "Personal Version");
    }

    #[test]
    fn test_one_time_holyday_matches_year() {
        let key = HolydayKey::SeasonDay {
            season: Season::Discord,
            day: 5,
        };
        let personal = vec![make_holyday(
            key,
            "The Incident",
            HolydaySource::Personal,
            false,
            Some(3191),
        )];
        let registry = HolydayRegistry::build(builtin_holydays(), vec![], personal);

        let date_matching = DiscordianDate::SeasonDay {
            year: 3191,
            season: Season::Discord,
            day: 5,
            weekday: Weekday::SettingOrange,
        };
        let date_other_year = DiscordianDate::SeasonDay {
            year: 3192,
            season: Season::Discord,
            day: 5,
            weekday: Weekday::SettingOrange,
        };

        let results_match = registry.lookup(&date_matching);
        let results_other = registry.lookup(&date_other_year);

        assert_eq!(results_match.len(), 1);
        assert_eq!(results_match[0].name, "The Incident");

        // In 3192, the one-time is not found; falls back to recurring default (Mojoday)
        assert_eq!(results_other.len(), 1);
        assert_eq!(results_other[0].name, "Mojoday");
    }

    #[test]
    fn test_one_time_does_not_shadow_other_years() {
        let key = HolydayKey::SeasonDay {
            season: Season::Chaos,
            day: 5,
        };
        let personal = vec![make_holyday(
            key,
            "One Time Only",
            HolydaySource::Personal,
            false,
            Some(3191),
        )];
        let registry = HolydayRegistry::build(builtin_holydays(), vec![], personal);

        let date_3190 = chaos5_date(3190);
        let results = registry.lookup(&date_3190);
        // Should fall back to default Mungday
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Mungday");
    }

    #[test]
    fn test_no_holyday_on_plain_day() {
        let registry = HolydayRegistry::build(builtin_holydays(), vec![], vec![]);
        let date = DiscordianDate::SeasonDay {
            year: 3191,
            season: Season::Chaos,
            day: 10,
            weekday: Weekday::Sweetmorn,
        };
        let results = registry.lookup(&date);
        assert!(results.is_empty());
    }

    #[test]
    fn test_st_tibs_lookup() {
        let registry = HolydayRegistry::build(builtin_holydays(), vec![], vec![]);
        let date = DiscordianDate::StTibsDay { year: 3190 };
        let results = registry.lookup(&date);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "St. Tib's Day");
    }
}
