//! Exercise the production bridge through a private, real session bus.
#[path = "../src/notifications.rs"]
mod notifications;

use std::collections::HashMap;
use std::time::Duration;

use notifications::{NotificationCommand, start_notification_service};
use tokio::sync::mpsc;
use zbus::{OwnedVariant, Variant};

#[test]
fn notification_dbus_round_trip() {
    let output = std::process::Command::new("dbus-run-session")
        .arg("--")
        .arg(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "private_bus_round_trip",
            "--nocapture",
        ])
        .env("HYPRDECK_PRIVATE_BUS_TEST", "1")
        .output()
        .expect("install dbus-run-session to run the isolated notification regression");
    assert!(
        output.status.success(),
        "private D-Bus test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
#[ignore = "launched only by notification_dbus_round_trip on its private bus"]
async fn private_bus_round_trip() {
    assert_eq!(
        std::env::var("HYPRDECK_PRIVATE_BUS_TEST").as_deref(),
        Ok("1")
    );
    tokio::time::timeout(Duration::from_secs(15), async {
        let existing_daemon = zbus::connection::Builder::session()
            .name("org.freedesktop.Notifications")
            .allow_name_replacements(true)
            .build()
            .await
            .unwrap();
        let (blocked_tx, _) = mpsc::unbounded_channel();
        assert!(
            start_notification_service(blocked_tx).await.is_err(),
            "startup must not replace even a daemon that permits replacement"
        );
        assert!(existing_daemon.release_name("org.freedesktop.Notifications").await.unwrap());
        drop(existing_daemon);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let service = start_notification_service(tx).await.unwrap();
        let (other_tx, _) = mpsc::unbounded_channel();
        assert!(
            start_notification_service(other_tx).await.is_err(),
            "a second service must not replace or queue behind the current owner"
        );
        let client = zbus::Connection::session().await.unwrap();
        let proxy = zbus::Proxy::new(
            &client,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await
        .unwrap();
        let caps: Vec<String> = proxy.call("GetCapabilities", &()).await.unwrap();
        assert_eq!(caps, ["body", "persistence"]);
        let info: (String, String, String, String) =
            proxy.call("GetServerInformation", &()).await.unwrap();
        assert_eq!(info.0, "HyprDeck");
        assert_eq!(info.2, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.3, "1.2");

        // a{sv} contains the basic and compound hint types used by real clients.
        let mut hints: HashMap<&str, OwnedVariant> = HashMap::new();
        hints.insert("urgency", 2_u8.into());
        hints.insert("resident", true.into());
        hints.insert("x", 42_i32.into());
        hints.insert(
            "category",
            Variant::from("im.received").try_to_owned().unwrap(),
        );
        hints.insert(
            "image-path",
            Variant::from("/tmp/icon.png").try_to_owned().unwrap(),
        );
        hints.insert(
            "image-data",
            Variant::from((
                1_i32,
                1_i32,
                4_i32,
                true,
                8_i32,
                4_i32,
                vec![255_u8, 0, 0, 255],
            ))
            .try_to_owned()
            .unwrap(),
        );
        let actions = vec!["default", "Open", "dismiss", "Dismiss", "orphan"];
        let id: u32 = proxy
            .call(
                "Notify",
                &(
                    "Test app",
                    0_u32,
                    "dialog-information",
                    "Hello 世界",
                    "A <b>body</b>",
                    &actions,
                    &hints,
                    -1_i32,
                ),
            )
            .await
            .unwrap();
        assert_ne!(id, 0);
        let NotificationCommand::Notify(request) = rx.recv().await.unwrap() else {
            panic!("expected notification");
        };
        assert_eq!(request.id, id);
        assert_eq!(request.app_name, "Test app");
        assert_eq!(request.app_icon, "dialog-information");
        assert_eq!(request.summary, "Hello 世界");
        assert_eq!(request.body, "A <b>body</b>");
        assert_eq!(request.expire_timeout_ms, -1);
        assert_eq!(request.actions.len(), 2);
        assert_eq!(request.actions[0].key, "default");
        assert_eq!(request.actions[1].label, "Dismiss");

        for timeout in [0_i32, 1200] {
            let replacement: u32 = proxy
                .call(
                    "Notify",
                    &(
                        "Test app",
                        id,
                        "",
                        "Updated",
                        "",
                        Vec::<String>::new(),
                        HashMap::<String, OwnedVariant>::new(),
                        timeout,
                    ),
                )
                .await
                .unwrap();
            assert_eq!(replacement, id);
            let NotificationCommand::Notify(request) = rx.recv().await.unwrap() else {
                panic!("expected replacement");
            };
            assert_eq!(request.id, id);
            assert_eq!(request.expire_timeout_ms, timeout);
            assert!(request.actions.is_empty());
        }
        let malformed: zbus::Result<u32> = proxy.call("Notify", &(42_u32,)).await;
        assert!(
            malformed.is_err(),
            "malformed wire signature must be rejected"
        );
        let _: () = proxy.call("CloseNotification", &(id,)).await.unwrap();
        assert!(matches!(rx.recv().await.unwrap(), NotificationCommand::Close(n) if n == id));
        let next: u32 = proxy
            .call(
                "Notify",
                &(
                    "Test app",
                    0_u32,
                    "",
                    "Next",
                    "",
                    Vec::<String>::new(),
                    HashMap::<String, OwnedVariant>::new(),
                    -1_i32,
                ),
            )
            .await
            .unwrap();
        assert!(next > id);
        assert!(matches!(rx.recv().await.unwrap(), NotificationCommand::Notify(n) if n.id == next));
        // Exercise an independent GLib serializer, not only the matching zbus client.
        let output = tokio::process::Command::new("gdbus")
            .args([
                "call", "--session", "--timeout", "5",
                "--dest", "org.freedesktop.Notifications",
                "--object-path", "/org/freedesktop/Notifications",
                "--method", "org.freedesktop.Notifications.Notify",
                "GDBus client", "0", "dialog-information", "External client", "External body",
                "['default', 'Open']",
                "{'urgency': <byte 1>, 'resident': <true>, 'image-data': <(1, 1, 4, true, 8, 4, [byte 255, 0, 0, 255])>}",
                "5000",
            ])
            .kill_on_drop(true)
            .output()
            .await
            .expect("install gdbus (libglib2.0-bin) to run the independent-client regression");
        assert!(output.status.success(), "gdbus call failed: {}", String::from_utf8_lossy(&output.stderr));
        let NotificationCommand::Notify(external) = rx.recv().await.unwrap() else {
            panic!("expected external client notification");
        };
        assert_eq!(external.app_name, "GDBus client");
        assert_eq!(external.summary, "External client");
        assert_eq!(external.body, "External body");
        assert_eq!(external.expire_timeout_ms, 5000);
        assert_eq!(external.actions[0].key, "default");
        assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), format!("(uint32 {},)", external.id));
        drop(service);
    })
    .await
    .expect("notification traffic timed out");
}
