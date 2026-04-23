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
