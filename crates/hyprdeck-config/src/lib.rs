//! Lightweight, versioned configuration contract for HyprDeck and its editors.
//!
//! This crate deliberately has no Wayland or rendering dependencies. Consumers
//! such as HyprCube can load, validate, and atomically save HyprDeck settings
//! without importing the panel runtime.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Version of the serialized configuration/schema integration contract.
pub const CONFIG_CONTRACT_VERSION: u32 = 1;

/// Errors produced while locating, loading, validating, or saving a config.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Neither `XDG_CONFIG_HOME` nor `HOME` is set in the environment.
    #[error("cannot determine config directory: neither XDG_CONFIG_HOME nor HOME is set")]
    NoBaseDir,
    /// Config file not found at the resolved path.
    #[error("config not found at {0}")]
    NotFound(PathBuf),
    /// Reading or writing a config failed.
    #[error("config I/O failed for {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// TOML could not be decoded.
    #[error("invalid HyprDeck TOML: {0}")]
    Decode(#[from] toml::de::Error),
    /// Config could not be encoded.
    #[error("could not encode HyprDeck TOML: {0}")]
    Encode(#[from] toml::ser::Error),
    /// Saving was refused because validation found errors.
    #[error("configuration has validation errors")]
    Validation(Vec<ConfigDiagnostic>),
}

/// Backward-compatible name retained for existing callers.
pub type ConfigPathError = ConfigError;

/// Return the canonical path for `hyprdeck.toml`.
pub fn default_config_path() -> Result<PathBuf, ConfigError> {
    resolve_config_path(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

fn resolve_config_path(
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
) -> Result<PathBuf, ConfigError> {
    let config_dir = if let Some(xdg) = xdg_config_home.filter(|value| !value.is_empty()) {
        PathBuf::from(xdg)
    } else if let Some(home) = home.filter(|value| !value.is_empty()) {
        PathBuf::from(home).join(".config")
    } else {
        return Err(ConfigError::NoBaseDir);
    };
    Ok(config_dir.join("hypr").join("hyprdeck.toml"))
}

/// Top-level user configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// Name of the selected embedded or user theme.
    pub theme: String,
    /// Per-field overrides applied on top of the selected theme.
    #[serde(default)]
    pub theme_overrides: ThemeOverrides,
    /// Module configuration keyed by stable module ID.
    #[serde(default)]
    pub modules: ModuleConfigs,
    /// Desktop-notification daemon and surface placement settings.
    #[serde(default)]
    pub notifications: NotificationConfig,
    /// Data introduced by newer consumers is retained across a load/save cycle.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

/// Placement and behaviour of HyprDeck's optional desktop-notification daemon.
///
/// The daemon is disabled by default so installing HyprDeck does not compete
/// with an already-running notification daemon. Set `enabled = true` to claim
/// `org.freedesktop.Notifications` while HyprDeck is running.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NotificationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub anchor: NotificationAnchor,
    /// `focused`, `primary`, or the connector name of a specific output.
    #[serde(default = "default_notification_monitor")]
    pub monitor: String,
    #[serde(default = "default_notification_margin")]
    pub margin_x: u32,
    #[serde(default = "default_notification_margin")]
    pub margin_y: u32,
    #[serde(default = "default_notification_gap")]
    pub gap: u32,
    #[serde(default = "default_notification_width")]
    pub width: u32,
    #[serde(default = "default_notification_max_visible")]
    pub max_visible: usize,
    /// Extra horizontal offset from the anchor-selected placement.
    #[serde(default)]
    pub offset_x: i32,
    /// Extra vertical offset from the anchor-selected placement.
    #[serde(default)]
    pub offset_y: i32,
    /// Used for notifications whose `expire_timeout` is negative.
    #[serde(default = "default_notification_timeout")]
    pub default_timeout_ms: u32,
    /// Preserve future notification settings across a load/save cycle.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            anchor: NotificationAnchor::TopRight,
            monitor: default_notification_monitor(),
            margin_x: default_notification_margin(),
            margin_y: default_notification_margin(),
            gap: default_notification_gap(),
            width: default_notification_width(),
            max_visible: default_notification_max_visible(),
            offset_x: 0,
            offset_y: 0,
            default_timeout_ms: default_notification_timeout(),
            extra: BTreeMap::new(),
        }
    }
}

/// Screen-edge placement presets for notification stacks.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAnchor {
    TopLeft,
    TopCenter,
    #[default]
    TopRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

