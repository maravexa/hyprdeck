//! Calendar module — displays the current date.
//!
//! Supports three display systems: Gregorian (default), Discordian, and Custom.
//! A popup month-grid view is left as a future enhancement (`// TODO` below).

use chrono::{Datelike as _, Local, NaiveDate};
use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Calendar system used for date display.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarSystem {
    #[default]
    Gregorian,
    Discordian,
    Custom,
}

/// Configuration for the calendar / date display module.
#[derive(Debug, Default, Deserialize)]
pub struct CalendarConfig {
    /// Calendar system to display.
    #[serde(default)]
    pub system: CalendarSystem,
    /// Show week numbers in the grid.
    #[serde(default)]
    pub show_week_numbers: bool,
    /// First day of the week: "monday" or "sunday".
    #[serde(default = "default_first_day")]
    pub first_day: String,
    /// Custom strftime format for Gregorian display (overrides default).
    pub date_format: Option<String>,
}

fn default_first_day() -> String {
    "monday".to_owned()
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the calendar / date display module.
pub struct CalendarModule {
    config: CalendarConfig,
    /// Cached display string (re-computed only when the date changes).
    display_text: String,
    /// Date at which `display_text` was last computed.
    last_date: Option<NaiveDate>,
}

impl CalendarModule {
    pub fn new(config: CalendarConfig) -> Self {
        CalendarModule {
            config,
            display_text: String::new(),
            last_date: None,
        }
    }

    fn compute_display(config: &CalendarConfig, date: NaiveDate) -> String {
        match config.system {
            CalendarSystem::Gregorian => {
                if let Some(fmt) = &config.date_format {
                    // Use the custom strftime format via chrono.
                    let dt = Local::now(); // approximate — date is correct, time irrelevant
                    dt.format(fmt).to_string()
                } else {
                    date.format("%A, %B %-d, %Y").to_string()
                }
            }
            CalendarSystem::Discordian => to_discordian(date),
            CalendarSystem::Custom => {
                // Custom system uses the date_format if provided, otherwise falls
                // back to a short ISO date.
                if let Some(fmt) = &config.date_format {
                    let dt = Local::now();
                    dt.format(fmt).to_string()
                } else {
                    date.format("%Y-%m-%d").to_string()
                }
            }
        }
    }
}

// ── Discordian calendar ───────────────────────────────────────────────────────

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Convert a Gregorian date to its Discordian equivalent as a display string.
///
/// Reference: <https://en.wikipedia.org/wiki/Discordian_calendar>
fn to_discordian(date: NaiveDate) -> String {
    let year = date.year() + 1166; // Year of Our Lady of Discord offset
    let doy = date.ordinal() as i32; // 1-based
    let leap = is_leap_year(date.year());

    // St. Tib's Day: intercalary day on Gregorian Feb 29.
    if leap && date.month() == 2 && date.day() == 29 {
        return format!("St. Tib's Day, YOLD {year}");
    }

    // After St. Tib's Day on a leap year, shift back one day so that the
    // 73-day season arithmetic stays correct.
    let doy = if leap && doy > 60 { doy - 1 } else { doy };

    const SEASONS: [&str; 5] = [
        "Chaos",
        "Discord",
        "Confusion",
        "Bureaucracy",
        "The Aftermath",
    ];
    const DAYS: [&str; 5] = [
        "Sweetmorn",
        "Boomtime",
        "Pungenday",
        "Prickle-Prickle",
        "Setting Orange",
    ];

    let idx = doy - 1; // 0-based
    let season = SEASONS[(idx / 73) as usize];
    let season_day = (idx % 73) + 1;
    let weekday = DAYS[(idx % 5) as usize];

    format!("{weekday}, {season} {season_day}, YOLD {year}")
}

impl PanelModule for CalendarModule {
    fn id(&self) -> &str {
        "calendar"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let sample = if self.display_text.is_empty() {
            "Wednesday, January 01, 2000"
        } else {
            self.display_text.as_str()
        };
        let h = theme.fonts.size + theme.padding.top + theme.padding.bottom;
        let font_size = render_utils::effective_font_size(h, theme.fonts.size);
        let w = render_utils::estimate_text_width(sample, font_size)
            + theme.padding.left
            + theme.padding.right;
        Size::new(w, h)
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        let today = ctx.now.date_naive();
        if self.last_date == Some(today) {
            return false;
        }
        let new_text = Self::compute_display(&self.config, today);
        self.last_date = Some(today);
        if new_text != self.display_text {
            self.display_text = new_text;
            true
        } else {
            false
        }
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font_size = render_utils::effective_font_size(bounds.height, theme.fonts.size);
        render_utils::draw_text_centered(
            canvas,
            &self.display_text,
            bounds,
            &theme.fonts.family,
            font_size,
            theme.colors.foreground,
        );
        // TODO: on click, toggle an expanded month-grid popup.
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
        // TODO: toggle expanded month view on click.
        EventResult::Ignored
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "system".to_owned(),
                    label: "Calendar system".to_owned(),
                    description: "Which calendar system to display.".to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec![
                            "gregorian".to_owned(),
                            "discordian".to_owned(),
                            "custom".to_owned(),
                        ],
                        default: "gregorian".to_owned(),
                    },
                },
                ConfigField {
                    key: "date_format".to_owned(),
                    label: "Date format".to_owned(),
                    description: "Optional strftime format string for the date display.".to_owned(),
                    field_type: ConfigFieldType::Text {
                        default: String::new(),
                    },
                },
                ConfigField {
                    key: "show_week_numbers".to_owned(),
                    label: "Show week numbers".to_owned(),
                    description: "Display ISO week numbers in the calendar grid.".to_owned(),
                    field_type: ConfigFieldType::Boolean { default: false },
                },
                ConfigField {
                    key: "first_day".to_owned(),
                    label: "First day of week".to_owned(),
                    description: "Whether the week starts on Monday or Sunday.".to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec!["monday".to_owned(), "sunday".to_owned()],
                        default: "monday".to_owned(),
                    },
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // ── Discordian conversion ─────────────────────────────────────────────────

    #[test]
    fn discordian_jan1_2024() {
        // Jan 1, 2024 → Sweetmorn, Chaos 1, YOLD 3190
        let s = to_discordian(date(2024, 1, 1));
        assert_eq!(s, "Sweetmorn, Chaos 1, YOLD 3190");
    }

    #[test]
    fn discordian_st_tibs_day_2024() {
        // Feb 29, 2024 (leap year) → St. Tib's Day
        let s = to_discordian(date(2024, 2, 29));
        assert_eq!(s, "St. Tib's Day, YOLD 3190");
    }

    #[test]
    fn discordian_dec31_2024() {
        // Dec 31, 2024 → Setting Orange, The Aftermath 73, YOLD 3190
        let s = to_discordian(date(2024, 12, 31));
        assert_eq!(s, "Setting Orange, The Aftermath 73, YOLD 3190");
    }

    #[test]
    fn discordian_non_leap_mar1() {
        // On a non-leap year, Mar 1 is day 60.  Season: Chaos 60.
        // Chaos covers days 1-73.  Day 60 → Chaos 60.
        // Weekday: day 59 (0-based) % 5 = 4 → "Setting Orange"
        let s = to_discordian(date(2023, 3, 1));
        assert!(s.contains("Chaos"), "expected Chaos season, got: {s}");
    }

    #[test]
    fn discordian_leap_mar1_shifts_correctly() {
        // On a leap year, Mar 1 is day 61, but after St. Tib's Day adjustment
        // it becomes day 60 in the Discordian count.
        let s = to_discordian(date(2024, 3, 1));
        // Should have the same Discordian day as a non-leap year Mar 1.
        let s_non_leap = to_discordian(date(2023, 3, 1));
        // The YOLD will differ (+1 for 2024 vs 2023), but season/weekday should match.
        let remove_yold = |s: &str| -> String { s.split(", YOLD").next().unwrap_or("").to_owned() };
        assert_eq!(remove_yold(&s), remove_yold(&s_non_leap));
    }

    // ── Module behaviour ──────────────────────────────────────────────────────

    #[test]
    fn update_returns_true_on_date_change() {
        let mut m = CalendarModule::new(CalendarConfig::default());
        let state = hyprdeck_core::HyprState::default();
        let t1 = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2024, 3, 1, 0, 0, 0).unwrap();
        let t2 = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2024, 3, 2, 0, 0, 0).unwrap();
        let ctx1 = UpdateContext {
            now: t1,
            hypr_state: &state,
        };
        let ctx2 = UpdateContext {
            now: t2,
            hypr_state: &state,
        };
        assert!(m.update(&ctx1), "first update should return true");
        assert!(!m.update(&ctx1), "same day should return false");
        assert!(m.update(&ctx2), "next day should return true");
    }

    #[test]
    fn handle_event_ignored() {
        let mut m = CalendarModule::new(CalendarConfig::default());
        let r = m.handle_event(
            &InputEvent::MousePress {
                x: 0.0,
                y: 0.0,
                button: hyprdeck_core::MouseButton::Left,
            },
            Rect::new(0.0, 0.0, 100.0, 32.0),
        );
        assert!(matches!(r, EventResult::Ignored));
    }
}
