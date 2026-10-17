//! Each lookup runs in a child process: the upstream directory cache is global.
use hyprdeck_modules::icon_utils::load_freedesktop_icon;
use image::{Rgba, RgbaImage};
use std::path::Path;
use std::process::Command;

fn png(path: &Path, size: u32, color: [u8; 4]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    RgbaImage::from_pixel(size, size, Rgba(color))
        .save(path)
        .unwrap();
}

fn theme(root: &Path, name: &str) {
    let dir = root.join("icons").join(name);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("index.theme"),
        format!(
            "[Icon Theme]\nName={name}\nDirectories=16/apps,24/apps,32/apps,48/apps\n\n{}",
            [16, 24, 32, 48]
                .map(|n| format!("[{n}/apps]\nSize={n}\nType=Fixed\nContext=Applications\n"))
                .join("\n")
        ),
    )
    .unwrap();
}

#[test]
fn icon_lookup_in_isolated_directories() {
    for case in [
        "default_and_format",
        "nearest_size",
        "path_order",
        "pixmaps",
        "svg",
        "missing",
    ] {
        let root =
            std::env::temp_dir().join(format!("hyprdeck-icon-{}-{case}", std::process::id()));
        std::fs::create_dir_all(root.join("home")).unwrap();
        for data in ["first", "second", "user"] {
            theme(&root.join(data), "hicolor");
        }
        theme(&root.join("first"), "OtherTheme");
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "isolated_lookup_case", "--nocapture"])
            .env("HYPRDECK_ICON_CASE", case)
            .env("HYPRDECK_ICON_ROOT", &root)
            .env("HOME", root.join("home"))
            .env("XDG_DATA_HOME", root.join("user"))
            .env(
                "XDG_DATA_DIRS",
                std::env::join_paths([root.join("first"), root.join("second")]).unwrap(),
            )
            .env("XDG_CONFIG_HOME", root.join("config"))
            .output()
            .unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn isolated_lookup_case() {
    let Ok(case) = std::env::var("HYPRDECK_ICON_CASE") else {
        return;
    };
    let root = std::path::PathBuf::from(std::env::var_os("HYPRDECK_ICON_ROOT").unwrap());
    let name = "hyprdeck-fixture-unique-icon";
    let red = [255, 0, 0, 255];
    let green = [0, 255, 0, 255];
    let first = root.join("first/icons/hicolor");
    match case.as_str() {
        "default_and_format" => {
            png(&first.join(format!("24/apps/{name}.png")), 24, red);
            png(
                &root.join(format!("first/icons/OtherTheme/24/apps/{name}.png")),
                24,
                green,
            );
            std::fs::write(first.join(format!("24/apps/{name}.svg")), "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='lime'/></svg>").unwrap();
            let icon = load_freedesktop_icon(name, 24).unwrap();
            assert_eq!(icon.dimensions(), (24, 24));
            assert_eq!(icon.get_pixel(0, 0).0, red);
        }
        "nearest_size" => {
            png(&first.join(format!("16/apps/{name}.png")), 16, red);
            png(&first.join(format!("32/apps/{name}.png")), 32, green);
            png(&first.join(format!("48/apps/{name}.png")), 48, red);
            let icon = load_freedesktop_icon(name, 28).unwrap();
            assert_eq!(icon.dimensions(), (32, 32));
            assert_eq!(icon.get_pixel(0, 0).0, green);
        }
        "path_order" => {
            png(&first.join(format!("24/apps/{name}.png")), 24, red);
            png(
                &root.join(format!("second/icons/hicolor/24/apps/{name}.png")),
                24,
                green,
            );
            png(
                &root.join(format!("user/icons/hicolor/24/apps/{name}.png")),
                24,
                green,
            );
            assert_eq!(
                load_freedesktop_icon(name, 24).unwrap().get_pixel(0, 0).0,
                red
            );
        }
        "pixmaps" => {
            png(&root.join(format!("second/pixmaps/{name}.png")), 24, green);
            assert_eq!(
                load_freedesktop_icon(name, 24).unwrap().get_pixel(0, 0).0,
                green
            );
        }
        "svg" => {
            let path = first.join(format!("24/apps/{name}.svg"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "<svg xmlns='http://www.w3.org/2000/svg' width='24' height='24'><rect width='24' height='24' fill='red' fill-opacity='0.5'/></svg>").unwrap();
            let icon = load_freedesktop_icon(name, 24).unwrap();
            assert_eq!(icon.dimensions(), (24, 24));
            let p = icon.get_pixel(12, 12).0;
            assert_eq!(&p[..3], &[255, 0, 0]);
            assert!((127..=128).contains(&p[3]));
        }
        "missing" => {
            std::fs::create_dir_all(root.join("first/icons/broken")).unwrap();
            std::fs::write(
                root.join("first/icons/broken/index.theme"),
                "not an ini theme",
            )
            .unwrap();
            assert!(load_freedesktop_icon(name, 24).is_none());
            png(&first.join(format!("24/apps/{name}.png")), 24, green);
            assert!(
                load_freedesktop_icon(name, 24).is_some(),
                "a missing result must not be cached"
            );
        }
        _ => panic!("unknown fixture case"),
    }
}