fn default_notification_monitor() -> String {
    "focused".to_owned()
}

const fn default_notification_margin() -> u32 {
    16
}

const fn default_notification_gap() -> u32 {
    10
}

const fn default_notification_width() -> u32 {
    360
}

const fn default_notification_max_visible() -> usize {
    4
}

const fn default_notification_timeout() -> u32 {
    5_000
}

/// Partial theme overrides supported by the runtime.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ThemeOverrides {
    pub bar_opacity: Option<f32>,
    pub accent_color: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    /// Preserve future theme overrides when edited by an older consumer.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}

/// Per-module configuration sections.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ModuleConfigs {
    #[serde(flatten)]
    pub modules: BTreeMap<String, toml::Value>,
}

impl Config {
    /// Load and decode a configuration file.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let source = fs::read_to_string(path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ConfigError::NotFound(path.to_owned())
            } else {
                ConfigError::Io {
                    path: path.to_owned(),
                    source,
                }
            }
        })?;
        toml::from_str(&source).map_err(ConfigError::from)
    }

    /// Return the raw TOML table for a module, if configured.
    pub fn module_config(&self, id: &str) -> Option<&toml::Value> {
        self.modules.modules.get(id)
    }

    /// Validate values independent of a selected theme or module implementation.
    pub fn validate(&self) -> Vec<ConfigDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.theme.trim().is_empty() {
            diagnostics.push(ConfigDiagnostic::error("theme", "theme must not be empty"));
        }
        if let Some(opacity) = self.theme_overrides.bar_opacity
            && !(0.0..=1.0).contains(&opacity)
        {
            diagnostics.push(ConfigDiagnostic::error(
                "theme_overrides.bar_opacity",
                "bar opacity must be between 0 and 1",
            ));
        }
        if let Some(size) = self.theme_overrides.font_size
            && (!size.is_finite() || size <= 0.0)
        {
            diagnostics.push(ConfigDiagnostic::error(
                "theme_overrides.font_size",
                "font size must be a finite positive number",
            ));
        }
        if let Some(color) = &self.theme_overrides.accent_color
            && !is_hex_color(color)
        {
            diagnostics.push(ConfigDiagnostic::error(
                "theme_overrides.accent_color",
                "color must use #rrggbb or #rrggbbaa",
            ));
        }
        for (id, value) in &self.modules.modules {
            if !value.is_table() {
                diagnostics.push(ConfigDiagnostic::error(
                    format!("modules.{id}"),
                    "module configuration must be a TOML table",
                ));
            }
        }
        let notifications = &self.notifications;
        if notifications.width == 0 {
            diagnostics.push(ConfigDiagnostic::error(
                "notifications.width",
                "notification width must be greater than zero",
            ));
        }
        if notifications.max_visible == 0 {
            diagnostics.push(ConfigDiagnostic::error(
                "notifications.max_visible",
                "max_visible must be greater than zero",
            ));
        }
        if notifications.monitor.trim().is_empty() {
            diagnostics.push(ConfigDiagnostic::error(
                "notifications.monitor",
                "monitor must be `focused`, `primary`, or an output name",
            ));
        }
        diagnostics
    }

    /// Validate configured module fields and theme identity against the
    /// runtime-generated editor schema in addition to shared invariants.
    pub fn validate_with_schema(&self, schema: &ConfigSchema) -> Vec<ConfigDiagnostic> {
        let mut diagnostics = self.validate();
        if !schema.themes.is_empty() && !schema.themes.iter().any(|theme| theme.id == self.theme) {
            diagnostics.push(ConfigDiagnostic::error(
                "theme",
                format!("unknown theme `{}`", self.theme),
            ));
        }

        if let Ok(config_value) = toml::Value::try_from(self) {
            for field in &schema.fields {
                if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.is_error() && diagnostic.path == field.key)
                {
                    continue;
                }
                let Some(value) = toml_value_at(&config_value, &field.key) else {
                    continue;
                };
                if let Some(message) = field.field_type.validate_value(value) {
                    diagnostics.push(ConfigDiagnostic::error(field.key.clone(), message));
                }
            }
        }

        for (module_id, config) in &self.modules.modules {
            let Some(module_schema) = schema
                .modules
                .iter()
                .find(|module| module.module_id == *module_id)
            else {
                diagnostics.push(ConfigDiagnostic::warning(
                    format!("modules.{module_id}"),
                    "module is not built into this HyprDeck version",
                ));
                continue;
            };
            for field in &module_schema.fields {
                let Some(value) = toml_value_at(config, &field.key) else {
                    continue;
                };
                if let Some(message) = field.field_type.validate_value(value) {
                    diagnostics.push(ConfigDiagnostic::error(
                        format!("modules.{module_id}.{}", field.key),
                        message,
                    ));
                }
            }
        }
        diagnostics
    }

    /// Validate and atomically replace `path`, retaining the prior file with a
    /// `.bak` suffix.
    pub fn save_atomic(&self, path: &Path) -> Result<(), ConfigError> {
        let diagnostics = self.validate();
        if diagnostics.iter().any(ConfigDiagnostic::is_error) {
            return Err(ConfigError::Validation(diagnostics));
        }

        let encoded = toml::to_string_pretty(self)?;
        let _: Config = toml::from_str(&encoded)?;

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
            path: parent.to_owned(),
            source,
        })?;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let file_name = path
            .file_name()
            .map_or_else(|| OsString::from("hyprdeck.toml"), OsString::from);
        let mut temp_name = file_name.clone();
        temp_name.push(format!(".tmp-{}-{stamp}", std::process::id()));
        let temp_path = parent.join(temp_name);

        let result = (|| {
            let mut temp = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)
                .map_err(|source| ConfigError::Io {
                    path: temp_path.clone(),
                    source,
                })?;
            if let Ok(metadata) = fs::metadata(path) {
                temp.set_permissions(metadata.permissions())
                    .map_err(|source| ConfigError::Io {
                        path: temp_path.clone(),
                        source,
                    })?;
            }
            temp.write_all(encoded.as_bytes())
                .and_then(|_| temp.sync_all())
                .map_err(|source| ConfigError::Io {
                    path: temp_path.clone(),
                    source,
                })?;

            if path.exists() {
                let backup = backup_path(path);
                fs::copy(path, &backup).map_err(|source| ConfigError::Io {
                    path: backup,
                    source,
                })?;
            }
            fs::rename(&temp_path, path).map_err(|source| ConfigError::Io {
                path: path.to_owned(),
                source,
            })?;
            Ok(())
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map_or_else(|| OsString::from("hyprdeck.toml"), OsString::from);
    name.push(".bak");
    path.with_file_name(name)
}

