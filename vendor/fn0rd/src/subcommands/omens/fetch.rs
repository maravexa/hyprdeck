//! Live weather fetch from wttr.in. Returns an error on any network or
//! parse failure so the caller can fall through to generative mode.

use std::time::Duration;

use serde_json::Value;

use crate::error::FnordError;
use crate::subcommands::omens::WeatherReading;

const WTTR_URL_FMT: &str = "https://wttr.in/{}?format=j1";

/// Fetch current weather from wttr.in for `location` and extract the
/// fields we care about. A 5-second timeout is enforced on the request.
pub fn fetch_weather(location: &str) -> Result<WeatherReading, FnordError> {
    if location.trim().is_empty() {
        return Err(FnordError::Parse("no location provided".to_string()));
    }

    let url = WTTR_URL_FMT.replacen("{}", &encode_location(location), 1);

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(5))
        .user_agent("fnord (https://github.com/maravexa/fnord)")
        .build()
        .map_err(|e| FnordError::Parse(format!("reqwest client error: {e}")))?;

    let resp = client
        .get(&url)
        .send()
        .map_err(|e| FnordError::Parse(format!("wttr.in fetch failed: {e}")))?;

    if !resp.status().is_success() {
        return Err(FnordError::Parse(format!(
            "wttr.in returned status {}",
            resp.status()
        )));
    }

    let json: Value = resp
        .json()
        .map_err(|e| FnordError::Parse(format!("wttr.in parse error: {e}")))?;

    parse_j1(&json, location)
}

/// Parse a `j1` response into a WeatherReading. Exposed for tests.
pub fn parse_j1(json: &Value, location: &str) -> Result<WeatherReading, FnordError> {
    let current = json
        .get("current_condition")
        .and_then(|c| c.as_array())
        .and_then(|a| a.first())
        .ok_or_else(|| FnordError::Parse("wttr.in: missing current_condition".to_string()))?;

    let temp_c = current
        .get("temp_C")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let temp_f = current
        .get("temp_F")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(32.0 + temp_c * 9.0 / 5.0);
    let humidity = current
        .get("humidity")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let wind_kmph = current
        .get("windspeedKmph")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let wind_dir = current
        .get("winddir16Point")
        .and_then(|v| v.as_str())
        .unwrap_or("N")
        .to_string();
    let desc = current
        .get("weatherDesc")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|d| d.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();
    let cloudcover = current
        .get("cloudcover")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    let precip_mm = current
        .get("precipMM")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(WeatherReading {
        location: location.to_string(),
        source: "wttr.in".to_string(),
        temp_c,
        temp_f,
        humidity,
        wind_kmph,
        wind_dir,
        description: desc,
        cloudcover,
        precip_mm,
    })
}

/// Light URL encoding — wttr.in accepts raw locations with spaces
/// URL-encoded, plus most punctuation. We encode spaces to `+` and pass
/// through alphanumerics, `-`, `_`, `.`, `,`.
fn encode_location(loc: &str) -> String {
    let mut out = String::with_capacity(loc.len());
    for c in loc.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ',') {
            out.push(c);
        } else if c == ' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", c as u32 & 0xFF));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_location_errors() {
        // This test intentionally uses an obviously-bogus location. We
        // can't guarantee offline, so we use a location reqwest will
        // reject at the URL level.
        let err = fetch_weather("").unwrap_err();
        assert!(format!("{err}").contains("no location"));
    }

    #[test]
    fn parse_j1_extracts_fields() {
        let sample = serde_json::json!({
            "current_condition": [{
                "temp_C": "18",
                "temp_F": "64",
                "humidity": "78",
                "windspeedKmph": "25",
                "winddir16Point": "NW",
                "weatherDesc": [{"value": "Rain"}],
                "cloudcover": "80",
                "precipMM": "4.2"
            }]
        });
        let w = parse_j1(&sample, "Portland").unwrap();
        assert_eq!(w.description, "Rain");
        assert_eq!(w.wind_dir, "NW");
        assert!((w.temp_c - 18.0).abs() < 1e-9);
        assert!((w.precip_mm - 4.2).abs() < 1e-9);
    }

    #[test]
    fn parse_j1_missing_fields_is_error() {
        let sample = serde_json::json!({"foo": "bar"});
        assert!(parse_j1(&sample, "x").is_err());
    }

    #[test]
    fn encode_location_handles_spaces() {
        assert_eq!(encode_location("Portland, OR"), "Portland,+OR");
    }
}
