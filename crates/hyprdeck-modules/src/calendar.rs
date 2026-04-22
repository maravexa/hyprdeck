//! Calendar module — displays the current date.
//!
//! Supports three display systems: Gregorian (default), Discordian, and Custom.
//! The popup renders a self-contained month/season grid using the table widget.

use chrono::{Datelike as _, Local, NaiveDate};
use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    PopupContent, PopupEventResult, Rect, Size, ThemeContext, UpdateContext,
};
use hyprdeck_core::widgets::{CellStyle, TableCell, TableConfig, compute_table_size, draw_table};

use crate::render_utils;

// ── Config ─────────────────────────────────────────────────────────────────────

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

// ── Module ─────────────────────────────────────────────────────────────────────

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
                    let dt = Local::now();
                    dt.format(fmt).to_string()
                } else {
                    date.format("%A, %B %-d, %Y").to_string()
                }
            }
            CalendarSystem::Discordian => to_discordian(date),
            CalendarSystem::Custom => {
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

// ── Discordian calendar (display string) ───────────────────────────────────────

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Convert a Gregorian date to its Discordian equivalent as a display string.
///
/// Reference: <https://en.wikipedia.org/wiki/Discordian_calendar>
fn to_discordian(date: NaiveDate) -> String {
    let year = date.year() + 1166;
    let doy = date.ordinal() as i32;
    let leap = is_leap_year(date.year());

    if leap && date.month() == 2 && date.day() == 29 {
        return format!("St. Tib's Day, YOLD {year}");
    }

    let doy = if leap && doy > 60 { doy - 1 } else { doy };

    const SEASONS: [&str; 5] = ["Chaos", "Discord", "Confusion", "Bureaucracy", "The Aftermath"];
    const DAYS: [&str; 5] = ["Sweetmorn", "Boomtime", "Pungenday", "Prickle-Prickle", "Setting Orange"];

    let idx = doy - 1;
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
        let font = theme
            .fonts
            .bold_family
            .as_deref()
            .unwrap_or(&theme.fonts.family);
        render_utils::draw_text_centered(
            canvas,
            &self.display_text,
            bounds,
            font,
            font_size,
            theme.colors.foreground,
        );
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
        EventResult::Ignored
    }

    fn has_popup(&self) -> bool {
        tracing::debug!("{} has_popup called → true", self.id());
        true
    }

    fn popup_content(&self) -> Option<Box<dyn PopupContent>> {
        tracing::debug!("{} popup_content called", self.id());
        Some(Box::new(CalendarPopup::new(&self.config.system)))
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

// ── Gregorian calendar data ────────────────────────────────────────────────────

fn gregorian_month_table(year: i32, month: u32, today: Option<u32>) -> (TableConfig, Vec<Vec<TableCell>>) {
    let config = TableConfig {
        columns: 7,
        column_headers: ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        title: Some(format!("{} {}", gregorian_month_name(month), year)),
        cell_width: 32.0,
        cell_height: 26.0,
        title_height: 30.0,
        header_height: 24.0,
        cell_font_size: 13.0,
        header_font_size: 11.0,
        title_font_size: 14.0,
        highlight_radius: 4.0,
    };

    let first_day = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let first_weekday = first_day.weekday().num_days_from_sunday() as usize;
    let days_in_month = days_in_gregorian_month(year, month);

    let mut rows: Vec<Vec<TableCell>> = Vec::new();
    let mut current_row: Vec<TableCell> = Vec::new();

    // Leading blanks
    for _ in 0..first_weekday {
        current_row.push(TableCell::default());
    }

    // Day cells
    for day in 1..=days_in_month {
        let style = if today == Some(day) {
            CellStyle::Highlighted
        } else {
            CellStyle::Normal
        };
        current_row.push(TableCell { text: day.to_string(), style });
        if current_row.len() == 7 {
            rows.push(current_row);
            current_row = Vec::new();
        }
    }

    // Trailing blanks to fill the last row
    if !current_row.is_empty() {
        while current_row.len() < 7 {
            current_row.push(TableCell::default());
        }
        rows.push(current_row);
    }

    (config, rows)
}

fn days_in_gregorian_month(year: i32, month: u32) -> u32 {
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
    };
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    next.unwrap().signed_duration_since(first).num_days() as u32
}

fn gregorian_month_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Unknown",
    }
}

// ── Discordian calendar data ───────────────────────────────────────────────────