fn is_hex_color(value: &str) -> bool {
    matches!(value.len(), 7 | 9)
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Serializable description of the complete editor-facing configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigSchema {
    pub contract_version: u32,
    /// Shared top-level and nested fields, using dotted TOML paths.
    #[serde(default)]
    pub fields: Vec<ConfigField>,
    /// Themes available to the running HyprDeck build.
    #[serde(default)]
    pub themes: Vec<ThemeMetadata>,
    pub modules: Vec<ModuleConfigSchema>,
}

impl ConfigSchema {
    pub fn new(modules: Vec<ModuleConfigSchema>) -> Self {
        Self {
            contract_version: CONFIG_CONTRACT_VERSION,
            fields: shared_config_fields(),
            themes: Vec::new(),
            modules,
        }
    }

    /// Attach discovered themes and make the top-level theme field a typed
    /// choice without teaching the contract crate about theme discovery.
    pub fn with_themes(mut self, themes: Vec<ThemeMetadata>) -> Self {
        if let Some(field) = self.fields.iter_mut().find(|field| field.key == "theme") {
            let options = themes.iter().map(|theme| theme.id.clone()).collect();
            field.field_type = ConfigFieldType::Choice {
                options,
                default: "win7".to_owned(),
            };
        }
        self.themes = themes;
        self
    }
}

/// Theme identity and presentation metadata included in the editor contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThemeMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Stable module IDs placed by this theme, in first-appearance order.
    #[serde(default)]
    pub modules: Vec<String>,
}

