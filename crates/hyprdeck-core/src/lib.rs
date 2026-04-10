pub mod action;
pub mod autohide;
pub mod config;
pub mod geometry;
pub mod ipc;
pub mod layout;
pub mod module;
pub mod panel;
pub mod render;
pub mod theme;

// Re-export tiny_skia::Pixmap so downstream crates that implement PanelModule
// can name the type without adding tiny-skia as a direct dependency.
pub use tiny_skia::Pixmap;

// Flat re-exports of the most commonly used types.
pub use action::Action;
pub use autohide::{AnimPhase, AutoHideMode, AutoHideState};
pub use config::{Config, ModuleConfigs, ThemeOverrides};
pub use geometry::{DisplayGeometry, Edge, Point, Rect, Size};
pub use ipc::{
    CommandClient, EventSocket, HyprEvent, HyprIpc, HyprState, IpcError, MonitorInfo, WindowInfo,
    Workspace,
};
pub use layout::{
    DockLayout, DockMagnification, HorizontalLayout, LayoutEngine, LayoutResult, ModuleGroups,
    VerticalLayout,
};
pub use module::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, ThemeContext, UpdateContext,
};
pub use panel::{ColorPalette, FontConfig, Padding, Panel, ResolvedStyle};
pub use render::RenderContext;
pub use theme::{DockConfig, LayoutType, PanelDefinition, StyleDefinition, ThemeDefinition};
