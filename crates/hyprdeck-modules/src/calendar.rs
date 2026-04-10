use hyprdeck_core::Pixmap;
use serde::Deserialize;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext,
};

/// Calendar system used for display.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CalendarSystem {
    #[default]
    Gregorian,
    Discordian,
    Custom,
}

/// Configuration for the calendar pop-up module.
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
}

fn default_first_day() -> String {
    "monday".to_owned()
}

/// Runtime state for the calendar module.
pub struct CalendarModule {
    config: CalendarConfig,
    /// Whether the calendar pop-up is currently expanded.
    expanded: bool,
    /// Cached current month/day for dirty-checking.
    last_day: Option<chrono::NaiveDate>,
}

impl CalendarModule {
    pub fn new(config: CalendarConfig) -> Self {
        CalendarModule {
            config,
            expanded: false,
            last_day: None,
        }
    }
}

impl PanelModule for CalendarModule {
    fn id(&self) -> &str {
        "calendar"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        todo!()
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        todo!()
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        todo!()
    }

    fn handle_event(&mut self, event: &InputEvent, bounds: Rect) -> EventResult {
        todo!()
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
