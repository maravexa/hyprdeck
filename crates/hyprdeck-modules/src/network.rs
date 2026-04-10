use serde::Deserialize;
use hyprdeck_core::Pixmap;

use hyprdeck_core::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Rect,
    Size, ThemeContext, UpdateContext,
};

/// How much network detail to display.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkDisplay {
    /// Icon only (wifi signal or ethernet icon).
    #[default]
    Icon,
    /// Icon plus SSID / interface name.
    IconLabel,
    /// Icon plus current upload/download rates.
    IconRate,
}

/// Configuration for the network status module.
#[derive(Debug, Default, Deserialize)]
pub struct NetworkConfig {
    #[serde(default)]
    pub display: NetworkDisplay,
    /// Network interface to monitor; auto-detected if absent.
    pub interface: Option<String>,
}

/// Snapshot of network state polled from the OS.
#[derive(Debug, Default)]
struct NetworkSnapshot {
    is_connected: bool,
    is_wireless: bool,
    ssid: Option<String>,
    signal_strength: Option<u8>,
    rx_bytes_per_sec: u64,
    tx_bytes_per_sec: u64,
}

/// Runtime state for the network indicator module.
pub struct NetworkModule {
    config: NetworkConfig,
    snapshot: NetworkSnapshot,
}

impl NetworkModule {
    pub fn new(config: NetworkConfig) -> Self {
        NetworkModule {
            config,
            snapshot: NetworkSnapshot::default(),
        }
    }
}

impl PanelModule for NetworkModule {
    fn id(&self) -> &str {
        "network"
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
        EventResult::Ignored
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "display".to_owned(),
                    label: "Display style".to_owned(),
                    description: "How much network detail to show.".to_owned(),
                    field_type: ConfigFieldType::Choice {
                        options: vec![
                            "icon".to_owned(),
                            "iconlabel".to_owned(),
                            "iconrate".to_owned(),
                        ],
                        default: "icon".to_owned(),
                    },
                },
                ConfigField {
                    key: "interface".to_owned(),
                    label: "Interface".to_owned(),
                    description: "Network interface name to monitor (e.g. \"wlan0\"). Auto-detected if empty.".to_owned(),
                    field_type: ConfigFieldType::Text { default: String::new() },
                },
            ],
        }
    }
}
