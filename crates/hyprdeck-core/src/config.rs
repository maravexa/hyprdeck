//! Compatibility re-exports for the standalone configuration contract.

pub use hyprdeck_config::{
    CONFIG_CONTRACT_VERSION, Config, ConfigDiagnostic, ConfigError, ConfigPathError, ConfigSchema,
    DiagnosticSeverity, ModuleConfigs, NotificationAnchor, NotificationConfig, ThemeMetadata,
    ThemeOverrides, default_config_path,
};