fn shared_config_fields() -> Vec<ConfigField> {
    vec![
        ConfigField {
            key: "theme".into(),
            label: "Theme".into(),
            description: "Embedded or user theme selected for every panel.".into(),
            field_type: ConfigFieldType::Text {
                default: "win7".into(),
            },
        },
        ConfigField {
            key: "theme_overrides.bar_opacity".into(),
            label: "Bar opacity".into(),
            description: "Override the selected theme's bar opacity.".into(),
            field_type: ConfigFieldType::Float {
                default: 1.0,
                min: Some(0.0),
                max: Some(1.0),
            },
        },
        ConfigField {
            key: "theme_overrides.accent_color".into(),
            label: "Accent color".into(),
            description: "Override the selected theme's #rrggbb or #rrggbbaa accent.".into(),
            field_type: ConfigFieldType::Color {
                default: "#3584e4".into(),
            },
        },
        ConfigField {
            key: "theme_overrides.font_family".into(),
            label: "Font family".into(),
            description: "Override the selected theme's font family.".into(),
            field_type: ConfigFieldType::Text {
                default: "sans-serif".into(),
            },
        },
        ConfigField {
            key: "theme_overrides.font_size".into(),
            label: "Font size".into(),
            description: "Override the selected theme's logical-pixel font size.".into(),
            field_type: ConfigFieldType::Float {
                default: 12.0,
                min: Some(6.0),
                max: Some(72.0),
            },
        },
        ConfigField {
            key: "notifications.enabled".into(),
            label: "Desktop notifications".into(),
            description: "Claim org.freedesktop.Notifications while HyprDeck is running.".into(),
            field_type: ConfigFieldType::Boolean { default: false },
        },
        ConfigField {
            key: "notifications.anchor".into(),
            label: "Notification placement".into(),
            description: "Screen anchor for the notification stack.".into(),
            field_type: ConfigFieldType::Choice {
                options: [
                    "top_left",
                    "top_center",
                    "top_right",
                    "bottom_left",
                    "bottom_center",
                    "bottom_right",
                ]
                .into_iter()
                .map(str::to_owned)
                .collect(),
                default: "top_right".into(),
            },
        },
        ConfigField {
            key: "notifications.monitor".into(),
            label: "Notification monitor".into(),
            description: "focused, primary, or an exact output connector name.".into(),
            field_type: ConfigFieldType::Text {
                default: "focused".into(),
            },
        },
        ConfigField {
            key: "notifications.width".into(),
            label: "Notification width".into(),
            description: "Notification surface width in logical pixels.".into(),
            field_type: ConfigFieldType::Integer {
                default: 360,
                min: Some(120),
                max: Some(1200),
            },
        },
        ConfigField {
            key: "notifications.margin_x".into(),
            label: "Horizontal margin".into(),
            description: "Distance from the selected horizontal screen edge.".into(),
            field_type: ConfigFieldType::Integer {
                default: 16,
                min: Some(0),
                max: Some(1000),
            },
        },
        ConfigField {
            key: "notifications.margin_y".into(),
            label: "Vertical margin".into(),
            description: "Distance from the selected vertical screen edge.".into(),
            field_type: ConfigFieldType::Integer {
                default: 16,
                min: Some(0),
                max: Some(1000),
            },
        },
        ConfigField {
            key: "notifications.gap".into(),
            label: "Notification gap".into(),
            description: "Space between stacked notifications.".into(),
            field_type: ConfigFieldType::Integer {
                default: 10,
                min: Some(0),
                max: Some(200),
            },
        },
        ConfigField {
            key: "notifications.max_visible".into(),
            label: "Maximum visible notifications".into(),
            description: "Maximum number of notification surfaces in the stack.".into(),
            field_type: ConfigFieldType::Integer {
                default: 4,
                min: Some(1),
                max: Some(20),
            },
        },
        ConfigField {
            key: "notifications.default_timeout_ms".into(),
            label: "Default timeout".into(),
            description: "Lifetime in milliseconds when an application uses the server default."
                .into(),
            field_type: ConfigFieldType::Integer {
                default: 5000,
                min: Some(0),
                max: Some(120000),
            },
        },
        ConfigField {
            key: "notifications.offset_x".into(),
            label: "Horizontal offset".into(),
            description: "Signed logical-pixel offset from the selected anchor.".into(),
            field_type: ConfigFieldType::Integer {
                default: 0,
                min: None,
                max: None,
            },
        },
        ConfigField {
            key: "notifications.offset_y".into(),
            label: "Vertical offset".into(),
            description: "Signed logical-pixel offset from the selected anchor.".into(),
            field_type: ConfigFieldType::Integer {
                default: 0,
                min: None,
                max: None,
            },
        },
    ]
}

/// Self-description of one module's configurable options.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleConfigSchema {
    pub module_id: String,
    pub fields: Vec<ConfigField>,
}

