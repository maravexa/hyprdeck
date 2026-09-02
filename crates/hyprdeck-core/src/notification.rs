//! Desktop-notification state, placement, and rendering primitives.
//!
//! The D-Bus service and Wayland surface lifecycle stay in the binary crate;
//! this module deliberately owns only deterministic, testable policy.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::{Canvas, NotificationAnchor, NotificationConfig, Rect, ThemeContext};

/// A notification action supplied as alternating `key`, `label` values by the
/// Desktop Notifications D-Bus API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationAction {
    pub key: String,
    pub label: String,
}

/// A request received from the desktop-notification D-Bus service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationRequest {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<NotificationAction>,
    /// Freedesktop timeout semantics: negative = server default, zero = never.
    pub expire_timeout_ms: i32,
}

/// A notification with its calculated deadline.
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: u32,
    pub app_name: String,
    pub app_icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<NotificationAction>,
    expires_at: Option<Instant>,
}

impl Notification {
    fn from_request(
        request: NotificationRequest,
        config: &NotificationConfig,
        now: Instant,
    ) -> Self {
        let timeout_ms = match request.expire_timeout_ms {
            timeout if timeout < 0 => config.default_timeout_ms,
            0 => 0,
            timeout => timeout as u32,
        };
        Self {
            id: request.id,
            app_name: request.app_name,
            app_icon: request.app_icon,
            summary: request.summary,
            body: request.body,
            actions: request.actions,
            expires_at: (timeout_ms > 0)
                .then(|| now + Duration::from_millis(u64::from(timeout_ms))),
        }
    }

    fn expired(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|deadline| deadline <= now)
    }
}

/// Result of replacing, inserting, or expiring notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationChange {
    Added(u32),
    Replaced(u32),
    Closed(u32),
}

/// Queue model used by the Wayland runtime and D-Bus service bridge.
#[derive(Debug, Default)]
pub struct NotificationCenter {
    notifications: VecDeque<Notification>,
}

impl NotificationCenter {
    pub fn notify(
        &mut self,
        request: NotificationRequest,
        config: &NotificationConfig,
        now: Instant,
    ) -> NotificationChange {
        let id = request.id;
        let notification = Notification::from_request(request, config, now);
        if let Some(existing) = self.notifications.iter_mut().find(|item| item.id == id) {
            *existing = notification;
            NotificationChange::Replaced(id)
        } else {
            self.notifications.push_back(notification);
            NotificationChange::Added(id)
        }
    }

    pub fn close(&mut self, id: u32) -> Option<NotificationChange> {
        let index = self.notifications.iter().position(|item| item.id == id)?;
        self.notifications.remove(index);
        Some(NotificationChange::Closed(id))
    }

    pub fn expire(&mut self, now: Instant) -> Vec<NotificationChange> {
        let mut expired = Vec::new();
        self.notifications.retain(|notification| {
            let keep = !notification.expired(now);
            if !keep {
                expired.push(NotificationChange::Closed(notification.id));
            }
            keep
        });
        expired
    }

    /// Newest notifications appear closest to the configured anchor.
    pub fn visible(&self, max_visible: usize) -> impl Iterator<Item = &Notification> {
        self.notifications.iter().rev().take(max_visible)
    }

    pub fn len(&self) -> usize {
        self.notifications.len()
    }

    pub fn is_empty(&self) -> bool {
        self.notifications.is_empty()
    }
}

/// Pixel size of the compact, two-line notification surface.
pub const NOTIFICATION_HEIGHT: u32 = 96;

/// A layer-shell independent position derived from a configured anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationPlacement {
    pub anchor: NotificationAnchor,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub left: i32,
}

/// Compute layer-shell margins for one item in a notification stack.
pub fn notification_placement(
    config: &NotificationConfig,
    output_width: u32,
    item_index: usize,
    item_height: u32,
) -> NotificationPlacement {
    let stack_offset = item_index.saturating_mul(item_height as usize + config.gap as usize) as i32;
    let x = config.margin_x as i32 + config.offset_x;
    let y = config.margin_y as i32 + config.offset_y;
    let centered_left =
        ((output_width.saturating_sub(config.width) / 2) as i32 + config.offset_x).max(0);

    match config.anchor {
        NotificationAnchor::TopLeft => NotificationPlacement {
            anchor: config.anchor,
            top: y + stack_offset,
            right: 0,
            bottom: 0,
            left: x,
        },
        NotificationAnchor::TopCenter => NotificationPlacement {
            anchor: config.anchor,
            top: y + stack_offset,
            right: 0,
            bottom: 0,
            left: centered_left,
        },
        NotificationAnchor::TopRight => NotificationPlacement {
            anchor: config.anchor,
            top: y + stack_offset,
            right: x,
            bottom: 0,
            left: 0,
        },
        NotificationAnchor::BottomLeft => NotificationPlacement {
            anchor: config.anchor,
            top: 0,
            right: 0,
            bottom: y + stack_offset,
            left: x,
        },
        NotificationAnchor::BottomCenter => NotificationPlacement {
            anchor: config.anchor,
            top: 0,
            right: 0,
            bottom: y + stack_offset,
            left: centered_left,
        },
        NotificationAnchor::BottomRight => NotificationPlacement {
            anchor: config.anchor,
            top: 0,
            right: x,
            bottom: y + stack_offset,
            left: 0,
        },
    }
}

