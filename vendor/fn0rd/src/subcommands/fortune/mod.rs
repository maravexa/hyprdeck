pub mod corpus;

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde_json::json;

use crate::cli::FortuneArgs;
use crate::config::Config;
use crate::date::convert::to_discordian;
use crate::date::types::{DiscordianDate, Season};
use crate::error::FnordError;
use crate::holydays::defaults::builtin_holydays;
use crate::holydays::registry::HolydayRegistry;
use crate::subcommands::util::hash_str;

use corpus::{BUILTIN_FORTUNES, OFFENSIVE_FORTUNES};

/// A parsed fortune with optional tags and a source identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fortune {
    pub text: String,
    pub tags: Vec<Tag>,
    pub source: String,
}

/// A fortune tag. Only `season:` and `holyday:` are currently recognised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag {
    Season(String),
    Holyday(String),
    Other(String, String),
}

impl Tag {
    pub fn key(&self) -> &str {
        match self {
            Tag::Season(_) => "season",
            Tag::Holyday(_) => "holyday",
            Tag::Other(k, _) => k,
        }
    }
    pub fn value(&self) -> &str {
        match self {
            Tag::Season(v) | Tag::Holyday(v) | Tag::Other(_, v) => v,
        }
    }
    pub fn to_display(&self) -> String {
        format!("{}:{}", self.key(), self.value())
    }
}