/// A configurable field in a module table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    pub description: String,
    pub field_type: ConfigFieldType,
}

/// Editor control and validation metadata for a configurable field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConfigFieldType {
    Text {
        default: String,
    },
    Integer {
        default: i64,
        min: Option<i64>,
        max: Option<i64>,
    },
    Float {
        default: f64,
        min: Option<f64>,
        max: Option<f64>,
    },
    Boolean {
        default: bool,
    },
    Choice {
        options: Vec<String>,
        default: String,
    },
    LabeledChoice {
        options: Vec<String>,
        labels: Vec<String>,
        default: String,
    },
    Color {
        default: String,
    },
}

impl ConfigFieldType {
    fn validate_value(&self, value: &toml::Value) -> Option<String> {
        match self {
            Self::Text { .. } if !value.is_str() => Some("expected text".into()),
            Self::Boolean { .. } if !value.is_bool() => Some("expected true or false".into()),
            Self::Color { .. } => match value.as_str() {
                Some(color) if is_hex_color(color) => None,
                _ => Some("expected a color in #rrggbb or #rrggbbaa form".into()),
            },
            Self::Integer { min, max, .. } => match value.as_integer() {
                None => Some("expected an integer".into()),
                Some(number) if min.is_some_and(|minimum| number < minimum) => {
                    Some(format!("must be at least {}", min.unwrap_or(number)))
                }
                Some(number) if max.is_some_and(|maximum| number > maximum) => {
                    Some(format!("must be at most {}", max.unwrap_or(number)))
                }
                Some(_) => None,
            },
            Self::Float { min, max, .. } => {
                let number = value
                    .as_float()
                    .or_else(|| value.as_integer().map(|v| v as f64));
                match number {
                    None => Some("expected a number".into()),
                    Some(number) if !number.is_finite() => Some("must be finite".into()),
                    Some(number) if min.is_some_and(|minimum| number < minimum) => {
                        Some(format!("must be at least {}", min.unwrap_or(number)))
                    }
                    Some(number) if max.is_some_and(|maximum| number > maximum) => {
                        Some(format!("must be at most {}", max.unwrap_or(number)))
                    }
                    Some(_) => None,
                }
            }
            Self::Choice { options, .. } | Self::LabeledChoice { options, .. } => {
                match value.as_str() {
                    None => Some("expected a choice string".into()),
                    Some(choice) if !options.iter().any(|option| option == choice) => {
                        Some(format!("expected one of: {}", options.join(", ")))
                    }
                    Some(_) => None,
                }
            }
            _ => None,
        }
    }
}

fn toml_value_at<'a>(root: &'a toml::Value, dotted_key: &str) -> Option<&'a toml::Value> {
    dotted_key
        .split('.')
        .try_fold(root, |value, key| value.as_table()?.get(key))
}

/// Severity of a validation diagnostic.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    Warning,
    Error,
}

/// Machine-readable validation feedback suitable for a settings editor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub severity: DiagnosticSeverity,
    pub path: String,
    pub message: String,
}

impl ConfigDiagnostic {
    pub fn error(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }

