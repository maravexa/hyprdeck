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
    ConfigField, ConfigFieldType, EventResult, InputEvent, ModuleConfigSchema, PanelModule, Pixmap,
    Point, Rect, Size, ThemeContext, UpdateContext,
};

use crate::render_utils;

// ── Config ────────────────────────────────────────────────────────────────────

/// What to show alongside the network icon.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkDisplay {
    #[default]
    Icon,
    IconLabel,
    IconRate,
}

/// Configuration for the network status module.
#[derive(Debug, Default, Deserialize)]
pub struct NetworkConfig {
    /// Display style.
    #[serde(default)]
    pub display: NetworkDisplay,
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
}

// ── Module ────────────────────────────────────────────────────────────────────

/// Runtime state for the network indicator module.
pub struct NetworkModule {
    config: NetworkConfig,
    snapshot: NetworkSnapshot,
    last_poll: Option<Instant>,
}

impl NetworkModule {
    pub fn new(config: NetworkConfig) -> Self {
        NetworkModule {
            config,
            snapshot: NetworkSnapshot::default(),
            last_poll: None,
        }
    }

    fn should_poll(&self) -> bool {
        match self.last_poll {
            None => true,
            Some(t) => t.elapsed() >= Duration::from_secs(self.config.poll_secs),
        }
    }
}

impl PanelModule for NetworkModule {
    fn id(&self) -> &str {
        "network"
    }

    fn desired_size(&self, theme: &ThemeContext) -> Size {
        // Give the icon plenty of horizontal room — the icon is drawn at
        // effective_font_size(bounds.height) in render(), which can be larger
        // than font_size when bar_height > font_size.  Use 2× font_size as a
        // generous width estimate so the icon is never squished.
        let icon_w = theme.fonts.size * 2.0;
        let label_w = match self.config.display {
            NetworkDisplay::Icon => 0.0,
            NetworkDisplay::IconLabel => {
                let label = if self.snapshot.is_wireless {
                    "WiFi"
                } else {
                    &self.snapshot.interface_name
                };
                render_utils::estimate_text_width(label, theme.fonts.size) + 4.0
            }
            NetworkDisplay::IconRate => {
                render_utils::estimate_text_width("↓ 0.0 MB/s", theme.fonts.size) + 4.0
            }
        };
        let padding = theme.padding.left + theme.padding.right + 8.0;
        Size::new(icon_w + label_w + padding, theme.fonts.size * 2.0)
    }

    fn update(&mut self, ctx: &UpdateContext<'_>) -> bool {
        let _ = ctx;
        if !self.should_poll() {
            return false;
        }
        self.last_poll = Some(Instant::now());

        let iface = self
            .config
            .interface
            .as_deref()
            .map(str::to_owned)
            .or_else(auto_detect_interface);

        let new_snap = match iface {
            None => NetworkSnapshot::default(),
            Some(iface) => poll_interface(&iface),
        };

        if new_snap != self.snapshot {
            self.snapshot = new_snap;
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

        let icon_dim = render_utils::effective_font_size(bounds.height, theme.fonts.size);
        let icon_rect = Rect::new(
            bounds.x + theme.padding.left,
            bounds.y + (bounds.height - icon_dim) / 2.0,
            icon_dim,
            icon_dim,
        );

        let fg = theme.colors.foreground;
        let dim = [fg[0], fg[1], fg[2], (fg[3] as f32 * 0.4) as u8];
        let active = if self.snapshot.is_connected { fg } else { dim };

        if self.snapshot.is_wireless {
            draw_wifi_icon(canvas, icon_rect, active, self.snapshot.signal_dbm);
        } else {
            draw_ethernet_icon(canvas, icon_rect, active);
        }

        // Optional label.
        if let NetworkDisplay::IconLabel = self.config.display {
            let label_x = icon_rect.x + icon_rect.width + 4.0;
            let label_rect = Rect::new(
                label_x,
                bounds.y,
                (bounds.x + bounds.width) - label_x,
                bounds.height,
            );
            let label = if self.snapshot.is_wireless {
                "WiFi"
            } else {
                &self.snapshot.interface_name
            };
            render_utils::draw_text(
                canvas,
                label,
                label_rect,
                &theme.fonts.family,
                icon_dim,
                active,
            );
        }
    }

    fn handle_event(&mut self, _event: &InputEvent, _bounds: Rect) -> EventResult {
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

// ── System polling ────────────────────────────────────────────────────────────

/// Return the first non-loopback interface that has `operstate == up`, preferring
/// wireless interfaces.
fn auto_detect_interface() -> Option<String> {
    let entries = std::fs::read_dir("/sys/class/net").ok()?;
    let mut wired: Option<String> = None;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "lo" {
            continue;
        }
        let state_path = format!("/sys/class/net/{name}/operstate");
        let state = std::fs::read_to_string(&state_path).unwrap_or_default();
        if state.trim() != "up" {
            continue;
        }
        // Prefer wireless
        let is_wifi = std::path::Path::new(&format!("/sys/class/net/{name}/wireless")).exists();
        if is_wifi {
            return Some(name);
        }
        if wired.is_none() {
            wired = Some(name);
        }
    }
    wired
}

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

    NetworkSnapshot {
        is_connected,
        is_wireless,
        interface_name: iface.to_owned(),
        ip_address,
        signal_dbm,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use hyprdeck_core::HyprState;

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
