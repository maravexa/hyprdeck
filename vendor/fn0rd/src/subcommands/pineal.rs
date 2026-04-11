//! `fnord pineal` — system information reframed as a Discordian
//! consciousness report. Uses `sysinfo` for metrics; consciousness level
//! is derived from uptime, load average, RAM, and core count.

use chrono::Local;
use owo_colors::OwoColorize;
use serde_json::json;
use sysinfo::{CpuRefreshKind, LoadAvg, MemoryRefreshKind, RefreshKind, System};

use crate::cli::PinealArgs;
use crate::config::Config;
use crate::date::convert::to_discordian;
use crate::error::FnordError;
use crate::subcommands::util::{current_user, hash_str, hostname, sparkle};

/// Hardcoded list of prophecies. Selection is deterministic based on
/// `hash(hostname + date)`.
pub const PROPHECIES: &[&str] = &[
    "Your RAM is full of forgotten intentions. Consider releasing them.",
    "The kernel has made 847,293 decisions today. Most were correct.",
    "Five cores hum in harmony. The other seven are just along for the ride.",
    "Uptime is a measure of commitment. Or stubbornness. The Oracle is unclear.",
    "Your CPU temperature is not shown here. This is intentional. Do not check.",
    "The load average approaches 0. Nirvana, or a broken cron job.",
    "Memory usage is 44%. The other 56% is potential. Or fnords.",
    "Every process is sacred. Except that one. You know the one.",
    "A kernel panic is just the machine being honest for once.",
    "Swap space is where dreams go when the RAM gets too crowded.",
    "Your uptime suggests you have forgotten how to reboot. This is wisdom.",
    "The Oracle notes your CPU is idle. The Oracle finds this suspicious.",
    "Every file descriptor is a tiny prayer. Close them gently.",
    "Hostnames are incantations. Choose yours with care.",
    "The fnords live in the page cache. Do not evict them.",
    "Your load average is 0.23. The sacred number would approve.",
    "One zombie process walks among the living. Offer it cabbages.",
];

/// Verbosity level for pineal output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verbosity {
    Minimal,
    Normal,
    Enlightened,
}

impl Verbosity {
    pub fn parse(s: &str) -> Result<Self, FnordError> {
        match s.to_lowercase().as_str() {
            "minimal" | "min" | "short" => Ok(Verbosity::Minimal),
            "normal" | "default" | "" => Ok(Verbosity::Normal),
            "enlightened" | "full" | "verbose" => Ok(Verbosity::Enlightened),
            other => Err(FnordError::Parse(format!("unknown verbosity: '{other}'"))),
        }
    }
}

/// Collected system metrics used to compute consciousness.
#[derive(Debug, Clone)]
pub struct PinealMetrics {
    pub host: String,
    pub user: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub cpu_model: String,
    pub core_count: usize,
    pub per_cpu_usage: Vec<f32>,
    pub total_ram_bytes: u64,
    pub used_ram_bytes: u64,
    pub uptime_seconds: u64,
    pub load_1m: f64,
    pub load_5m: f64,
    pub load_15m: f64,
}

impl PinealMetrics {
    pub fn ram_used_pct(&self) -> f64 {
        if self.total_ram_bytes == 0 {
            0.0
        } else {
            100.0 * (self.used_ram_bytes as f64) / (self.total_ram_bytes as f64)
        }
    }
}

/// Collect metrics from `sysinfo`. Host and user fall back through the
/// same helpers used by `pope`/`oracle`.
pub fn collect_metrics() -> PinealMetrics {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    sys.refresh_cpu();
    sys.refresh_memory();

    let host = System::host_name().unwrap_or_else(hostname);
    let user = current_user();
    let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
    let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
    let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());

    let cpus = sys.cpus();
    let core_count = cpus.len();
    let cpu_model = cpus
        .first()
        .map(|c| c.brand().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());
    let per_cpu_usage: Vec<f32> = cpus.iter().map(|c| c.cpu_usage()).collect();

    let total_ram_bytes = sys.total_memory();
    let used_ram_bytes = sys.used_memory();
    let uptime_seconds = System::uptime();
    let LoadAvg { one, five, fifteen } = System::load_average();

    PinealMetrics {
        host,
        user,
        os_name,
        os_version,
        kernel_version,
        cpu_model,
        core_count,
        per_cpu_usage,
        total_ram_bytes,
        used_ram_bytes,
        uptime_seconds,
        load_1m: one,
        load_5m: five,
        load_15m: fifteen,
    }
}

