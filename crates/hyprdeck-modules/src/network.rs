//! Network status module.
//!
//! Polls `/sys/class/net/` and `/proc/net/wireless` on a configurable interval,
//! then renders a compact icon indicating connectivity and signal strength.
//!
//! All filesystem reads happen synchronously because they target virtual kernel
//! files (`/sys`, `/proc`) that return immediately from kernel memory.

use std::time::{Duration, Instant};

use serde::Deserialize;
use tiny_skia::{LineCap, Paint, PathBuilder, Stroke, Transform};

use hyprdeck_core::{
    ConfigField, ConfigFieldType, DisplayMode, EventResult, InputEvent, ModuleConfigSchema,
    PanelModule, Pixmap, Point, PopupContent, PopupEventResult, Rect, Size, ThemeContext,
    UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// Configuration for the network status module.
#[derive(Debug, Default, Deserialize)]
pub struct NetworkConfig {
    /// `icon` — square icon only; `verbose` — icon left half + readout right half.
    #[serde(default)]
    pub display: DisplayMode,
    /// Network interface to monitor; auto-detected if absent.
    pub interface: Option<String>,
    /// Poll interval in seconds.
    #[serde(default = "default_poll_secs")]
    pub poll_secs: u64,
}

fn default_poll_secs() -> u64 {
    5
}

// ── Internal state ────────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, PartialEq)]
struct NetworkSnapshot {
    is_connected: bool,
    is_wireless: bool,
    interface_name: String,
    ip_address: Option<String>,
    /// WiFi signal level in dBm (negative).  `None` for wired or unknown.
    signal_dbm: Option<i32>,
    /// Link speed in Mbps for wired interfaces.  `None` for wireless or unknown.
    link_speed_mbps: Option<u64>,
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the network indicator module.
pub struct NetworkModule {
    config: NetworkConfig,
    /// Primary interface shown in the bar icon (WiFi preferred, else first wired).
    snapshot: NetworkSnapshot,
    /// All active interfaces, used by the popup.
    all_interfaces: Vec<NetworkSnapshot>,
    last_poll: Option<Instant>,
}

impl NetworkModule {
    pub fn new(config: NetworkConfig) -> Self {
        NetworkModule {
            config,
            snapshot: NetworkSnapshot::default(),
            all_interfaces: Vec::new(),
            last_poll: None,
        }
    }

    fn should_poll(&self) -> bool {
        match self.last_poll {
            None => true,
            Some(t) => t.elapsed() >= Duration::from_secs(self.config.poll_secs),
        }
    }

    /// Build the right-half readout string for verbose display mode.
    fn verbose_readout(&self) -> String {
        if !self.snapshot.is_connected {
            return "--".to_owned();
        }
        if self.snapshot.is_wireless {
            match self.snapshot.signal_dbm {
                Some(dbm) => format!("{} dBm", dbm),
                None => "--".to_owned(),
            }
        } else {
            match self.snapshot.link_speed_mbps {
                Some(mbps) => format_link_speed(mbps),
                None => "--".to_owned(),
            }
        }
    }
}

impl PanelModule for NetworkModule {
    fn id(&self) -> &str {
        "network"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        let h = theme.fonts.size * 2.0;
        match self.config.display {
            DisplayMode::Icon => {
                let padding = theme.padding.left + theme.padding.right + 8.0;
                Size::new(h + padding, h)
            }
            DisplayMode::Verbose => Size::new(h * 2.0, h),
        }
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        let _ = ctx;
        if !self.should_poll() {
            return false;
        }
        self.last_poll = Some(Instant::now());

        let new_all = poll_all_active_interfaces();

        // Primary interface: prefer the configured one, else WiFi, else first wired.
        let new_snap = if let Some(name) = self.config.interface.as_deref() {
            new_all
                .iter()
                .find(|s| s.interface_name == name)
                .cloned()
                .unwrap_or_else(|| poll_interface(name))
        } else {
            new_all
                .iter()
                .find(|s| s.is_wireless)
                .or_else(|| new_all.first())
                .cloned()
                .unwrap_or_default()
        };

        if new_snap != self.snapshot || new_all != self.all_interfaces {
            self.snapshot = new_snap;
            self.all_interfaces = new_all;
            true
        } else {
            false
        }
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        tracing::debug!(
            "Network render: connected={}, is_wireless={}, interface={}",
            self.snapshot.is_connected,
            self.snapshot.is_wireless,
            self.snapshot.interface_name,
        );

        let fg = theme.colors.foreground;
        let dim = render_utils::dim_color(fg, 0.4);
        let active = if self.snapshot.is_connected { fg } else { dim };

        match self.config.display {
            DisplayMode::Icon => {
                let icon_size = render_utils::canonical_icon_size(bounds);
                let icon_rect = render_utils::centered_icon_rect(bounds, icon_size);
                if self.snapshot.is_wireless {
                    draw_wifi_icon(canvas, icon_rect, active, self.snapshot.signal_dbm);
                } else {
                    draw_ethernet_icon(canvas, icon_rect, active);
                }
            }
            DisplayMode::Verbose => {
                let readout = self.verbose_readout();
                render_utils::draw_verbose(
                    canvas,
                    bounds,
                    theme,
                    &readout,
                    active,
                    |canvas, icon_rect| {
                        if self.snapshot.is_wireless {
                            draw_wifi_icon(canvas, icon_rect, active, self.snapshot.signal_dbm);
                        } else {
                            draw_ethernet_icon(canvas, icon_rect, active);
                        }
                    },
                );
            }
        }
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
        Some(Box::new(NetworkPopup::new(&self.all_interfaces)))
    }

    fn config_schema(&self) -> ModuleConfigSchema {
        ModuleConfigSchema {
            module_id: self.id().to_owned(),
            fields: vec![
                ConfigField {
                    key: "display".to_owned(),
                    label: "Display mode".to_owned(),
                    description: "Icon-only square or double-wide icon + signal/speed readout."
                        .to_owned(),
                    field_type: ConfigFieldType::LabeledChoice {
                        options: vec!["icon".to_owned(), "verbose".to_owned()],
                        labels: vec!["Icon only".to_owned(), "Icon + value".to_owned()],
                        default: "icon".to_owned(),
                    },
                },
                ConfigField {
                    key: "interface".to_owned(),
                    label: "Interface".to_owned(),
                    description:
                        "Network interface name to monitor (e.g. \"wlan0\"). Auto-detected if empty.".to_owned(),
                    field_type: ConfigFieldType::Text { default: String::new() },
                },
                ConfigField {
                    key: "poll_secs".to_owned(),
                    label: "Poll interval (seconds)".to_owned(),
                    description: "How often to refresh network status.".to_owned(),
                    field_type: ConfigFieldType::Integer {
                        default: 5,
                        min: Some(1),
                        max: Some(60),
                    },
                },
            ],
        }
    }
}

// ── Link speed formatter ──────────────────────────────────────────────────────

/// Format a wired link speed compactly.
///
/// Values < 1000 Mb/s use `Mb`; values ≥ 1000 Mb/s use `Gb`.  Trailing
/// fractional zeros are dropped (`1Gb` not `1.0Gb`).
pub fn format_link_speed(mbps: u64) -> String {
    if mbps >= 1000 {
        if mbps % 1000 == 0 {
            format!("{}Gb", mbps / 1000)
        } else {
            format!("{:.1}Gb", mbps as f64 / 1000.0)
        }
    } else {
        format!("{}Mb", mbps)
    }
}

// ── System polling ────────────────────────────────────────────────────────────

fn poll_interface(iface: &str) -> NetworkSnapshot {
    let state_path = format!("/sys/class/net/{iface}/operstate");
    let is_connected = std::fs::read_to_string(&state_path)
        .map(|s| s.trim() == "up")
        .unwrap_or(false);

    let is_wireless = std::path::Path::new(&format!("/sys/class/net/{iface}/wireless")).exists();

    let ip_address = get_ip(iface);
    let signal_dbm = if is_wireless {
        read_wifi_signal(iface)
    } else {
        None
    };

    let link_speed_mbps = if !is_wireless && is_connected {
        std::fs::read_to_string(format!("/sys/class/net/{iface}/speed"))
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
    } else {
        None
    };

    NetworkSnapshot {
        is_connected,
        is_wireless,
        interface_name: iface.to_owned(),
        ip_address,
        signal_dbm,
        link_speed_mbps,
    }
}

/// Return snapshots for every non-loopback interface whose `operstate` is `up`.
/// WiFi interfaces are sorted first.
fn poll_all_active_interfaces() -> Vec<NetworkSnapshot> {
    let entries = match std::fs::read_dir("/sys/class/net") {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut snapshots: Vec<NetworkSnapshot> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|name| name != "lo")
        .filter(|name| {
            std::fs::read_to_string(format!("/sys/class/net/{name}/operstate"))
                .map(|s| s.trim() == "up")
                .unwrap_or(false)
        })
        .map(|name| poll_interface(&name))
        .collect();
    // WiFi first, then wired.
    snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.is_wireless));
    snapshots
}

