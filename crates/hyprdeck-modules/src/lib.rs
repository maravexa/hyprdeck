pub mod calendar;
pub mod clock;
pub mod favorites;
pub mod icon_utils;
pub mod lunar;
pub mod menu;
pub mod network;
pub mod power;
pub mod render_utils;
pub mod shell;
pub mod sound;
pub mod weather;
pub mod window_list;
pub mod workspaces;

pub use calendar::CalendarModule;
pub use clock::ClockModule;
pub use favorites::FavoritesModule;
pub use lunar::LunarModule;
pub use menu::MenuModule;
pub use network::NetworkModule;
pub use power::PowerModule;
pub use shell::ShellModule;
pub use sound::SoundModule;
pub use weather::WeatherModule;
pub use window_list::WindowListModule;
pub use workspaces::WorkspacesModule;

/// Instantiate a module by its string ID, consuming a raw TOML config value.
///
/// Returns `None` if `id` is not recognised.  Each module is responsible for
/// deserialising its own config section; unrecognised keys are silently ignored.
pub fn create_module(id: &str, config: toml::Value) -> Option<Box<dyn hyprdeck_core::PanelModule>> {
    match id {
        "calendar" => {
            let cfg = parse_module_config::<calendar::CalendarConfig>(id, config);
            Some(Box::new(CalendarModule::new(cfg)))
        }
        "clock" => {
            let cfg = parse_module_config::<clock::ClockConfig>(id, config);
            Some(Box::new(ClockModule::new(cfg)))
        }
        "workspaces" => {
            let cfg = parse_module_config::<workspaces::WorkspacesConfig>(id, config);
            Some(Box::new(WorkspacesModule::new(cfg)))
        }
        "weather" => {
            let cfg = parse_module_config::<weather::WeatherConfig>(id, config);
            Some(Box::new(WeatherModule::new(cfg)))
        }
        "lunar" => {
            let cfg = parse_module_config::<lunar::LunarConfig>(id, config);
            Some(Box::new(LunarModule::new(cfg)))
        }
        "network" => {
            let cfg = parse_module_config::<network::NetworkConfig>(id, config);
            Some(Box::new(NetworkModule::new(cfg)))
        }
        "menu_button" => {
            let cfg = parse_module_config::<menu::MenuConfig>(id, config);
            Some(Box::new(MenuModule::new(cfg)))
        }
        "favorites" => {
            let cfg = parse_module_config::<favorites::FavoritesConfig>(id, config);
            Some(Box::new(FavoritesModule::new(cfg)))
        }
        "shell" => {
            let cfg = parse_module_config::<shell::ShellConfig>(id, config);
            Some(Box::new(ShellModule::new(cfg)))
        }
        "window_list" => {
            let cfg = parse_module_config::<window_list::WindowListConfig>(id, config);
            Some(Box::new(WindowListModule::new(cfg)))
        }
        "sound" => {
            let cfg = parse_module_config::<sound::SoundConfig>(id, config);
            Some(Box::new(SoundModule::new(cfg)))
        }
        "power" => {
            let cfg = parse_module_config::<power::PowerConfig>(id, config);
            Some(Box::new(PowerModule::new(cfg)))
        }
        unknown => {
            tracing::warn!("Unknown module ID: '{}' — skipping", unknown);
            None
        }
    }
}

/// Deserialize a `toml::Value` into a module config struct, falling back to
/// `Default` on parse failure.
fn parse_module_config<T>(module_id: &str, value: toml::Value) -> T
where
    T: Default + serde::de::DeserializeOwned,
{
    match value.try_into() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "Failed to parse config for module '{}': {}. Using defaults.",
                module_id,
                e,
            );
            T::default()
        }
    }
}

/// Return the IDs of all built-in modules.
pub fn builtin_module_ids() -> &'static [&'static str] {
    &[
        "calendar",
        "clock",
        "favorites",
        "lunar",
        "menu_button",
        "network",
        "power",
        "shell",
        "sound",
        "weather",
        "window_list",
        "workspaces",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_config() -> toml::Value {
        toml::Value::Table(toml::map::Map::new())
    }

    #[test]
    fn every_known_id_creates_a_module() {
        for id in builtin_module_ids() {
            let module = create_module(id, empty_config());
            assert!(module.is_some(), "create_module('{}') returned None", id,);
            let m = module.expect("just checked");
            assert_eq!(m.id(), *id, "module id() mismatch for '{}'", id);
        }
    }

    #[test]
    fn unknown_id_returns_none() {
        let result = create_module("nonexistent_module", empty_config());
        assert!(result.is_none());
    }

    #[test]
    fn none_config_uses_defaults() {
        // Empty table should work the same as no config
        let module = create_module("clock", empty_config());
        assert!(module.is_some());
        assert_eq!(module.expect("checked").id(), "clock");
    }

    #[test]
    fn invalid_config_falls_back_to_defaults() {
        // Pass an integer where a table is expected — should not panic
        let bad_config = toml::Value::Integer(42);
        let module = create_module("clock", bad_config);
        assert!(module.is_some());
        assert_eq!(module.expect("checked").id(), "clock");
    }

    #[test]
    fn available_modules_matches_registry() {
        let ids = builtin_module_ids();
        assert_eq!(ids.len(), 12);
        for id in ids {
            assert!(
                create_module(id, empty_config()).is_some(),
                "builtin_module_ids contains '{}' but registry doesn't handle it",
                id,
            );
        }
    }

    #[test]
    fn module_with_valid_config_parses() {
        let mut table = toml::map::Map::new();
        table.insert("format".to_owned(), toml::Value::String("%H:%M:%S".into()));
        let config = toml::Value::Table(table);
        let module = create_module("clock", config);
        assert!(module.is_some());
    }
}