/// Consciousness level 0..=100, derived from the metrics per the formula
/// in the spec.
pub fn consciousness_level(m: &PinealMetrics) -> i32 {
    let uptime_days = (m.uptime_seconds / 86_400) as i64;
    let mut level: i32 = 23;
    level += ((uptime_days % 5) * 5) as i32;
    if m.load_1m < 1.0 {
        level += 10;
    }
    if m.ram_used_pct() < 50.0 {
        level += 7;
    }
    if m.core_count >= 5 {
        level += 5;
    }
    if m.load_1m > 4.0 {
        level -= 15;
    }
    level.clamp(0, 100)
}

/// Map a consciousness level to a human-readable label.
pub fn consciousness_label(level: i32) -> &'static str {
    match level {
        0..=20 => "Greyface Mode — seek disorder immediately",
        21..=40 => "Mildly Confused — promising",
        41..=60 => "Erisian Awareness — the fnords are becoming visible",
        61..=80 => "Illuminated — All Hail Discordia",
        81..=99 => "Transcendent Bureaucrat — you have filed all forms",
        100 => "Pope-Level Consciousness — Kallisti",
        _ => "Unknown Consciousness",
    }
}

/// Pick a prophecy deterministically based on hostname + date.
pub fn pick_prophecy(host: &str, date: &str) -> &'static str {
    let h = hash_str(&format!("{host}:{date}"));
    PROPHECIES[(h as usize) % PROPHECIES.len()]
}

/// Entry point used by main.
pub fn run(
    args: &PinealArgs,
    _config: &Config,
    json_out: bool,
    no_color: bool,
    no_unicode: bool,
) -> Result<(), FnordError> {
    let metrics = collect_metrics();
    let level = consciousness_level(&metrics);
    let label = consciousness_label(level);

    let verbosity = match &args.verbosity {
        Some(s) => Verbosity::parse(s)?,
        None => Verbosity::Normal,
    };

    if json_out {
        return print_json(&metrics, level, label);
    }

    match verbosity {
        Verbosity::Minimal => print_minimal(&metrics, level, label),
        Verbosity::Normal => print_normal(&metrics, level, label, no_color, no_unicode),
        Verbosity::Enlightened => {
            print_normal(&metrics, level, label, no_color, no_unicode);
            print_enlightened_extras(&metrics, no_color, no_unicode);
        }
    }

    if args.raw {
        print_raw(&metrics, level);
    }

    Ok(())
}

fn print_minimal(m: &PinealMetrics, level: i32, label: &str) {
    // Short label: take only the part before "—" if present.
    let short_label = label.split('—').next().unwrap_or(label).trim();
    println!("{}: {level}/100 — {short_label}", m.host);
}

fn print_normal(m: &PinealMetrics, level: i32, label: &str, no_color: bool, no_unicode: bool) {
    let sp = sparkle(no_unicode);
    let heading = format!("  {sp} PINEAL REPORT {sp}");
    let subheading = "  Discordian Systems Consciousness Assessment";
    println!();
    if no_color {
        println!("{heading}");
    } else {
        println!("{}", heading.bold().magenta());
    }
    println!("{subheading}");
    println!();

    println!("  HOST ENTITY:    {} ({})", m.host, host_style(&m.host));
    println!("  CONSCIOUSNESS:  {level}/100 — {label}");
    let uptime_pretty = format_uptime(m.uptime_seconds);
    let disc_weeks = (m.uptime_seconds as f64) / (5.0 * 86_400.0);
    println!("  UPTIME:         {uptime_pretty}");
    println!(
        "                  ({disc_weeks:.2} Discordian weeks — approaching enlightenment cycle)"
    );

    let total_gb = bytes_to_gb(m.total_ram_bytes);
    let used_gb = bytes_to_gb(m.used_ram_bytes);
    let ram_pct = m.ram_used_pct();
    let ram_flavor = if ram_pct < 50.0 {
        "spacious"
    } else if ram_pct < 80.0 {
        "tolerable"
    } else {
        "bureaucratic"
    };
    println!(
        "  MIND SPACE:     {used_gb:.1} / {total_gb:.1} GB utilized ({ram_pct:.0}% — {ram_flavor})"
    );

    let core_flavor = core_flavor_text(m.core_count);
    println!(
        "  PROCESSING:     {} — {} cores ({core_flavor})",
        m.cpu_model.trim(),
        m.core_count
    );
    println!(
        "  KERNEL:         {} {} (order attempting to manage chaos)",
        m.os_name, m.kernel_version
    );
    let chao = chao_balance(m.load_1m);
    println!(
        "  LOAD:           {:.2} / {:.2} / {:.2} — {chao}",
        m.load_1m, m.load_5m, m.load_15m
    );

    println!();
    let assessment = assessment_line(level);
    println!("  ASSESSMENT:     {assessment}");
    println!("                  Fnord.");
}

