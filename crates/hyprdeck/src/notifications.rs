//! `org.freedesktop.Notifications` service bridge.
//!
//! The service has no rendering knowledge: it sends requests to the Wayland
//! event loop, which owns notification lifetimes and layer-shell surfaces.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use hyprdeck_core::{NotificationAction, NotificationRequest};
use tokio::sync::mpsc;
use zbus::zvariant::OwnedValue;

#[derive(Debug)]
pub enum NotificationCommand {
    Notify(NotificationRequest),
    Close(u32),
}

struct NotificationService {
    tx: mpsc::UnboundedSender<NotificationCommand>,
    next_id: AtomicU32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl NotificationService {
    /// Display or replace a notification.
    #[allow(clippy::too_many_arguments)] // D-Bus signature mandated by the specification.
    async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        _hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id == 0 {
            self.next_id.fetch_add(1, Ordering::Relaxed).max(1)
        } else {
            replaces_id
        };
        let actions = actions
            .chunks_exact(2)
            .map(|pair| NotificationAction {
                key: pair[0].clone(),
                label: pair[1].clone(),
            })
            .collect();
        let request = NotificationRequest {
            id,
            app_name,
            app_icon,
            summary,
            body,
            actions,
            expire_timeout_ms: expire_timeout,
        };
        if self.tx.send(NotificationCommand::Notify(request)).is_err() {
            tracing::warn!("notification service stopped before request could be displayed");
        }
        id
    }

    async fn close_notification(&self, id: u32) {
        if self.tx.send(NotificationCommand::Close(id)).is_err() {
            tracing::warn!("notification service stopped before notification could close");
        }
    }

    async fn get_capabilities(&self) -> Vec<String> {
        // Actions are retained in the model but not advertised until notification
        // surfaces have input routing and ActionInvoked signal support.
        vec!["body".into(), "persistence".into()]
    }

    async fn get_server_information(&self) -> (String, String, String, String) {
        (
            "HyprDeck".into(),
            "HyprDeck".into(),
            env!("CARGO_PKG_VERSION").into(),
            "1.2".into(),
        )
    }
}

/// Claim the desktop-notification well-known name and start serving requests.
/// The returned connection must remain alive for the daemon to remain active.
pub async fn start_notification_service(
    tx: mpsc::UnboundedSender<NotificationCommand>,
) -> zbus::Result<zbus::Connection> {
    zbus::connection::Builder::session()?
        .name("org.freedesktop.Notifications")?
        .serve_at(
            "/org/freedesktop/Notifications",
            NotificationService {
                tx,
                next_id: AtomicU32::new(1),
            },
        )?
        .build()
        .await
}
