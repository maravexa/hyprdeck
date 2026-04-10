pub mod calendar;
pub mod clock;
pub mod favorites;
pub mod lunar;
pub mod menu;
pub mod network;
pub mod shell;
pub mod weather;
pub mod window_list;
pub mod workspaces;

pub use calendar::CalendarModule;
pub use clock::ClockModule;
pub use favorites::FavoritesModule;
pub use lunar::LunarModule;
pub use menu::MenuModule;
pub use network::NetworkModule;
pub use shell::ShellModule;
pub use weather::WeatherModule;
pub use window_list::WindowListModule;
pub use workspaces::WorkspacesModule;

/// Instantiate a module by its string ID, consuming a raw TOML config value.
///
/// Returns `None` if `id` is not recognised.  Each module is responsible for
/// deserialising its own config section; unrecognised keys are silently ignored.
pub fn create_module(
    id: &str,
    config: toml::Value,
) -> Option<Box<dyn hyprdeck_core::PanelModule>> {
    todo!()
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
        "shell",
        "weather",
        "window_list",
        "workspaces",
    ]
}