fn print_enlightened_extras(m: &PinealMetrics, _no_color: bool, _no_unicode: bool) {
    println!();
    println!("  --- expanded consciousness ---");
    println!(
        "  UNAME:          {} / {} / {}",
        m.os_name, m.os_version, m.kernel_version
    );
    let total_gb = bytes_to_gb(m.total_ram_bytes);
    let used_gb = bytes_to_gb(m.used_ram_bytes);
    let free_gb = (total_gb - used_gb).max(0.0);
    println!("  RAM BREAKDOWN:  total={total_gb:.2}GB used={used_gb:.2}GB free={free_gb:.2}GB");
    if !m.per_cpu_usage.is_empty() {
        let parts: Vec<String> = m
            .per_cpu_usage
            .iter()
            .enumerate()
            .map(|(i, u)| format!("cpu{i}={u:.1}%"))
            .collect();
        println!("  PER-CPU:        {}", parts.join(" "));
    }
    let today = Local::now().date_naive();
    let prophecy = pick_prophecy(&m.host, &today.to_string());
    println!("  PROPHECY:       {prophecy}");
    let disc = to_discordian(today);
    println!("  DISCORDIAN:     {disc}");
}

fn print_raw(m: &PinealMetrics, level: i32) {
    println!();
    println!("  --- raw system values ---");
    println!("  host:              {}", m.host);
    println!("  user:              {}", m.user);
    println!("  os_name:           {}", m.os_name);
    println!("  os_version:        {}", m.os_version);
    println!("  kernel_version:    {}", m.kernel_version);
    println!("  cpu_model:         {}", m.cpu_model);
    println!("  core_count:        {}", m.core_count);
    println!("  total_ram_bytes:   {}", m.total_ram_bytes);
    println!("  used_ram_bytes:    {}", m.used_ram_bytes);
    println!("  ram_used_pct:      {:.2}", m.ram_used_pct());
    println!("  uptime_seconds:    {}", m.uptime_seconds);
    println!("  load_1m:           {:.3}", m.load_1m);
    println!("  load_5m:           {:.3}", m.load_5m);
    println!("  load_15m:          {:.3}", m.load_15m);
    println!("  consciousness:     {level}");
}

fn print_json(m: &PinealMetrics, level: i32, label: &str) -> Result<(), FnordError> {
    let today = Local::now().date_naive();
    let prophecy = pick_prophecy(&m.host, &today.to_string());
    let obj = json!({
        "host": m.host,
        "user": m.user,
        "os": {
            "name": m.os_name,
            "version": m.os_version,
            "kernel": m.kernel_version,
        },
        "cpu": {
            "model": m.cpu_model,
            "core_count": m.core_count,
            "per_cpu_usage": m.per_cpu_usage,
        },
        "memory": {
            "total_bytes": m.total_ram_bytes,
            "used_bytes": m.used_ram_bytes,
            "used_pct": m.ram_used_pct(),
        },
        "uptime_seconds": m.uptime_seconds,
        "load": {
            "one": m.load_1m,
            "five": m.load_5m,
            "fifteen": m.load_15m,
        },
        "consciousness": {
            "level": level,
            "label": label,
        },
        "prophecy": prophecy,
        "discordian_date": to_discordian(today).to_string(),
    });
    println!("{}", serde_json::to_string_pretty(&obj).unwrap());
    Ok(())
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let rem = seconds % 86_400;
    let hours = rem / 3_600;
    let rem = rem % 3_600;
    let minutes = rem / 60;
    if days > 0 {
        format!("{days} days, {hours} hours, {minutes} minutes")
    } else if hours > 0 {
        format!("{hours} hours, {minutes} minutes")
    } else {
        format!("{minutes} minutes")
    }
}

fn bytes_to_gb(bytes: u64) -> f64 {
    (bytes as f64) / (1024.0 * 1024.0 * 1024.0)
}

fn host_style(host: &str) -> String {
    // Deterministically assign a hostname a flavor text.
    let flavors = [
        "Her Incoherence of the Flaxen Cabbage",
        "High Priest of the Five Cores",
        "Keeper of the Sacred Swap",
        "Bureaucrat of the Inner Ring",
        "Dispenser of Fnords",
        "Curator of Orderly Chaos",
    ];
    let h = hash_str(host);
    flavors[(h as usize) % flavors.len()].to_string()
}

fn core_flavor_text(cores: usize) -> String {
    let sacred = 5.0_f64;
    let ratio = (cores as f64) / sacred;
    if cores == 5 {
        "exactly the sacred five".to_string()
    } else if cores < 5 {
        format!("{ratio:.2}x the sacred five — below threshold")
    } else {
        format!("{ratio:.2}x the sacred five")
    }
}