const DISCORDIAN_SEASONS: [&str; 5] =
    ["Chaos", "Discord", "Confusion", "Bureaucracy", "The Aftermath"];

const DISCORDIAN_WEEKDAYS: [&str; 5] = ["SM", "BT", "PD", "PP", "SO"];

fn discordian_season_table(
    year: i32,
    season_index: usize,
    today_season_day: Option<u32>,
) -> (TableConfig, Vec<Vec<TableCell>>) {
    let yold = year + 1166;

    let config = TableConfig {
        columns: 5,
        column_headers: DISCORDIAN_WEEKDAYS.iter().map(|s| s.to_string()).collect(),
        title: Some(format!("{}, YOLD {}", DISCORDIAN_SEASONS[season_index], yold)),
        cell_width: 38.0,
        cell_height: 26.0,
        title_height: 30.0,
        header_height: 24.0,
        cell_font_size: 13.0,
        header_font_size: 11.0,
        title_font_size: 14.0,
        highlight_radius: 4.0,
    };

    let mut rows: Vec<Vec<TableCell>> = Vec::new();
    let mut current_row: Vec<TableCell> = Vec::new();

    for day in 1..=73u32 {
        let style = if today_season_day == Some(day) {
            CellStyle::Highlighted
        } else {
            CellStyle::Normal
        };
        current_row.push(TableCell { text: day.to_string(), style });
        if current_row.len() == 5 {
            rows.push(current_row);
            current_row = Vec::new();
        }
    }

    // Trailing blanks (73 = 14×5 + 3, so last row has 3 days + 2 blanks)
    if !current_row.is_empty() {
        while current_row.len() < 5 {
            current_row.push(TableCell::default());
        }
        rows.push(current_row);
    }

    (config, rows)
}

/// Compute the current Discordian season index (0–4) and day within that season (1–73).
///
/// On St. Tib's Day (Feb 29 of a leap year) returns (0, 59) as a best-effort
/// approximation — the intercalary day sits between Chaos 59 and Chaos 60.
fn gregorian_to_discordian_season(date: NaiveDate) -> (usize, u32) {
    let doy = date.ordinal(); // 1-based
    let leap = is_leap_year(date.year());

    if leap && date.month() == 2 && date.day() == 29 {
        return (0, 59);
    }

    let doy = if leap && doy > 60 { doy - 1 } else { doy };

    let season_index = ((doy - 1) / 73) as usize;
    let season_day = ((doy - 1) % 73) + 1;
    (season_index, season_day)
}

// ── Calendar popup ─────────────────────────────────────────────────────────────

/// Popup content for the calendar module — renders a self-contained grid using the table widget.
pub struct CalendarPopup {
    config: TableConfig,
    rows: Vec<Vec<TableCell>>,
}

impl CalendarPopup {
    pub fn new(system: &CalendarSystem) -> Self {
        let now = Local::now();
        let (config, rows) = match system {
            CalendarSystem::Gregorian => gregorian_month_table(
                now.year(),
                now.month(),
                Some(now.day()),
            ),
            CalendarSystem::Discordian => {
                let date = now.date_naive();
                let (season_idx, season_day) = gregorian_to_discordian_season(date);
                discordian_season_table(now.year(), season_idx, Some(season_day))
            }
            CalendarSystem::Custom => gregorian_month_table(
                now.year(),
                now.month(),
                Some(now.day()),
            ),
        };
        Self { config, rows }
    }
}