    pub fn warning(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            path: path.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_config() -> Config {
        Config {
            theme: "win7".into(),
            theme_overrides: ThemeOverrides::default(),
            modules: ModuleConfigs::default(),
            notifications: NotificationConfig::default(),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn config_roundtrip_preserves_unknown_data() {
        let source = r#"
theme = "win7"
future_option = "kept"

[theme_overrides]
future_color = "kept"

[modules.clock]
format = "%H:%M"
"#;
        let config: Config = toml::from_str(source).unwrap();
        let encoded = toml::to_string(&config).unwrap();
        let decoded: Config = toml::from_str(&encoded).unwrap();
        assert_eq!(decoded.extra["future_option"].as_str(), Some("kept"));
        assert_eq!(
            decoded.theme_overrides.extra["future_color"].as_str(),
            Some("kept")
        );
        assert_eq!(
            decoded.module_config("clock").unwrap()["format"].as_str(),
            Some("%H:%M")
        );
    }

    #[test]
    fn validation_rejects_invalid_shared_values() {
        let mut config = sample_config();
        config.theme.clear();
        config.theme_overrides.bar_opacity = Some(1.5);
        config.theme_overrides.accent_color = Some("red".into());
        assert_eq!(config.validate().len(), 3);
    }

    #[test]
    fn notification_config_parses_presets_and_custom_offsets() {
        let config: Config = toml::from_str(
            r#"
theme = "win7"

[notifications]
enabled = true
anchor = "bottom_center"
monitor = "DP-2"
margin_x = 24
gap = 8
width = 420
offset_x = -12
offset_y = 6
max_visible = 3
"#,
        )
        .unwrap();

        assert!(config.notifications.enabled);
        assert_eq!(
            config.notifications.anchor,
            NotificationAnchor::BottomCenter
        );
        assert_eq!(config.notifications.monitor, "DP-2");
        assert_eq!(config.notifications.offset_x, -12);
        assert_eq!(config.notifications.max_visible, 3);
    }

    #[test]
    fn atomic_save_keeps_backup_and_loads_new_value() {
        let dir = std::env::temp_dir().join(format!(
            "hyprdeck-config-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("hyprdeck.toml");

        let mut config = sample_config();
        config.save_atomic(&path).unwrap();
        config.theme = "gnome_classic".into();
        config.save_atomic(&path).unwrap();

        assert_eq!(Config::load(&path).unwrap().theme, "gnome_classic");
        assert_eq!(Config::load(&backup_path(&path)).unwrap().theme, "win7");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn schema_serialization_is_versioned_and_typed() {
        let schema = ConfigSchema::new(vec![ModuleConfigSchema {
            module_id: "clock".into(),
            fields: vec![ConfigField {
                key: "format".into(),
                label: "Format".into(),
                description: "Chrono format".into(),
                field_type: ConfigFieldType::Text {
                    default: "%H:%M".into(),
                },
            }],
        }]);
        let json = serde_json::to_value(schema).unwrap();
        assert_eq!(json["contract_version"], 1);
        assert_eq!(
            json["modules"][0]["fields"][0]["field_type"]["type"],
            "text"
        );
    }

    #[test]
    fn schema_validation_checks_nested_typed_module_fields() {
        let mut config = sample_config();
        config.modules.modules.insert(
            "power".into(),
            toml::from_str::<toml::Value>("[commands]\nshutdown = 3\n").unwrap(),
        );
        config.modules.modules.insert(
            "future_module".into(),
            toml::Value::Table(Default::default()),
        );
        let schema = ConfigSchema::new(vec![ModuleConfigSchema {
            module_id: "power".into(),
            fields: vec![ConfigField {
                key: "commands.shutdown".into(),
                label: "Shutdown".into(),
                description: String::new(),
                field_type: ConfigFieldType::Text {
                    default: "systemctl poweroff".into(),
                },
            }],
        }])
        .with_themes(vec![ThemeMetadata {
            id: "win7".into(),
            name: "Windows 7".into(),
            description: String::new(),
            modules: vec!["power".into()],
        }]);

        let diagnostics = config.validate_with_schema(&schema);
        assert!(
            diagnostics
                .iter()
                .any(|item| { item.is_error() && item.path == "modules.power.commands.shutdown" })
        );
        assert!(diagnostics.iter().any(|item| {
            item.severity == DiagnosticSeverity::Warning && item.path == "modules.future_module"
        }));
    }

    #[test]
    fn schema_validation_checks_notification_ranges() {
        let mut config = sample_config();
        config.notifications.width = 1;
        config.notifications.max_visible = 25;

        let diagnostics = config.validate_with_schema(&ConfigSchema::new(Vec::new()));
        assert!(
            diagnostics
                .iter()
                .any(|item| item.is_error() && item.path == "notifications.width")
        );
        assert!(
            diagnostics
                .iter()
                .any(|item| item.is_error() && item.path == "notifications.max_visible")
        );
    }

    #[test]
    fn notification_schema_default_matches_runtime_default() {
        let config = sample_config();
        let schema = ConfigSchema::new(Vec::new());
        let max_visible = schema
            .fields
            .iter()
            .find(|field| field.key == "notifications.max_visible")
            .expect("notification field is present");
        assert_eq!(
            max_visible.field_type,
            ConfigFieldType::Integer {
                default: config.notifications.max_visible as i64,
                min: Some(1),
                max: Some(20),
            }
        );
    }

    #[test]
    fn empty_xdg_falls_back_to_home() {
        let path = resolve_config_path(Some(OsString::new()), Some("/home/test".into())).unwrap();
        assert_eq!(path, Path::new("/home/test/.config/hypr/hyprdeck.toml"));
    }
}
