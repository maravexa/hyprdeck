pub mod action;
pub mod app;
pub mod autohide;
pub mod config;
pub mod geometry;
pub mod ipc;
pub mod layout;
pub mod module;
pub mod output;
pub mod panel;
pub mod popup;
pub mod render;
pub mod theme;
pub mod widgets;

// Re-export tiny_skia::Pixmap so downstream crates that implement PanelModule
// can name the type without adding tiny-skia as a direct dependency.
pub use tiny_skia::Pixmap;

// Flat re-exports of the most commonly used types.
pub use action::{Action, ActionError, dispatch_action};
pub use app::{App, ModuleFactory};
pub use autohide::{AnimPhase, AutoHideMode, AutoHideState};
pub use config::{Config, ModuleConfigs, ThemeOverrides};
pub use geometry::{DisplayGeometry, Edge, Point, Rect, Size};
pub use ipc::{
    CommandClient, EventSocket, HyprEvent, HyprIpc, HyprState, IpcError, MonitorInfo, WindowInfo,
    Workspace,
};
pub use layout::{
    Axis, DockAnimState, DockLayout, DockLayoutConfig, HorizontalLayout, LayoutEngine,
    LayoutResult, ModuleGroups, ModuleSizeProvider, VerticalLayout,
};
pub use module::{
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, MouseButton,
    PanelModule, ThemeContext, UpdateContext,
};
pub use popup::{PopupContent, PopupEventResult, PopupState};
pub use output::OutputState;
pub use panel::{
    Color, ColorPalette, FontConfig, InputResult, Padding, Panel, ResolvedModuleStyles,
    ResolvedSeparator, ResolvedStyle, ResolvedWindowListStyle, ResolvedWorkspacesStyle,
};
pub use render::Canvas;
pub use theme::{
    ButtonStyleDef, DockConfig, LayoutType, ModuleStyleMap, PanelDefinition, StyleDefinition,
    ThemeDefinition, WindowListStyleDef, WorkspacesStyleDef,
};