impl PopupContent for CalendarPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        let table_size = compute_table_size(&self.config, self.rows.len());
        Size::new(table_size.width + 24.0, table_size.height + 24.0)
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let bold_font = theme.fonts.bold_family.as_deref().unwrap_or(&theme.fonts.family);
        let mono_font = theme.fonts.mono_family.as_deref().unwrap_or(&theme.fonts.family);

        let fg = theme.colors.foreground;
        let header_color = [fg[0], fg[1], fg[2], (fg[3] as f32 * 0.5) as u8];

        draw_table(
            canvas,
            bounds,
            &self.config,
            &self.rows,
            theme.colors.foreground,
            theme.colors.background,
            theme.colors.accent,
            header_color,
            theme.colors.foreground,
            &theme.fonts.family,
            bold_font,
            mono_font,
        );
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> PopupEventResult {
        PopupEventResult::Ignored
    }

    fn update(&mut self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    // ── Discordian display string ──────────────────────────────────────────────

    #[test]
    fn discordian_jan1_2024() {
        let s = to_discordian(date(2024, 1, 1));
        assert_eq!(s, "Sweetmorn, Chaos 1, YOLD 3190");
    }

    #[test]
    fn discordian_st_tibs_day_2024() {
        let s = to_discordian(date(2024, 2, 29));
        assert_eq!(s, "St. Tib's Day, YOLD 3190");
    }

    #[test]
    fn discordian_dec31_2024() {
        let s = to_discordian(date(2024, 12, 31));
        assert_eq!(s, "Setting Orange, The Aftermath 73, YOLD 3190");
    }

    #[test]
    fn discordian_non_leap_mar1() {
        let s = to_discordian(date(2023, 3, 1));
        assert!(s.contains("Chaos"), "expected Chaos season, got: {s}");
    }

    #[test]
    fn discordian_leap_mar1_shifts_correctly() {
        let s = to_discordian(date(2024, 3, 1));
        let s_non_leap = to_discordian(date(2023, 3, 1));
        let remove_yold = |s: &str| -> String { s.split(", YOLD").next().unwrap_or("").to_owned() };
        assert_eq!(remove_yold(&s), remove_yold(&s_non_leap));
    }

    // ── Gregorian table data ───────────────────────────────────────────────────

    #[test]
    fn gregorian_april_2026() {
        let (config, rows) = gregorian_month_table(2026, 4, Some(15));
        assert_eq!(config.columns, 7);
        assert_eq!(config.title, Some("April 2026".into()));
        // April 1, 2026 is a Wednesday (index 3 from Sunday)
        assert_eq!(rows[0][0].text, ""); // Sun
        assert_eq!(rows[0][1].text, ""); // Mon
        assert_eq!(rows[0][2].text, ""); // Tue
        assert_eq!(rows[0][3].text, "1"); // Wed
        // Day 15 should be highlighted
        let day15 = rows.iter().flatten().find(|c| c.text == "15").unwrap();
        assert!(matches!(day15.style, CellStyle::Highlighted));
    }

    #[test]
    fn gregorian_february_leap_year() {
        let (_, rows) = gregorian_month_table(2024, 2, Some(29));
        let all_days: Vec<&str> = rows
            .iter()
            .flatten()
            .filter(|c| !c.text.is_empty())
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(all_days.last().unwrap(), &"29");
    }

    // ── Discordian table data ──────────────────────────────────────────────────

    #[test]
    fn discordian_season_has_73_days() {
        let (config, rows) = discordian_season_table(2026, 0, Some(1));
        assert_eq!(config.columns, 5);
        let all_days: Vec<&str> = rows
            .iter()
            .flatten()
            .filter(|c| !c.text.is_empty())
            .map(|c| c.text.as_str())
            .collect();
        assert_eq!(all_days.len(), 73);
        assert_eq!(all_days[0], "1");
        assert_eq!(all_days[72], "73");
    }

    #[test]
    fn gregorian_to_discordian_jan_1() {
        let (season, day) = gregorian_to_discordian_season(date(2026, 1, 1));
        assert_eq!(season, 0); // Chaos
        assert_eq!(day, 1);
    }

    #[test]
    fn gregorian_to_discordian_dec_31() {
        let (season, day) = gregorian_to_discordian_season(date(2026, 12, 31));
        assert_eq!(season, 4); // The Aftermath
        assert_eq!(day, 73);
    }

    #[test]
    fn days_in_month_correct() {
        assert_eq!(days_in_gregorian_month(2026, 2), 28);
        assert_eq!(days_in_gregorian_month(2024, 2), 29); // leap
        assert_eq!(days_in_gregorian_month(2026, 4), 30);
        assert_eq!(days_in_gregorian_month(2026, 1), 31);
    }

    // ── Module behaviour ──────────────────────────────────────────────────────

    #[test]
    fn update_returns_true_on_date_change() {
        let mut m = CalendarModule::new(CalendarConfig::default());
        let state = hyprdeck_core::HyprState::default();
        let t1 = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2024, 3, 1, 0, 0, 0).unwrap();
        let t2 = chrono::TimeZone::with_ymd_and_hms(&chrono::Local, 2024, 3, 2, 0, 0, 0).unwrap();
        let ctx1 = UpdateContext { now: t1, hypr_state: &state, output_name: "" };
        let ctx2 = UpdateContext { now: t2, hypr_state: &state, output_name: "" };
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