fn chao_balance(load: f64) -> &'static str {
    if load < 0.5 {
        "The Chao sleeps"
    } else if load < 1.5 {
        "The Chao is in balance"
    } else if load < 3.0 {
        "The Chao stirs"
    } else {
        "The Chao dances wildly"
    }
}

fn assessment_line(level: i32) -> &'static str {
    match level {
        0..=20 => "Your machine requires immediate disorder.",
        21..=40 => "Your machine is mildly discordant. Acceptable.",
        41..=60 => "Your machine is becoming aware. Continue.",
        61..=80 => "Your machine is adequately enlightened.",
        81..=99 => "Your machine has transcended most bureaucracy.",
        100 => "Your machine has achieved Pope-level consciousness. Kallisti!",
        _ => "Consciousness beyond measurement.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_metrics() -> PinealMetrics {
        PinealMetrics {
            host: "testhost".to_string(),
            user: "tester".to_string(),
            os_name: "TestOS".to_string(),
            os_version: "1.0".to_string(),
            kernel_version: "1.0-test".to_string(),
            cpu_model: "Test CPU".to_string(),
            core_count: 4,
            per_cpu_usage: vec![],
            total_ram_bytes: 16 * 1024 * 1024 * 1024,
            used_ram_bytes: 4 * 1024 * 1024 * 1024,
            uptime_seconds: 0,
            load_1m: 0.5,
            load_5m: 0.5,
            load_15m: 0.5,
        }
    }

    #[test]
    fn consciousness_level_is_clamped_to_valid_range() {
        let mut m = base_metrics();
        m.load_1m = 100.0;
        m.core_count = 0;
        m.uptime_seconds = 0;
        m.used_ram_bytes = m.total_ram_bytes;
        let level = consciousness_level(&m);
        assert!((0..=100).contains(&level));

        // Maxed-out metrics
        m.load_1m = 0.1;
        m.core_count = 16;
        m.uptime_seconds = 4 * 86_400; // uptime_days = 4
        m.used_ram_bytes = 0;
        let level = consciousness_level(&m);
        assert!((0..=100).contains(&level));
    }

    #[test]
    fn consciousness_level_is_at_least_23_for_reasonable_system() {
        // The "reasonable" qualifier in the spec allows the load overload
        // penalty to bring the number below 23; we instead exercise a
        // sensible defaults-friendly system and assert level >= 23.
        let m = base_metrics();
        let level = consciousness_level(&m);
        assert!(level >= 23, "expected level >= 23, got {level}");
    }

    #[test]
    fn labels_cover_all_ranges_including_boundaries() {
        let cases: &[(i32, &str)] = &[
            (0, "Greyface"),
            (20, "Greyface"),
            (21, "Mildly"),
            (40, "Mildly"),
            (41, "Erisian"),
            (60, "Erisian"),
            (61, "Illuminated"),
            (80, "Illuminated"),
            (81, "Transcendent"),
            (99, "Transcendent"),
            (100, "Pope"),
        ];
        for (lvl, expected_fragment) in cases {
            let label = consciousness_label(*lvl);
            assert!(
                label.contains(expected_fragment),
                "level {lvl} -> label '{label}' did not contain '{expected_fragment}'"
            );
        }
    }

    #[test]
    fn prophecy_selection_is_deterministic() {
        let a = pick_prophecy("archbox", "2026-04-09");
        let b = pick_prophecy("archbox", "2026-04-09");
        assert_eq!(a, b);
        let c = pick_prophecy("otherhost", "2026-04-09");
        // Different host should *probably* yield a different prophecy,
        // but we only assert equal inputs → equal outputs.
        let _ = c;
    }

    #[test]
    fn prophecy_corpus_has_at_least_fifteen_entries() {
        assert!(PROPHECIES.len() >= 15);
    }

    #[test]
    fn verbosity_parse_accepts_known_values() {
        assert_eq!(Verbosity::parse("minimal").unwrap(), Verbosity::Minimal);
        assert_eq!(Verbosity::parse("normal").unwrap(), Verbosity::Normal);
        assert_eq!(
            Verbosity::parse("enlightened").unwrap(),
            Verbosity::Enlightened
        );
        assert!(Verbosity::parse("unknown").is_err());
    }

    #[test]
    fn format_uptime_handles_zero() {
        assert!(format_uptime(0).contains("0 minutes"));
    }

    #[test]
    fn format_uptime_days_hours_minutes() {
        // 5 days, 3 hours, 12 minutes = 5*86400 + 3*3600 + 12*60
        let s = format_uptime(5 * 86_400 + 3 * 3_600 + 12 * 60);
        assert!(s.contains("5 days"));
        assert!(s.contains("3 hours"));
        assert!(s.contains("12 minutes"));
    }
}