/// Render one compact notification using the currently selected panel theme.
pub fn render_notification(canvas: &mut Canvas, notification: &Notification, theme: &ThemeContext) {
    canvas.clear();
    let bounds = Rect::new(0.0, 0.0, canvas.width() as f32, canvas.height() as f32);
    canvas.fill_rounded_rect_alpha(
        bounds,
        theme.colors.background,
        theme.border_radius.max(6.0),
        theme.opacity,
    );

    let accent = Rect::new(0.0, 0.0, 4.0, bounds.height);
    canvas.fill_rounded_rect(accent, theme.colors.accent, 2.0);
    let content_x = 16.0;
    let content_width = (bounds.width - content_x - 12.0).max(1.0);
    let title = if notification.summary.trim().is_empty() {
        &notification.app_name
    } else {
        &notification.summary
    };
    canvas.draw_text_ellipsis(
        title,
        Rect::new(content_x, 10.0, content_width, 30.0),
        theme
            .fonts
            .bold_family
            .as_deref()
            .unwrap_or(&theme.fonts.family),
        theme.fonts.size.max(12.0),
        theme.colors.foreground,
    );
    if !notification.body.trim().is_empty() {
        let mut muted = theme.colors.foreground;
        muted[3] = muted[3].min(210);
        canvas.draw_text_ellipsis(
            &notification.body.replace('\n', " "),
            Rect::new(content_x, 46.0, content_width, 28.0),
            &theme.fonts.family,
            (theme.fonts.size - 1.0).max(11.0),
            muted,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> NotificationConfig {
        NotificationConfig {
            enabled: true,
            width: 400,
            gap: 8,
            ..NotificationConfig::default()
        }
    }

    fn request(id: u32, timeout: i32) -> NotificationRequest {
        NotificationRequest {
            id,
            app_name: "app".into(),
            app_icon: String::new(),
            summary: "summary".into(),
            body: "body".into(),
            actions: vec![NotificationAction {
                key: "open".into(),
                label: "Open".into(),
            }],
            expire_timeout_ms: timeout,
        }
    }

    #[test]
    fn replacement_keeps_one_item_and_resets_its_deadline() {
        let now = Instant::now();
        let mut center = NotificationCenter::default();
        assert_eq!(
            center.notify(request(7, 5), &config(), now),
            NotificationChange::Added(7)
        );
        assert_eq!(
            center.notify(request(7, 1_000), &config(), now),
            NotificationChange::Replaced(7)
        );
        assert_eq!(center.len(), 1);
        assert!(center.expire(now + Duration::from_millis(10)).is_empty());
    }

    #[test]
    fn expiry_removes_only_elapsed_notifications() {
        let now = Instant::now();
        let mut center = NotificationCenter::default();
        center.notify(request(1, 10), &config(), now);
        center.notify(request(2, 0), &config(), now);
        assert_eq!(
            center.expire(now + Duration::from_millis(11)),
            vec![NotificationChange::Closed(1)]
        );
        assert_eq!(center.len(), 1);
    }

    #[test]
    fn placements_stack_from_the_selected_anchor() {
        let mut config = config();
        config.anchor = NotificationAnchor::BottomRight;
        config.margin_x = 20;
        config.margin_y = 12;
        config.offset_x = -3;
        let first = notification_placement(&config, 1920, 0, 96);
        let second = notification_placement(&config, 1920, 1, 96);
        assert_eq!(first.right, 17);
        assert_eq!(first.bottom, 12);
        assert_eq!(second.bottom, 116);
    }

    #[test]
    fn centered_position_uses_output_width() {
        let mut config = config();
        config.anchor = NotificationAnchor::TopCenter;
        assert_eq!(notification_placement(&config, 1_000, 0, 96).left, 300);
    }
}