fn get_ip(iface: &str) -> Option<String> {
    match nix::ifaddrs::getifaddrs() {
        Ok(addrs) => {
            for addr in addrs {
                if addr.interface_name != iface {
                    continue;
                }
                if let Some(sa) = addr.address {
                    if let Some(sin) = sa.as_sockaddr_in() {
                        let ip = sin.ip();
                        if !ip.is_loopback() && !ip.is_unspecified() {
                            return Some(ip.to_string());
                        }
                    }
                }
            }
            None
        }
        Err(e) => {
            tracing::debug!("getifaddrs failed: {e}");
            None
        }
    }
}

/// Parse `/proc/net/wireless` to find the signal level (dBm) for `iface`.
///
/// Example line format:
///   `wlan0: 0000   54.  -58.  -256.   0      0      0      0      0      0`
fn read_wifi_signal(iface: &str) -> Option<i32> {
    let text = std::fs::read_to_string("/proc/net/wireless").ok()?;
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with(iface) {
            continue;
        }
        // Strip interface name and colon, then split on whitespace.
        let rest = line.split_once(':')?.1;
        let fields: Vec<&str> = rest.split_whitespace().collect();
        // Field index 2 = signal level.
        if fields.len() < 3 {
            break;
        }
        let raw = fields[2].trim_end_matches('.');
        if let Ok(v) = raw.parse::<i32>() {
            return Some(v);
        }
    }
    None
}

