use std::fmt;

use crate::holydays::types::HolydayKey;

pub const YOLD_OFFSET: i32 = 1166;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Season {
    Chaos,
    Discord,
    Confusion,
    Bureaucracy,
    Aftermath,
}

impl Season {
    #[allow(dead_code)]
    pub fn apostle(&self) -> &'static str {
        match self {
            Season::Chaos => "Hung Mung",
            Season::Discord => "Dr. Van Van Mojo",
            Season::Confusion => "Sri Syadasti",
            Season::Bureaucracy => "Zarathud",
            Season::Aftermath => "The Elder Malaclypse",
        }
    }

    #[allow(dead_code)]
    pub fn ordinal(&self) -> u8 {
        match self {
            Season::Chaos => 1,
            Season::Discord => 2,
            Season::Confusion => 3,
            Season::Bureaucracy => 4,
            Season::Aftermath => 5,
        }
    }

    /// Returns the (start_doy, end_doy) in a non-leap year (1-indexed, inclusive).
    /// St. Tib's Day (Feb 29 in leap years) inserts between doy 59 and 60 but
    /// doesn't belong to any season, so seasons 3-5 shift by 1 in leap years.
    /// This returns the base (non-leap) ranges.
    #[allow(dead_code)]
    pub fn day_range(&self) -> (u16, u16) {
        match self {
            Season::Chaos => (1, 73),
            Season::Discord => (74, 146),
            Season::Confusion => (147, 219),
            Season::Bureaucracy => (220, 292),
            Season::Aftermath => (293, 365),
        }
    }

    pub fn from_season_day_offset(offset: u16) -> Self {
        // offset is 0-indexed (0 = Chaos 1, 364 = Aftermath 73)
        match offset / 73 {
            0 => Season::Chaos,
            1 => Season::Discord,
            2 => Season::Confusion,
            3 => Season::Bureaucracy,
            _ => Season::Aftermath,
        }
    }

    /// Returns all seasons in order
    pub fn all() -> [Season; 5] {
        [
            Season::Chaos,
            Season::Discord,
            Season::Confusion,
            Season::Bureaucracy,
            Season::Aftermath,
        ]
    }
}

impl fmt::Display for Season {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Season::Chaos => "Chaos",
            Season::Discord => "Discord",
            Season::Confusion => "Confusion",
            Season::Bureaucracy => "Bureaucracy",
            Season::Aftermath => "Aftermath",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weekday {
    Sweetmorn,
    Boomtime,
    Pungenday,
    PricklePrickle,
    SettingOrange,
}

impl Weekday {
    /// day is 1-indexed day of season (1..=73)
    pub fn from_day_of_season(day: u8) -> Weekday {
        match (day - 1) % 5 {
            0 => Weekday::Sweetmorn,
            1 => Weekday::Boomtime,
            2 => Weekday::Pungenday,
            3 => Weekday::PricklePrickle,
            _ => Weekday::SettingOrange,
        }
    }

    #[allow(dead_code)]
    pub fn ordinal(&self) -> u8 {
        match self {
            Weekday::Sweetmorn => 0,
            Weekday::Boomtime => 1,
            Weekday::Pungenday => 2,
            Weekday::PricklePrickle => 3,
            Weekday::SettingOrange => 4,
        }
    }
}

impl fmt::Display for Weekday {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Weekday::Sweetmorn => "Sweetmorn",
            Weekday::Boomtime => "Boomtime",
            Weekday::Pungenday => "Pungenday",
            Weekday::PricklePrickle => "Prickle-Prickle",
            Weekday::SettingOrange => "Setting Orange",
        };
        write!(f, "{name}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscordianDate {
    SeasonDay {
        year: i32,
        season: Season,
        day: u8,
        weekday: Weekday,
    },
    StTibsDay {
        year: i32,
    },
}

impl DiscordianDate {
    pub fn year(&self) -> i32 {
        match self {
            DiscordianDate::SeasonDay { year, .. } => *year,
            DiscordianDate::StTibsDay { year } => *year,
        }
    }

    #[allow(dead_code)]
    pub fn is_st_tibs(&self) -> bool {
        matches!(self, DiscordianDate::StTibsDay { .. })
    }

    pub fn holyday_key(&self) -> HolydayKey {
        match self {
            DiscordianDate::StTibsDay { .. } => HolydayKey::StTibs,
            DiscordianDate::SeasonDay { season, day, .. } => HolydayKey::SeasonDay {
                season: *season,
                day: *day,
            },
        }
    }
}

pub fn ordinal_suffix(n: u8) -> &'static str {
    match n % 100 {
        11..=13 => "th",
        _ => match n % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        },
    }
}

impl fmt::Display for DiscordianDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiscordianDate::StTibsDay { year } => {
                write!(f, "St. Tib's Day, in the YOLD {year}")
            }
            DiscordianDate::SeasonDay {
                year,
                season,
                day,
                weekday,
            } => {
                let suf = ordinal_suffix(*day);
                write!(
                    f,
                    "{weekday}, the {day}{suf} of {season}, in the YOLD {year}"
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weekday_cycling() {
        assert_eq!(Weekday::from_day_of_season(1), Weekday::Sweetmorn);
        assert_eq!(Weekday::from_day_of_season(2), Weekday::Boomtime);
        assert_eq!(Weekday::from_day_of_season(3), Weekday::Pungenday);
        assert_eq!(Weekday::from_day_of_season(4), Weekday::PricklePrickle);
        assert_eq!(Weekday::from_day_of_season(5), Weekday::SettingOrange);
        assert_eq!(Weekday::from_day_of_season(6), Weekday::Sweetmorn);
        assert_eq!(Weekday::from_day_of_season(73), Weekday::Pungenday); // (73-1)%5 = 72%5 = 2
    }

    #[test]
    fn test_ordinal_suffix() {
        assert_eq!(ordinal_suffix(1), "st");
        assert_eq!(ordinal_suffix(2), "nd");
        assert_eq!(ordinal_suffix(3), "rd");
        assert_eq!(ordinal_suffix(4), "th");
        assert_eq!(ordinal_suffix(11), "th");
        assert_eq!(ordinal_suffix(12), "th");
        assert_eq!(ordinal_suffix(13), "th");
        assert_eq!(ordinal_suffix(21), "st");
        assert_eq!(ordinal_suffix(22), "nd");
    }

    #[test]
    fn test_season_display() {
        assert_eq!(Season::Chaos.to_string(), "Chaos");
        assert_eq!(Season::Aftermath.to_string(), "Aftermath");
    }

    #[test]
    fn test_discordian_date_display() {
        let d = DiscordianDate::SeasonDay {
            year: 3191,
            season: Season::Chaos,
            day: 1,
            weekday: Weekday::Sweetmorn,
        };
        assert_eq!(
            d.to_string(),
            "Sweetmorn, the 1st of Chaos, in the YOLD 3191"
        );
    }

    #[test]
    fn test_st_tibs_display() {
        let d = DiscordianDate::StTibsDay { year: 3190 };
        assert_eq!(d.to_string(), "St. Tib's Day, in the YOLD 3190");
    }
}
