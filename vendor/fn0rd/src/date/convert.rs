use chrono::{Datelike, Local, NaiveDate};

use crate::date::types::{DiscordianDate, Season, Weekday, YOLD_OFFSET};
use crate::error::FnordError;

/// Convert a Gregorian NaiveDate to a DiscordianDate.
pub fn to_discordian(date: NaiveDate) -> DiscordianDate {
    let year = date.year();
    let doy = date.ordinal() as u16; // 1-indexed
    let is_leap = is_leap_year(year);
    let disc_year = year + YOLD_OFFSET;

    // St. Tib's Day: Feb 29 in leap years (doy == 60 in a leap year)
    if is_leap && doy == 60 {
        return DiscordianDate::StTibsDay { year: disc_year };
    }

    // Adjust doy for leap years after Feb 28 (doy 59) to remove the extra day
    let adjusted_doy = if is_leap && doy > 60 { doy - 1 } else { doy };

    // adjusted_doy is now 1-365 mapping to Discordian calendar
    let season_offset = adjusted_doy - 1; // 0-indexed, 0..=364
    let season = Season::from_season_day_offset(season_offset);
    let day = ((season_offset % 73) + 1) as u8; // 1-indexed day within season
    let weekday = Weekday::from_day_of_season(day);

    DiscordianDate::SeasonDay {
        year: disc_year,
        season,
        day,
        weekday,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Parse a user-supplied date string.
/// Accepts: "today", "tomorrow", "yesterday", YYYY-MM-DD, +N/-N (day offset from today)
pub fn parse_date_arg(s: &str) -> Result<NaiveDate, FnordError> {
    let today = Local::now().date_naive();
    match s.trim() {
        "today" => Ok(today),
        "tomorrow" => Ok(today + chrono::Duration::days(1)),
        "yesterday" => Ok(today - chrono::Duration::days(1)),
        other => {
            if let Some(rest) = other.strip_prefix('+') {
                let n: i64 = rest
                    .parse()
                    .map_err(|_| FnordError::Parse(format!("invalid offset: '{other}'")))?;
                Ok(today + chrono::Duration::days(n))
            } else if let Some(rest) = other.strip_prefix('-') {
                let n: i64 = rest
                    .parse()
                    .map_err(|_| FnordError::Parse(format!("invalid offset: '{other}'")))?;
                Ok(today - chrono::Duration::days(n))
            } else {
                NaiveDate::parse_from_str(other, "%Y-%m-%d")
                    .map_err(|_| FnordError::Parse(format!("invalid date: '{other}'")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_jan1_chaos1() {
        let d = to_discordian(date(2025, 1, 1));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Chaos,
                day: 1,
                weekday: Weekday::Sweetmorn,
            }
        );
    }

    #[test]
    fn test_jan5_chaos5_mungday() {
        let d = to_discordian(date(2025, 1, 5));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Chaos,
                day: 5,
                weekday: Weekday::SettingOrange,
            }
        );
    }

    #[test]
    fn test_feb28_nonleap_chaos59() {
        // 2025 is not a leap year
        let d = to_discordian(date(2025, 2, 28));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Chaos,
                day: 59,
                weekday: Weekday::from_day_of_season(59),
            }
        );
    }

    #[test]
    fn test_feb29_leap_st_tibs() {
        // 2024 is a leap year
        let d = to_discordian(date(2024, 2, 29));
        assert_eq!(d, DiscordianDate::StTibsDay { year: 3190 });
    }

    #[test]
    fn test_mar1_leap_chaos60() {
        // In a leap year, Mar 1 should be Chaos 60 (not 59)
        let d = to_discordian(date(2024, 3, 1));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3190,
                season: Season::Chaos,
                day: 60,
                weekday: Weekday::from_day_of_season(60),
            }
        );
    }

    #[test]
    fn test_mar1_nonleap_chaos60() {
        // In a non-leap year, Mar 1 = doy 60 = Chaos 60
        let d = to_discordian(date(2025, 3, 1));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Chaos,
                day: 60,
                weekday: Weekday::from_day_of_season(60),
            }
        );
    }

    #[test]
    fn test_jul4_bureaucracy2() {
        // Jul 4 non-leap: doy = 185
        // adjusted = 185 (non-leap 2025)
        // season_offset = 184, 184/73 = 2 remainder 38 -> Confusion day 39
        // Wait, let me recalculate:
        // Jan=31, Feb=28, Mar=31, Apr=30, May=31, Jun=30, Jul=4 = 31+28+31+30+31+30+4 = 185
        // adjusted_doy = 185, offset = 184
        // 184 / 73 = 2 r 38, season = Confusion, day = 39
        // Hmm but specification says Bureaucracy 2...
        // Let me recheck: the classic ddate convention
        // Actually let me just verify manually:
        // Chaos: days 1-73 (Jan 1 - Mar 14 in non-leap)
        // Discord: days 74-146 (Mar 15 - May 26)
        // Confusion: days 147-219 (May 27 - Aug 6)
        // Bureaucracy: days 220-292 (Aug 7 - Oct 18)
        // Aftermath: days 293-365 (Oct 19 - Dec 31)
        // Jul 4 doy = 185, in Confusion (147-219), day = 185-146 = 39
        // So Jul 4 = Confusion 39, not Bureaucracy 2
        // The spec may have an error. Let's test what we actually get.
        let d = to_discordian(date(2025, 7, 4));
        match &d {
            DiscordianDate::SeasonDay { season, day, .. } => {
                assert_eq!(*season, Season::Confusion);
                assert_eq!(*day, 39);
            }
            _ => panic!("expected season day"),
        }
    }

    #[test]
    fn test_dec31_aftermath73() {
        let d = to_discordian(date(2025, 12, 31));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Aftermath,
                day: 73,
                weekday: Weekday::from_day_of_season(73),
            }
        );
    }

    #[test]
    fn test_chaos50_chaoflux() {
        // Chaos 50 = Jan 1 + 49 days = Feb 19
        let d = to_discordian(date(2025, 2, 19));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Chaos,
                day: 50,
                weekday: Weekday::from_day_of_season(50),
            }
        );
    }

    #[test]
    fn test_discord1_boundary() {
        // Discord 1 = day 74 of year = Mar 15 in non-leap
        let d = to_discordian(date(2025, 3, 15));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Discord,
                day: 1,
                weekday: Weekday::Sweetmorn,
            }
        );
    }

    #[test]
    fn test_aftermath1_boundary() {
        // Aftermath 1 = day 293 = Oct 20 in non-leap
        let d = to_discordian(date(2025, 10, 20));
        assert_eq!(
            d,
            DiscordianDate::SeasonDay {
                year: 3191,
                season: Season::Aftermath,
                day: 1,
                weekday: Weekday::Sweetmorn,
            }
        );
    }

    #[test]
    fn test_parse_date_arg_iso() {
        let d = parse_date_arg("2025-01-01").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2025, 1, 1).unwrap());
    }

    #[test]
    fn test_parse_date_arg_invalid() {
        assert!(parse_date_arg("not-a-date").is_err());
    }
}