pub fn run(
    args: &FortuneArgs,
    config: &Config,
    json: bool,
    _no_color: bool,
    _no_unicode: bool,
) -> Result<(), FnordError> {
    let fortunes = collect_fortunes(config, args.offensive)?;
    if fortunes.is_empty() {
        return Err(FnordError::Parse(
            "no fortunes available (builtin disabled and no fortune files)".to_string(),
        ));
    }

    // Apply --tag filter if present.
    let filtered: Vec<&Fortune> = if let Some(tag_filter) = &args.tag {
        let (key, value) = split_tag_filter(tag_filter);
        fortunes
            .iter()
            .filter(|f| {
                f.tags.iter().any(|t| {
                    let key_match = key.map(|k| t.key() == k).unwrap_or(true);
                    key_match && t.value().eq_ignore_ascii_case(value)
                })
            })
            .collect()
    } else {
        fortunes.iter().collect()
    };

    if filtered.is_empty() {
        return Err(FnordError::Parse(format!(
            "no fortunes match tag '{}'",
            args.tag.as_deref().unwrap_or("")
        )));
    }

    // Determine current season and holyday for weighting.
    let today_naive = Local::now().date_naive();
    let today_disc = to_discordian(today_naive);
    let current_season = current_season(&today_disc);
    let current_holyday = current_holyday_name(&today_disc);

    let weight_by_season = config.fortune.weight_by_season && !args.random;
    let weight_by_holyday = config.fortune.weight_by_holyday && !args.random;

    let count = args.count.max(1);
    let base_seed = Local::now().timestamp_nanos_opt().unwrap_or(0) as u64;
    let mut picks: Vec<&Fortune> = Vec::with_capacity(count);
    for i in 0..count {
        let seed = hash_str(&format!("{base_seed}:{i}"));
        let f = if args.random {
            pick_uniform(&filtered, seed)
        } else {
            pick_weighted(
                &filtered,
                seed,
                weight_by_season,
                weight_by_holyday,
                current_season,
                current_holyday.as_deref(),
            )
        };
        picks.push(f);
    }

    if json {
        let arr: Vec<serde_json::Value> = picks
            .iter()
            .map(|f| {
                json!({
                    "text": f.text,
                    "tags": f.tags.iter().map(|t| t.to_display()).collect::<Vec<_>>(),
                    "source": f.source,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap());
        return Ok(());
    }

    // Plain output: separate multiple fortunes with "%".
    for (i, f) in picks.iter().enumerate() {
        if i > 0 {
            println!("%");
        }
        println!("{}", f.text);
    }

    Ok(())
}

fn split_tag_filter(s: &str) -> (Option<&str>, &str) {
    match s.split_once(':') {
        Some((k, v)) => (Some(k), v),
        None => (None, s),
    }
}

fn current_season(d: &DiscordianDate) -> Option<Season> {
    match d {
        DiscordianDate::SeasonDay { season, .. } => Some(*season),
        DiscordianDate::StTibsDay { .. } => None,
    }
}

fn current_holyday_name(d: &DiscordianDate) -> Option<String> {
    let registry = HolydayRegistry::build(builtin_holydays(), vec![], vec![]);
    let holydays = registry.lookup(d);
    holydays.first().map(|h| normalize_holyday(&h.name))
}

fn normalize_holyday(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Build the fortune pool from builtin corpus + configured files.
pub fn collect_fortunes(
    config: &Config,
    include_offensive: bool,
) -> Result<Vec<Fortune>, FnordError> {
    let mut out: Vec<Fortune> = Vec::new();
    if config.fortune.builtin {
        for &t in BUILTIN_FORTUNES {
            out.push(Fortune {
                text: t.to_string(),
                tags: vec![],
                source: "builtin".to_string(),
            });
        }
    }

    if include_offensive && config.fortune.offensive {
        for &t in OFFENSIVE_FORTUNES {
            out.push(Fortune {
                text: t.to_string(),
                tags: vec![],
                source: "builtin:offensive".to_string(),
            });
        }
    }

    for file in &config.fortune.files {
        let path = PathBuf::from(file);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        out.extend(parse_fortune_file(&content, &name));
    }
    Ok(out)
}

/// Parse a Unix-style fortune file. Fortunes are separated by a line
/// containing only `%`. Lines starting with `% ` (before a fortune's body)
/// are parsed as tags.
pub fn parse_fortune_file(content: &str, source: &str) -> Vec<Fortune> {
    let mut out: Vec<Fortune> = Vec::new();
    let mut tags: Vec<Tag> = Vec::new();
    let mut body: Vec<&str> = Vec::new();
    let mut in_body = false;

    for line in content.lines() {
        let trimmed = line.trim_end();
        if trimmed == "%" {
            if !body.is_empty() {
                out.push(Fortune {
                    text: body.join("\n").trim().to_string(),
                    tags: std::mem::take(&mut tags),
                    source: source.to_string(),
                });
                body.clear();
            } else {
                tags.clear();
            }
            in_body = false;
            continue;
        }
        if !in_body && trimmed.starts_with("% ") {
            if let Some(t) = parse_tag(&trimmed[2..]) {
                tags.push(t);
            }
            continue;
        }
        in_body = true;
        body.push(line);
    }
    if !body.is_empty() {
        out.push(Fortune {
            text: body.join("\n").trim().to_string(),
            tags,
            source: source.to_string(),
        });
    }
    // Drop any completely empty fortunes.
    out.retain(|f| !f.text.is_empty());
    out
}

fn parse_tag(s: &str) -> Option<Tag> {
    let s = s.trim();
    let (k, v) = s.split_once(':')?;
    let k = k.trim().to_lowercase();
    let v = v.trim().to_string();
    if v.is_empty() {
        return None;
    }
    match k.as_str() {
        "season" => Some(Tag::Season(v.to_lowercase())),
        "holyday" => Some(Tag::Holyday(v.to_lowercase().replace(' ', ""))),
        other => Some(Tag::Other(other.to_string(), v)),
    }
}

pub fn compute_weight(
    f: &Fortune,
    weight_by_season: bool,
    weight_by_holyday: bool,
    current_season: Option<Season>,
    current_holyday: Option<&str>,
) -> u64 {
    let mut w: u64 = 1;
    if weight_by_holyday {
        if let Some(h) = current_holyday {
            if f.tags
                .iter()
                .any(|t| matches!(t, Tag::Holyday(v) if v.eq_ignore_ascii_case(h)))
            {
                w = w.saturating_mul(5);
            }
        }
    }
    if weight_by_season {
        if let Some(season) = current_season {
            let name = season.to_string().to_lowercase();
            if f.tags
                .iter()
                .any(|t| matches!(t, Tag::Season(v) if v.eq_ignore_ascii_case(&name)))
            {
                w = w.saturating_mul(3);
            }
        }
    }
    w
}

fn pick_uniform<'a>(fortunes: &'a [&'a Fortune], seed: u64) -> &'a Fortune {
    let idx = (seed as usize) % fortunes.len();
    fortunes[idx]
}

pub fn pick_weighted<'a>(
    fortunes: &'a [&'a Fortune],
    seed: u64,
    weight_by_season: bool,
    weight_by_holyday: bool,
    current_season: Option<Season>,
    current_holyday: Option<&str>,
) -> &'a Fortune {
    let weights: Vec<u64> = fortunes
        .iter()
        .map(|f| {
            compute_weight(
                f,
                weight_by_season,
                weight_by_holyday,
                current_season,
                current_holyday,
            )
        })
        .collect();
    let total: u64 = weights.iter().sum();
    if total == 0 {
        return pick_uniform(fortunes, seed);
    }
    let mut target = seed % total;
    for (i, w) in weights.iter().enumerate() {
        if target < *w {
            return fortunes[i];
        }
        target -= *w;
    }
    fortunes[fortunes.len() - 1]
}

#[allow(dead_code)]
fn is_file<P: AsRef<Path>>(p: P) -> bool {
    fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{Config, FortuneConfig};

    #[test]
    fn builtin_corpus_loads() {
        let cfg = Config::default();
        let pool = collect_fortunes(&cfg, false).unwrap();
        assert!(
            pool.len() >= 30,
            "expected at least 30 fortunes, got {}",
            pool.len()
        );
        assert!(pool.iter().any(|f| f.text == "Fnord."));
        assert!(pool
            .iter()
            .any(|f| f.text == "This fortune intentionally left fnord."));
    }

    #[test]
    fn builtin_disabled_excludes_builtin() {
        let cfg = Config {
            fortune: FortuneConfig {
                builtin: false,
                ..FortuneConfig::default()
            },
            ..Config::default()
        };
        let pool = collect_fortunes(&cfg, false).unwrap();
        assert!(pool.is_empty());
    }

    #[test]
    fn parse_fortune_file_splits_on_percent() {
        let content = "\
fortune one
%
fortune two
has multiple lines
%
fortune three
";
        let out = parse_fortune_file(content, "test");
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].text, "fortune one");
        assert_eq!(out[1].text, "fortune two\nhas multiple lines");
        assert_eq!(out[2].text, "fortune three");
    }

    #[test]
    fn parse_fortune_file_parses_tags() {
        let content = "\
% season:chaos
% holyday:mungday
This fortune has tags.
%
% season:aftermath
Another tagged fortune.
%
An untagged fortune.
";
        let out = parse_fortune_file(content, "test");
        assert_eq!(out.len(), 3);
        assert_eq!(out[0].tags.len(), 2);
        assert!(matches!(&out[0].tags[0], Tag::Season(v) if v == "chaos"));
        assert!(matches!(&out[0].tags[1], Tag::Holyday(v) if v == "mungday"));
        assert_eq!(out[1].tags.len(), 1);
        assert!(matches!(&out[1].tags[0], Tag::Season(v) if v == "aftermath"));
        assert!(out[2].tags.is_empty());
    }

    #[test]
    fn weighted_selection_favors_tagged_fortunes() {
        // Build a pool with 10 untagged + 1 season-tagged fortune.
        let mut pool: Vec<Fortune> = (0..10)
            .map(|i| Fortune {
                text: format!("plain {i}"),
                tags: vec![],
                source: "test".to_string(),
            })
            .collect();
        pool.push(Fortune {
            text: "tagged chaos".to_string(),
            tags: vec![Tag::Season("chaos".to_string())],
            source: "test".to_string(),
        });
        let refs: Vec<&Fortune> = pool.iter().collect();

        let mut tagged_hits = 0;
        let iterations = 1000;
        for i in 0..iterations {
            let seed = hash_str(&format!("seed:{i}"));
            let picked = pick_weighted(&refs, seed, true, false, Some(Season::Chaos), None);
            if picked.text == "tagged chaos" {
                tagged_hits += 1;
            }
        }
        // Base rate is 1/11 ≈ 90 per 1000. With 3x weight for tagged,
        // expected is ~3/(10+3) ≈ 230 per 1000, well over 2 * base_rate.
        let base_rate = iterations / 11; // ~ 90
        assert!(
            tagged_hits > 2 * base_rate,
            "tagged_hits={tagged_hits}, base_rate={base_rate}"
        );
    }

    #[test]
    fn compute_weight_holyday_and_season_stack() {
        let f = Fortune {
            text: "x".to_string(),
            tags: vec![
                Tag::Holyday("mungday".to_string()),
                Tag::Season("chaos".to_string()),
            ],
            source: "t".to_string(),
        };
        let w = compute_weight(&f, true, true, Some(Season::Chaos), Some("mungday"));
        assert_eq!(w, 15);
    }
}
