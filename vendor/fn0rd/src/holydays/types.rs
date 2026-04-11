use crate::date::types::Season;
use crate::error::FnordError;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HolydayKey {
    StTibs,
    SeasonDay { season: Season, day: u8 },
}

impl HolydayKey {
    pub fn parse(s: &str) -> Result<Self, FnordError> {
        if s.eq_ignore_ascii_case("st-tibs") || s.eq_ignore_ascii_case("st_tibs") {
            return Ok(HolydayKey::StTibs);
        }
        // Format: <season>-<day>
        let (season_str, day_str) = s
            .rsplit_once('-')
            .ok_or_else(|| FnordError::Parse(format!("invalid holyday key: '{s}'")))?;
        let day: u8 = day_str
            .parse()
            .map_err(|_| FnordError::Parse(format!("invalid holyday day: '{day_str}'")))?;
        let season = match season_str.to_lowercase().as_str() {
            "chaos" => Season::Chaos,
            "discord" => Season::Discord,
            "confusion" => Season::Confusion,
            "bureaucracy" => Season::Bureaucracy,
            "aftermath" => Season::Aftermath,
            other => return Err(FnordError::Parse(format!("unknown season: '{other}'"))),
        };
        if !(1..=73).contains(&day) {
            return Err(FnordError::Parse(format!(
                "holyday day out of range: {day}"
            )));
        }
        Ok(HolydayKey::SeasonDay { season, day })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum HolydaySource {
    Default,
    Cabal,
    Personal,
}

#[derive(Debug, Clone)]
pub struct Holyday {
    pub key: HolydayKey,
    pub name: String,
    pub description: Option<String>,
    pub greeting: Option<String>,
    #[allow(dead_code)]
    pub apostle: Option<String>,
    pub recurring: bool,
    pub year: Option<i32>,
    #[allow(dead_code)]
    pub source: HolydaySource,
}

/// Personal holyday file format (TOML)
#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct PersonalHolydayFile {
    #[serde(default)]
    pub holyday: Vec<PersonalHolydayEntry>,
}

#[allow(dead_code)]
#[derive(Debug, serde::Deserialize)]
pub struct PersonalHolydayEntry {
    pub name: String,
    pub date: String,
    pub description: Option<String>,
    pub greeting: Option<String>,
    #[serde(default = "default_recurring")]
    pub recurring: bool,
    pub year: Option<i32>,
}

#[allow(dead_code)]
fn default_recurring() -> bool {
    true
}

impl PersonalHolydayEntry {
    #[allow(dead_code)]
    pub fn into_holyday(self, source: HolydaySource) -> Result<Holyday, FnordError> {
        let key = HolydayKey::parse(&self.date)?;
        Ok(Holyday {
            key,
            name: self.name,
            description: self.description,
            greeting: self.greeting,
            apostle: None,
            recurring: self.recurring,
            year: self.year,
            source,
        })
    }
}
