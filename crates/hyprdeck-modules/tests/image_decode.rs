use hyprdeck_modules::icon_utils::load_icon_from_path;
use std::path::Path;

#[test]
fn png_and_ico_preserve_rgba_pixels() {
    for name in ["rgba.png", "palette.png", "png.ico", "bmp.ico"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let image = load_icon_from_path(&path).unwrap_or_else(|| panic!("decode {name}"));
        assert_eq!(image.dimensions(), (2, 1), "{name}");
        assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 128], "{name}");
        assert_eq!(image.get_pixel(1, 0).0, [0, 255, 0, 255], "{name}");
    }
}

#[test]
fn invalid_icon_remains_a_missing_icon() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/invalid.png");
    assert!(load_icon_from_path(&path).is_none());
}