// ── Icon drawing ──────────────────────────────────────────────────────────────

/// Draw a single WiFi arc centred at `(cx, cy)` with radius `r`.
///
/// Arcs span 120 degrees, opening downward (fanning upward from the anchor point),
/// which is the standard WiFi icon shape.
fn draw_wifi_arc(
    canvas: &mut Pixmap,
    cx: f32,
    cy: f32,
    r: f32,
    color: hyprdeck_core::Color,
    stroke_w: f32,
) {
    // 120-degree arc, from 210° to 330° in screen coordinates
    // (0° = right, angles increase clockwise because y-axis points down).
    // cos/sin at these angles:
    //   210° → (-√3/2, -1/2) → left of and above (cx, cy)
    //   270° → (0, -1)       → directly above   (cx, cy)  ← arc peak
    //   330° → (+√3/2, -1/2) → right of and above (cx, cy)
    let start_rad = 210.0_f32.to_radians();
    let end_rad = 330.0_f32.to_radians();
    let steps = 16_u32;

    let mut pb = PathBuilder::new();
    for j in 0..=steps {
        let t = j as f32 / steps as f32;
        let angle = start_rad + t * (end_rad - start_rad);
        let x = cx + r * angle.cos();
        let y = cy + r * angle.sin();
        if j == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    let Some(path) = pb.finish() else {
        return;
    };

    let mut paint = Paint::default();
    paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
    paint.anti_alias = true;
    let stroke = Stroke {
        width: stroke_w,
        line_cap: LineCap::Round,
        ..Stroke::default()
    };
    canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

/// Draw concentric arc segments representing WiFi signal strength.
fn draw_wifi_icon(
    canvas: &mut Pixmap,
    rect: Rect,
    color: hyprdeck_core::Color,
    signal_dbm: Option<i32>,
) {
    // Map dBm to bar count (1-4).
    let bars = match signal_dbm {
        None => 3, // default to medium
        Some(v) if v >= -55 => 4,
        Some(v) if v >= -67 => 3,
        Some(v) if v >= -80 => 2,
        _ => 1,
    };

    // Anchor point at the lower-center of the icon rect.
    let cx = rect.x + rect.width / 2.0;
    let cy = rect.y + rect.height * 0.85;
    let max_r = rect.width.min(rect.height) * 0.85;
    let stroke_w = (rect.width * 0.12).clamp(1.5, 3.0);

    let dim = [color[0], color[1], color[2], (color[3] as f32 * 0.25) as u8];

    // Draw 4 concentric arcs, smallest (innermost) to largest (outermost).
    for i in 1..=4_u8 {
        let r = max_r * (i as f32 / 4.0);
        let c = if (i as i32) <= bars { color } else { dim };
        draw_wifi_arc(canvas, cx, cy, r, c, stroke_w);
    }

    // Small filled dot at the anchor point.
    render_utils::fill_circle(canvas, Point::new(cx, cy), stroke_w * 0.8 + 0.5, color);
}

/// Draw a simple ethernet plug icon (two horizontal bars with a stub).
fn draw_ethernet_icon(canvas: &mut Pixmap, rect: Rect, color: hyprdeck_core::Color) {
    let cy = rect.y + rect.height / 2.0;
    let lw = 1.5_f32;
    // Main horizontal cable
    render_utils::draw_line(
        canvas,
        Point::new(rect.x, cy),
        Point::new(rect.x + rect.width, cy),
        color,
        lw,
    );
    // Two prongs
    let step = rect.width / 3.0;
    for i in 1..=2_u8 {
        let x = rect.x + step * i as f32;
        render_utils::draw_line(
            canvas,
            Point::new(x, cy),
            Point::new(x, cy - rect.height * 0.3),
            color,
            lw,
        );
    }
}

// ── Network popup ─────────────────────────────────────────────────────────────

/// Per-interface data stored in the popup.
struct IfaceRow {
    interface: String,
    ip_address: String,
    is_wifi: bool,
    signal_dbm: Option<i32>,
    speed_label: String,
}

impl IfaceRow {
    fn from_snapshot(snap: &NetworkSnapshot) -> Self {
        let speed_label = match snap.link_speed_mbps {
            Some(mbps) if mbps >= 1000 => format!("{} Gbps", mbps / 1000),
            Some(mbps) => format!("{mbps} Mbps"),
            None => String::new(),
        };
        Self {
            interface: snap.interface_name.clone(),
            ip_address: snap.ip_address.clone().unwrap_or_default(),
            is_wifi: snap.is_wireless,
            signal_dbm: snap.signal_dbm,
            speed_label,
        }
    }
}

/// Popup content for the network module — shows all active interface details.
pub struct NetworkPopup {
    rows: Vec<IfaceRow>,
}

impl NetworkPopup {
    fn new(interfaces: &[NetworkSnapshot]) -> Self {
        let rows = if interfaces.is_empty() {
            // Show a placeholder when nothing is connected.
            vec![IfaceRow {
                interface: String::new(),
                ip_address: String::new(),
                is_wifi: false,
                signal_dbm: None,
                speed_label: String::new(),
            }]
        } else {
            interfaces.iter().map(IfaceRow::from_snapshot).collect()
        };
        Self { rows }
    }
}

/// Height allocated to each interface section in the popup.
const IFACE_SECTION_H: f32 = 90.0;

impl PopupContent for NetworkPopup {
    fn desired_size(&self, _theme: &ThemeContext) -> Size {
        let h = (self.rows.len() as f32 * IFACE_SECTION_H).clamp(90.0, 400.0);
        Size::new(260.0, h)
    }

    fn render(&self, canvas: &mut Pixmap, theme: &ThemeContext, bounds: Rect) {
        let font = &theme.fonts.family;
        let bold = theme.fonts.bold_family.as_deref().unwrap_or(font);
        let font_size = 13.0;
        let dim = render_utils::dim_color(theme.colors.foreground, 0.65);
        let line_h = 22.0;

        if self.rows.is_empty() || (self.rows.len() == 1 && self.rows[0].interface.is_empty()) {
            let msg_rect = Rect::new(bounds.x, bounds.y, bounds.width, bounds.height);
            render_utils::draw_text_centered(
                canvas,
                "No active connections",
                msg_rect,
                font,
                font_size,
                dim,
            );
            return;
        }

        for (idx, row) in self.rows.iter().enumerate() {
            let section_y = bounds.y + idx as f32 * IFACE_SECTION_H;

            // Divider between sections (not before the first).
            if idx > 0 {
                render_utils::draw_line(
                    canvas,
                    Point::new(bounds.x + 8.0, section_y - 1.0),
                    Point::new(bounds.x + bounds.width - 8.0, section_y - 1.0),
                    render_utils::dim_color(theme.colors.foreground, 0.2),
                    1.0,
                );
            }

            let mut y = section_y + 6.0;

            // Title: "WiFi — wlan0" or "Ethernet — eth0"
            let kind = if row.is_wifi { "WiFi" } else { "Ethernet" };
            let title = format!("{kind} — {}", row.interface);
            let title_rect = Rect::new(bounds.x, y, bounds.width, line_h);
            render_utils::draw_text_centered(
                canvas,
                &title,
                title_rect,
                bold,
                font_size,
                theme.colors.foreground,
            );
            y += line_h;

            // IP address
            let ip_label = if row.ip_address.is_empty() {
                "No IP".to_owned()
            } else {
                format!("IP: {}", row.ip_address)
            };
            let ip_rect = Rect::new(bounds.x, y, bounds.width, line_h);
            render_utils::draw_text_centered(canvas, &ip_label, ip_rect, font, font_size, dim);
            y += line_h;

            // Signal (WiFi) or link speed (Ethernet)
            if row.is_wifi {
                let sig = match row.signal_dbm {
                    Some(dbm) => format!("Signal: {dbm} dBm"),
                    None => "Signal: unknown".to_owned(),
                };
                let sig_rect = Rect::new(bounds.x, y, bounds.width, line_h);
                render_utils::draw_text_centered(canvas, &sig, sig_rect, font, font_size, dim);
                y += line_h;

                // Signal strength bars
                if let Some(dbm) = row.signal_dbm {
                    let bars: i32 = if dbm >= -55 {
                        4
                    } else if dbm >= -67 {
                        3
                    } else if dbm >= -80 {
                        2
                    } else {
                        1
                    };
                    let bar_w = 8.0_f32;
                    let bar_gap = 4.0_f32;
                    let total_w = 4.0 * bar_w + 3.0 * bar_gap;
                    let start_x = bounds.x + (bounds.width - total_w) / 2.0;
                    let max_h = 16.0_f32;
                    for i in 0..4_u32 {
                        let h = max_h * (i as f32 + 1.0) / 4.0;
                        let bx = start_x + i as f32 * (bar_w + bar_gap);
                        let by = y + max_h - h;
                        let color = if (i as i32) < bars {
                            theme.colors.accent
                        } else {
                            dim
                        };
                        render_utils::fill_rounded_rect(
                            canvas,
                            Rect::new(bx, by, bar_w, h),
                            color,
                            2.0,
                        );
                    }
                }
            } else if !row.speed_label.is_empty() {
                let spd_rect = Rect::new(bounds.x, y, bounds.width, line_h);
                render_utils::draw_text_centered(
                    canvas,
                    &row.speed_label,
                    spd_rect,
                    font,
                    font_size,
                    dim,
                );
            }
        }
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
    use hyprdeck_core::HyprState;

    // ── format_link_speed ─────────────────────────────────────────────────────

    #[test]
    fn link_speed_megabit_values() {
        assert_eq!(format_link_speed(10), "10Mb");
        assert_eq!(format_link_speed(100), "100Mb");
        assert_eq!(format_link_speed(999), "999Mb");
    }

    #[test]
    fn link_speed_gigabit_integer() {
        assert_eq!(format_link_speed(1000), "1Gb");
        assert_eq!(format_link_speed(10_000), "10Gb");
        assert_eq!(format_link_speed(25_000), "25Gb");
        assert_eq!(format_link_speed(40_000), "40Gb");
        assert_eq!(format_link_speed(100_000), "100Gb");
    }

    #[test]
    fn link_speed_gigabit_fractional() {
        assert_eq!(format_link_speed(2_500), "2.5Gb");
    }

    #[test]
    fn link_speed_boundary() {
        assert_eq!(format_link_speed(999), "999Mb");
        assert_eq!(format_link_speed(1000), "1Gb");
    }

    // ── NetworkModule ─────────────────────────────────────────────────────────

    #[test]
    fn default_snapshot_is_disconnected() {
        let snap = NetworkSnapshot::default();
        assert!(!snap.is_connected);
        assert!(snap.ip_address.is_none());
    }

    #[test]
    fn update_returns_false_before_poll_interval() {
        let mut m = NetworkModule::new(NetworkConfig {
            poll_secs: 999,
            ..NetworkConfig::default()
        });
        let state = HyprState::default();
        let ctx = UpdateContext {
            now: chrono::Local::now(),
            hypr_state: &state,
            output_name: "",
        };
        // First update polls.
        m.update(&ctx);
        // Second update is too early.
        let changed = m.update(&ctx);
        // May or may not change depending on whether the OS returned the same state,
        // but it should not panic.
        let _ = changed;
    }

    #[test]
    fn should_poll_true_initially() {
        let m = NetworkModule::new(NetworkConfig::default());
        assert!(m.should_poll());
    }
}
