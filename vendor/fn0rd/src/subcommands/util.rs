//! Small utilities shared by the pope / oracle / fortune / koan
//! subcommands: deterministic hashing, hostname/username lookup,
//! and ASCII/unicode bullet selection.

use std::hash::Hasher;

use fnv::FnvHasher;

/// Deterministic, platform-stable FNV-1a hash of a string.
pub fn hash_str(s: &str) -> u64 {
    let mut h = FnvHasher::default();
    h.write(s.as_bytes());
    h.finish()
}

/// Pick an element from `list` using `seed`. Panics if `list` is empty,
/// which is fine — all wordlists are compile-time and non-empty.
pub fn pick<T>(list: &[T], seed: u64) -> &T {
    &list[(seed as usize) % list.len()]
}

/// Current user from `$USER`, falling back to "anonymous".
pub fn current_user() -> String {
    std::env::var("USER")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "anonymous".to_string())
}

/// Best-effort hostname lookup: `$HOSTNAME`, then `/etc/hostname`,
/// then the `uname -n` command, then "localhost".
pub fn hostname() -> String {
    if let Ok(h) = std::env::var("HOSTNAME") {
        if !h.is_empty() {
            return h;
        }
    }
    if let Ok(s) = std::fs::read_to_string("/etc/hostname") {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    if let Ok(output) = std::process::Command::new("uname").arg("-n").output() {
        if output.status.success() {
            if let Ok(s) = String::from_utf8(output.stdout) {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    "localhost".to_string()
}

/// Return the "sparkle" symbol based on the --no-unicode flag and
/// the config's `output.unicode` setting. Defaults to `✦`, falls back
/// to `*` in ASCII mode.
pub fn sparkle(no_unicode: bool) -> &'static str {
    if no_unicode {
        "*"
    } else {
        "✦"
    }
}
