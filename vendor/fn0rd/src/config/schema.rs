use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_week_start() -> String {
    "sweetmorn".to_string()
}

fn default_units() -> String {
    "discordian".to_string()
}

fn default_provider() -> String {
    "wttr.in".to_string()
}

fn default_offline_mode() -> String {
    "generative".to_string()
}

fn default_body() -> String {
    "luna".to_string()
}

fn default_system() -> String {
    "western".to_string()
}

fn default_log_format() -> String {
    "plaintext".to_string()
}

fn default_log_path() -> String {
    "~/.config/eris/grimoire".to_string()
}

fn default_timestamp_style() -> String {
    "discordian".to_string()
}

fn default_fnord_replacement() -> String {
    "FNORD".to_string()
}

fn default_fnord_rate() -> f64 {
    0.03
}

fn default_preserve_structure() -> bool {
    true
}

fn default_color() -> String {
    "auto".to_string()
}

fn default_unicode() -> String {
    "auto".to_string()
}

fn default_pager() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct IdentityConfig {
    #[serde(default)]
    pub pope_title: String,
    #[serde(default)]
    pub sect_name: String,
    #[serde(default)]
    pub cabal: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CalendarConfig {
    #[serde(default)]
    pub holyday_files: Vec<String>,
    #[serde(default = "default_true")]
    pub show_apostle: bool,
    #[serde(default = "default_true")]
    pub show_season: bool,
    #[serde(default = "default_true")]
    pub show_holyday: bool,
    #[serde(default = "default_week_start")]
    pub week_start: String,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            holyday_files: vec![],
            show_apostle: true,
            show_season: true,
            show_holyday: true,
            week_start: "sweetmorn".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WeatherConfig {
    #[serde(default)]
    pub location: String,
    #[serde(default = "default_units")]
    pub units: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_offline_mode")]
    pub offline_mode: String,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            location: String::new(),
            units: "discordian".to_string(),
            provider: "wttr.in".to_string(),
            offline_mode: "generative".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoonConfig {
    #[serde(default = "default_body")]
    pub body: String,
    #[serde(default = "default_true")]
    pub show_emoji: bool,
    #[serde(default = "default_true")]
    pub show_phase_name: bool,
}

impl Default for MoonConfig {
    fn default() -> Self {
        Self {
            body: "luna".to_string(),
            show_emoji: true,
            show_phase_name: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ZodiacConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_system")]
    pub system: String,
    #[serde(default)]
    pub show_with_date: bool,
}

impl Default for ZodiacConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            system: "western".to_string(),
            show_with_date: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FortuneConfig {
    #[serde(default = "default_true")]
    pub builtin: bool,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default = "default_true")]
    pub weight_by_holyday: bool,
    #[serde(default = "default_true")]
    pub weight_by_season: bool,
    #[serde(default)]
    pub offensive: bool,
}

impl Default for FortuneConfig {
    fn default() -> Self {
        Self {
            builtin: true,
            files: vec![],
            weight_by_holyday: true,
            weight_by_season: true,
            offensive: false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogConfig {
    #[serde(default = "default_log_path")]
    pub path: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    #[serde(default = "default_timestamp_style")]
    pub timestamp_style: String,
    #[serde(default)]
    pub append_fortune: bool,
    #[serde(default)]
    pub append_omens: bool,
    #[serde(default)]
    pub editor: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            path: "~/.config/eris/grimoire".to_string(),
            format: "plaintext".to_string(),
            timestamp_style: "discordian".to_string(),
            append_fortune: false,
            append_omens: false,
            editor: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FnordConfig {
    #[serde(default = "default_fnord_replacement")]
    pub replacement: String,
    #[serde(default = "default_fnord_rate")]
    pub rate: f64,
    #[serde(default = "default_preserve_structure")]
    pub preserve_structure: bool,
    #[serde(default)]
    pub seed: String,
}

impl Default for FnordConfig {
    fn default() -> Self {
        Self {
            replacement: "FNORD".to_string(),
            rate: 0.03,
            preserve_structure: true,
            seed: String::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_unicode")]
    pub unicode: String,
    #[serde(default = "default_pager")]
    pub pager: String,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            color: "auto".to_string(),
            unicode: "auto".to_string(),
            pager: "auto".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(default)]
    pub identity: IdentityConfig,
    #[serde(default)]
    pub calendar: CalendarConfig,
    #[serde(default)]
    pub weather: WeatherConfig,
    #[serde(default)]
    pub moon: MoonConfig,
    #[serde(default)]
    pub zodiac: ZodiacConfig,
    #[serde(default)]
    pub fortune: FortuneConfig,
    #[serde(default)]
    pub log: LogConfig,
    #[serde(default)]
    pub fnord: FnordConfig,
    #[serde(default)]
    pub output: OutputConfig,
}
