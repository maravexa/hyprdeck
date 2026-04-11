use assert_cmd::Command;
use predicates::prelude::*;

fn fnord() -> Command {
    Command::cargo_bin("fnord").unwrap()
}

#[test]
fn test_no_args_exits_zero_and_prints_date() {
    fnord()
        .assert()
        .success()
        .stdout(predicate::str::contains("YOLD"));
}

#[test]
fn test_date_subcommand_today() {
    fnord()
        .arg("date")
        .assert()
        .success()
        .stdout(predicate::str::contains("YOLD"));
}

#[test]
fn test_date_mungday() {
    fnord()
        .args(["date", "--date", "2025-01-05"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mungday"));
}

#[test]
fn test_date_st_tibs() {
    fnord()
        .args(["date", "--date", "2024-02-29"])
        .assert()
        .success()
        .stdout(predicate::str::contains("St. Tib's Day"));
}

#[test]
fn test_date_short_exits_zero() {
    fnord().args(["date", "--short"]).assert().success();
}

#[test]
fn test_cal_exits_zero() {
    fnord()
        .arg("cal")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_help_lists_subcommands() {
    fnord()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("date"))
        .stdout(predicate::str::contains("cal"))
        .stdout(predicate::str::contains("pope"));
}

#[test]
fn test_date_help() {
    fnord()
        .args(["date", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--date"))
        .stdout(predicate::str::contains("--format"));
}

#[test]
fn test_date_format_flag() {
    fnord()
        .args(["date", "--date", "2025-01-01", "--format", "%A %B %d %Y"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Sweetmorn"))
        .stdout(predicate::str::contains("Chaos"))
        .stdout(predicate::str::contains("3191"));
}

#[test]
fn test_date_json_flag() {
    fnord()
        .args(["date", "--date", "2025-01-05", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"year\""))
        .stdout(predicate::str::contains("3191"));
}

#[test]
fn test_cal_all_flag() {
    fnord()
        .args(["cal", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Chaos"))
        .stdout(predicate::str::contains("Discord"))
        .stdout(predicate::str::contains("Aftermath"));
}

#[test]
fn test_cal_specific_season() {
    fnord()
        .args(["cal", "--season", "discord"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Discord"));
}

#[test]
fn test_fnord_redact_reads_stdin() {
    fnord()
        .arg("fnord")
        .arg("--rate")
        .arg("1.0")
        .arg("--pure-chaos")
        .arg("--seed")
        .arg("test")
        .write_stdin("hello world\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("fnord").or(predicate::str::contains("FNORD")));
}

#[test]
fn test_cabbage_reads_stdin() {
    fnord()
        .arg("cabbage")
        .arg("-c")
        .write_stdin("one\ntwo\nthree\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("3"));
}

#[test]
fn test_chaos_reads_stdin() {
    fnord()
        .arg("chaos")
        .arg("--seed")
        .arg("test")
        .write_stdin("a\nb\nc\n")
        .assert()
        .success();
}

#[test]
fn test_law_searches_stdin() {
    fnord()
        .args(["law", "fox"])
        .write_stdin("the quick brown fox\nno match here\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("fox"));
}

#[test]
fn test_date_offset_positive() {
    fnord()
        .args(["date", "--date", "+0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("YOLD"));
}

#[test]
fn test_date_today_keyword() {
    fnord()
        .args(["date", "--date", "today"])
        .assert()
        .success()
        .stdout(predicate::str::contains("YOLD"));
}

// ─── pope ──────────────────────────────────────────────────────────────────

#[test]
fn pope_exits_zero() {
    fnord()
        .arg("pope")
        .env("USER", "eris")
        .env("HOSTNAME", "archbox")
        .assert()
        .success()
        .stdout(predicate::str::contains("PAPAL DECLARATION"));
}

#[test]
fn pope_short_contains_username() {
    fnord()
        .args(["pope", "--short"])
        .env("USER", "eris")
        .env("HOSTNAME", "archbox")
        .assert()
        .success()
        .stdout(predicate::str::contains("eris:"));
}

#[test]
fn pope_bull_contains_decrees() {
    fnord()
        .args(["pope", "--bull"])
        .env("USER", "eris")
        .env("HOSTNAME", "archbox")
        .assert()
        .success()
        .stdout(predicate::str::contains("BULLA DISCORDIANA"))
        .stdout(predicate::str::contains("PAPAL DECREES"))
        .stdout(predicate::str::contains("Eris, Goddess of Chaos"));
}

#[test]
fn pope_json_is_valid_json() {
    let output = fnord()
        .args(["pope", "--json"])
        .env("USER", "eris")
        .env("HOSTNAME", "archbox")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    assert!(v.get("user").is_some());
    assert!(v.get("pope_title").is_some());
}

// ─── oracle ────────────────────────────────────────────────────────────────

#[test]
fn oracle_with_question_exits_zero() {
    fnord()
        .args(["oracle", "is a hotdog a sandwich"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ORACLE"));
}

#[test]
fn oracle_deterministic() {
    let out1 = fnord()
        .args(["oracle", "is a hotdog a sandwich"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out2 = fnord()
        .args(["oracle", "is a hotdog a sandwich"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(out1, out2, "oracle should be deterministic");
}

#[test]
fn oracle_json_is_valid_json() {
    let output = fnord()
        .args(["oracle", "what is a fnord", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    assert!(v.get("question").is_some());
    assert!(v.get("answer").is_some());
    assert!(v.get("seed").is_some());
}

// ─── fortune ───────────────────────────────────────────────────────────────

#[test]
fn fortune_exits_zero() {
    fnord()
        .arg("fortune")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn fortune_count_returns_n() {
    let output = fnord()
        .args(["fortune", "--count", "3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    // 3 fortunes separated by "%\n" means 2 separators.
    let separators = s.lines().filter(|l| *l == "%").count();
    assert_eq!(
        separators, 2,
        "expected 2 separators for 3 fortunes, got {separators}: {s}"
    );
}

#[test]
fn fortune_json_is_valid_json() {
    let output = fnord()
        .args(["fortune", "--json", "--count", "2"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    let arr = v.as_array().expect("expected array");
    assert_eq!(arr.len(), 2);
    assert!(arr[0].get("text").is_some());
    assert!(arr[0].get("tags").is_some());
    assert!(arr[0].get("source").is_some());
}

// ─── koan ──────────────────────────────────────────────────────────────────

#[test]
fn koan_exits_zero() {
    fnord()
        .arg("koan")
        .assert()
        .success()
        .stdout(predicate::str::contains("KOAN"));
}

#[test]
fn koan_count_returns_n() {
    let output = fnord()
        .args(["koan", "--count", "3"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let count = s.matches("KOAN").count();
    assert_eq!(count, 3, "expected 3 KOAN headings, got {count}: {s}");
}

#[test]
fn koan_seed_is_reproducible() {
    let out1 = fnord()
        .args(["koan", "--seed", "kallisti"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out2 = fnord()
        .args(["koan", "--seed", "kallisti"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(out1, out2, "same seed should produce same koan");
}

#[test]
fn koan_json_is_valid_json() {
    let output = fnord()
        .args(["koan", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    let arr = v.as_array().expect("expected array");
    assert_eq!(arr.len(), 1);
    assert!(arr[0].get("setup").is_some());
    assert!(arr[0].get("question").is_some());
    assert!(arr[0].get("response").is_some());
}

// ─── moon ──────────────────────────────────────────────────────────────────

#[test]
fn moon_exits_zero() {
    fnord()
        .arg("moon")
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn moon_phobos_exits_zero() {
    fnord()
        .args(["moon", "--body", "phobos"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Phobos"));
}

#[test]
fn moon_next_exits_zero() {
    fnord()
        .args(["moon", "--body", "titan", "--next"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Next full moon"))
        .stdout(predicate::str::contains("Next new moon"));
}

#[test]
fn moon_json_is_valid_json() {
    let output = fnord()
        .args(["moon", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    assert!(v.get("body").is_some());
    assert!(v.get("phase_name").is_some());
    assert!(v.get("phase_angle").is_some());
    assert!(v.get("illumination_pct").is_some());
}

#[test]
fn moon_random_exits_zero() {
    fnord()
        .args(["moon", "--body", "random"])
        .assert()
        .success();
}

#[test]
fn moon_season_exits_zero() {
    fnord()
        .args(["moon", "--season"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ─── zodiac ────────────────────────────────────────────────────────────────

#[test]
fn zodiac_exits_zero() {
    fnord()
        .arg("zodiac")
        .assert()
        .success()
        .stdout(predicate::str::contains("ZODIAC"));
}

#[test]
fn zodiac_discordian_exits_zero() {
    fnord()
        .args(["zodiac", "--system", "discordian"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DISCORDIAN"));
}

#[test]
fn zodiac_all_systems_exit_zero() {
    for system in &["western", "vedic", "chinese", "discordian"] {
        fnord()
            .args(["zodiac", "--system", system])
            .assert()
            .success();
    }
}

#[test]
fn zodiac_json_is_valid_json() {
    let output = fnord()
        .args(["zodiac", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    assert!(v.get("system").is_some());
    assert!(v.get("sign").is_some());
    assert!(v.get("description").is_some());
    assert!(v.get("date").is_some());
}

#[test]
fn zodiac_jul4_is_cancer() {
    fnord()
        .args(["zodiac", "--system", "western", "--date", "1984-07-04"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Cancer"));
}

// ─── omens ─────────────────────────────────────────────────────────────────

#[test]
fn omens_generative_exits_zero() {
    fnord()
        .args(["omens", "--generative"])
        .assert()
        .success()
        .stdout(predicate::str::contains("OMENS"));
}

#[test]
fn omens_generative_is_deterministic() {
    let out1 = fnord()
        .args(["omens", "--generative", "--date", "2025-06-15"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let out2 = fnord()
        .args(["omens", "--generative", "--date", "2025-06-15"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(out1, out2, "generative omens should be deterministic");
}

#[test]
fn omens_generative_discordian_units() {
    fnord()
        .args(["omens", "--generative", "--units", "discordian"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Fn"))
        .stdout(predicate::str::contains("Cabbage Units"));
}

#[test]
fn omens_generative_raw_exits_zero() {
    fnord()
        .args(["omens", "--generative", "--raw"])
        .assert()
        .success()
        .stdout(predicate::str::contains("raw"));
}

#[test]
fn omens_json_is_valid_json() {
    let output = fnord()
        .args(["omens", "--generative", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    assert!(v.get("raw").is_some());
    assert!(v.get("discordian").is_some());
    assert!(v.get("interpretation").is_some());
    assert!(v.get("directive").is_some());
}

// ─── log ───────────────────────────────────────────────────────────────────

fn temp_grimoire_path(name: &str) -> std::path::PathBuf {
    // Deliberately leak the TempDir handle so the directory outlives the
    // test; each test creates a unique directory so there's no collision.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(name);
    std::mem::forget(dir);
    path
}

#[test]
fn log_writes_entry_to_tempfile() {
    let path = temp_grimoire_path("grimoire");
    fnord()
        .args([
            "log",
            "Test entry for the grimoire",
            "--file",
            path.to_str().unwrap(),
        ])
        .assert()
        .success();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("Test entry for the grimoire"));
    assert!(content.contains("YOLD"));
}

#[test]
fn log_list_exits_zero() {
    let path = temp_grimoire_path("list-grimoire");
    fnord()
        .args(["log", "first entry", "--file", path.to_str().unwrap()])
        .assert()
        .success();
    fnord()
        .args(["log", "second entry", "--file", path.to_str().unwrap()])
        .assert()
        .success();
    fnord()
        .args(["log", "--list", "--file", path.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("second entry"));
}

#[test]
fn log_creates_file_if_not_exists() {
    let path = temp_grimoire_path("nested/path/grimoire");
    fnord()
        .args(["log", "hello", "--file", path.to_str().unwrap()])
        .assert()
        .success();
    assert!(path.exists());
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# Grimoire of "));
    assert!(content.contains("hello"));
}

#[test]
fn log_json_list_is_valid_json() {
    let path = temp_grimoire_path("json-grimoire");
    fnord()
        .args(["log", "one", "--file", path.to_str().unwrap()])
        .assert()
        .success();
    fnord()
        .args(["log", "two", "--file", path.to_str().unwrap()])
        .assert()
        .success();
    let output = fnord()
        .args(["log", "--list", "--file", path.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    let arr = v.as_array().expect("expected array");
    assert!(arr.len() >= 2);
    assert!(arr[0].get("index").is_some());
    assert!(arr[0].get("discordian_date").is_some());
    assert!(arr[0].get("body").is_some());
}

// ─── wake ──────────────────────────────────────────────────────────────────

#[test]
fn wake_exits_zero() {
    fnord()
        .arg("wake")
        .assert()
        .success()
        .stdout(predicate::str::contains("All Hail Eris"));
}

#[test]
fn wake_with_fortune_exits_zero() {
    fnord()
        .args(["wake", "--fortune"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn wake_json_is_valid_json() {
    let output = fnord()
        .args(["wake", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    assert!(v.get("discordian_date").is_some());
    assert!(v.get("date_lines").is_some());
}

#[test]
fn wake_no_unicode_exits_zero() {
    fnord()
        .args(["wake", "--no-unicode"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

// ─── pineal ────────────────────────────────────────────────────────────────

#[test]
fn pineal_exits_zero() {
    fnord()
        .arg("pineal")
        .assert()
        .success()
        .stdout(predicate::str::contains("PINEAL REPORT"));
}

#[test]
fn pineal_minimal_exits_zero() {
    fnord()
        .args(["pineal", "--verbosity", "minimal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("/100"));
}

#[test]
fn pineal_enlightened_exits_zero() {
    fnord()
        .args(["pineal", "--verbosity", "enlightened"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PROPHECY"));
}

#[test]
fn pineal_json_is_valid_json() {
    let output = fnord()
        .args(["pineal", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    assert!(v.get("host").is_some());
    assert!(v.get("consciousness").is_some());
    assert!(v.get("uptime_seconds").is_some());
    assert!(v.get("memory").is_some());
}

#[test]
fn pineal_raw_exits_zero() {
    fnord()
        .args(["pineal", "--raw"])
        .assert()
        .success()
        .stdout(predicate::str::contains("raw system values"));
}

// ─── holyday subcommand ────────────────────────────────────────────────────────

#[test]
fn holyday_list_exits_zero() {
    fnord()
        .args(["holyday", "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mungday"));
}

#[test]
fn holyday_show_exits_zero() {
    fnord().args(["holyday", "show"]).assert().success();
}

#[test]
fn holyday_list_json_is_valid_json() {
    let output = fnord()
        .args(["holyday", "list", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON");
    let arr = v.as_array().expect("expected array");
    assert!(!arr.is_empty());
    assert!(arr[0].get("key").is_some());
    assert!(arr[0].get("name").is_some());
}

// ─── format strings ────────────────────────────────────────────────────────────

#[test]
fn date_format_weekday_token() {
    // %A should produce a weekday name
    let output = fnord()
        .args(["date", "--date", "2025-01-01", "--format", "%A"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    assert!(
        [
            "Sweetmorn",
            "Boomtime",
            "Pungenday",
            "Prickle-Prickle",
            "Setting Orange"
        ]
        .iter()
        .any(|w| s.contains(w)),
        "%A did not produce a weekday name: {s}"
    );
}

#[test]
fn date_format_season_token() {
    let output = fnord()
        .args(["date", "--date", "2025-01-01", "--format", "%B"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    assert!(
        ["Chaos", "Discord", "Confusion", "Bureaucracy", "Aftermath"]
            .iter()
            .any(|w| s.contains(w)),
        "%B did not produce a season name: {s}"
    );
}

#[test]
fn date_format_year_token() {
    let output = fnord()
        .args(["date", "--date", "2025-01-01", "--format", "%Y"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    assert!(s.contains("3191"), "%Y did not produce YOLD 3191: {s}");
}

#[test]
fn date_help_format_exits_zero() {
    fnord()
        .args(["date", "--help-format"])
        .assert()
        .success()
        .stdout(predicate::str::contains("FORMAT TOKENS"))
        .stdout(predicate::str::contains("%A"))
        .stdout(predicate::str::contains("%Y"));
}

// ─── shell completions binary ──────────────────────────────────────────────────

#[test]
fn generate_completions_binary_exists() {
    // The binary is only built with --features generate-assets,
    // so we just verify the source file is present.
    assert!(
        std::path::Path::new("src/bin/generate-completions.rs").exists(),
        "generate-completions source not found"
    );
}

// ─── global flags ──────────────────────────────────────────────────────────────

#[test]
fn no_color_flag_produces_no_ansi() {
    let output = fnord().args(["--no-color", "date"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains('\x1B'),
        "output contains ANSI codes despite --no-color"
    );
}

#[test]
fn no_unicode_flag_produces_no_unicode() {
    let output = fnord().args(["--no-unicode", "wake"]).output().unwrap();
    assert!(output.status.success());
    // Output should not contain box-drawing characters (U+2500+)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let has_box_drawing = stdout
        .chars()
        .any(|c| ('\u{2500}'..='\u{257F}').contains(&c));
    assert!(
        !has_box_drawing,
        "output contains box-drawing chars despite --no-unicode"
    );
}

#[test]
fn json_flag_on_date_is_valid_json() {
    let output = fnord()
        .args(["--json", "date"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let s = String::from_utf8(output).unwrap();
    let v: serde_json::Value = serde_json::from_str(&s).expect("invalid JSON from --json date");
    assert!(v.get("year").is_some());
}

#[test]
fn version_flag_exits_zero() {
    fnord()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("0.1.0"));
}

#[test]
fn help_flag_exits_zero() {
    fnord()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("fnord"));
}
