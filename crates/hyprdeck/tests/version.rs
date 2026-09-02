use assert_cmd::Command;

#[test]
fn version_flag_prints_crate_version() {
    let expected = format!("hyprdeck {}\n", env!("CARGO_PKG_VERSION"));
    Command::cargo_bin("hyprdeck")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn short_version_flag_prints_crate_version() {
    let expected = format!("hyprdeck {}\n", env!("CARGO_PKG_VERSION"));
    Command::cargo_bin("hyprdeck")
        .unwrap()
        .arg("-V")
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn config_schema_flag_prints_versioned_json_without_wayland() {
    let assert = Command::cargo_bin("hyprdeck")
        .unwrap()
        .arg("--print-config-schema")
        .assert()
        .success();
    let json: serde_json::Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    assert_eq!(json["contract_version"], 1);
    assert!(json["modules"].as_array().unwrap().len() >= 12);
    assert!(json["fields"].as_array().unwrap().len() >= 10);
    assert_eq!(json["themes"].as_array().unwrap().len(), 5);
}

#[test]
fn validate_config_flag_does_not_connect_to_wayland() {
    let path = std::env::temp_dir().join(format!(
        "hyprdeck-cli-config-{}-{}.toml",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    std::fs::write(&path, "theme = \"win7\"\n").unwrap();
    Command::cargo_bin("hyprdeck")
        .unwrap()
        .arg("--validate-config")
        .arg(&path)
        .assert()
        .success()
        .stdout("[]\n");
    std::fs::remove_file(path).unwrap();
}
