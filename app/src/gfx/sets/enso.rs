//! ENSO status (El Niño / La Niña) from the NOAA Climate Prediction Center's monthly ENSO
//! diagnostic discussion: the alert status, the synopsis and the issue date, for the winter
//! outlook graphics. Read straight from CPC at most every 6 hours (the product is monthly).
use crate::gfx::bind::Bindings;
use std::sync::{Mutex, OnceLock};

const URL: &str = "https://www.cpc.ncep.noaa.gov/products/analysis_monitoring/enso_advisory/ensodisc.shtml";
const RECHECK: i64 = 6 * 3600;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Enso {
    /// "El Niño Advisory", "La Niña Watch", "Not Active"...
    pub status: String,
    /// The discussion's synopsis: one or two sentences.
    pub synopsis: String,
    /// "November 12, 2026".
    pub issued: String,
}

#[derive(Default)]
struct Store {
    checked: i64,
    current: Option<Enso>,
}

fn store() -> &'static Mutex<Store> {
    static S: OnceLock<Mutex<Store>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(Store::default()))
}

/// The page as plain text: tags removed, entities decoded, whitespace collapsed. CPC serves
/// Windows-1252 / Latin-1, so bytes map to characters one to one.
fn plain_text(bytes: &[u8]) -> String {
    let raw: String = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    };
    let mut out = String::with_capacity(raw.len());
    let mut in_tag = false;
    for c in raw.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&nbsp;", " ").replace("&ntilde;", "ñ").replace("&deg;", "°");
    let out = decode_numeric_entities(&out).replace("&amp;", "&");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "&#37;" -> "%", "&#241;" -> "ñ".
fn decode_numeric_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("&#") {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 2..];
        match tail.find(';').filter(|&j| j > 0 && j <= 6).and_then(|j| tail[..j].parse::<u32>().ok().and_then(char::from_u32).map(|c| (c, j))) {
            Some((c, j)) => {
                out.push(c);
                rest = &tail[j + 1..];
            }
            None => {
                out.push_str("&#");
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Text after `label` up to the first of `ends`.
fn between<'a>(text: &'a str, label: &str, ends: &[&str]) -> Option<&'a str> {
    let start = text.find(label)? + label.len();
    let rest = &text[start..];
    let end = ends.iter().filter_map(|e| rest.find(e)).min().unwrap_or(rest.len());
    Some(rest[..end].trim().trim_matches(|c: char| !c.is_alphanumeric() && c != ')' && c != '.' && c != '%').trim())
}

pub(super) fn parse(bytes: &[u8]) -> Option<Enso> {
    let text = plain_text(bytes);
    let status = between(&text, "ENSO Alert System Status:", &["Synopsis:"])?.to_owned();
    let synopsis_all = between(&text, "Synopsis:", &[" Compared to", " Over the last", " During "])?;
    // First two sentences at most.
    let mut synopsis = String::new();
    for sentence in synopsis_all.split_inclusive(". ").take(2) {
        synopsis.push_str(sentence);
    }
    let synopsis = synopsis.trim().to_owned();
    let issued = between(&text, "issued by CLIMATE PREDICTION CENTER/NCEP/NWS", &["ENSO Alert", " Synopsis"])
        .map(|s| s.to_owned())
        .and_then(|s| chrono::NaiveDate::parse_from_str(&s, "%d %B %Y").ok())
        .map(|d| d.format("%B %-d, %Y").to_string())
        .unwrap_or_default();
    (!status.is_empty() && status.len() < 60 && !synopsis.is_empty()).then_some(Enso { status, synopsis, issued })
}

pub(super) async fn refresh() {
    let now = chrono::Utc::now().timestamp();
    if store().lock().map(|s| s.current.is_some() && now - s.checked < RECHECK).unwrap_or(true) {
        return;
    }
    let found = super::report_feed::get_bytes(URL, std::time::Duration::from_secs(30)).await.and_then(|b| parse(&b));
    if let Ok(mut s) = store().lock() {
        s.checked = now;
        if found.is_some() {
            s.current = found;
        }
    }
}

pub(super) fn bind(b: &mut Bindings) {
    let current = store().lock().ok().and_then(|s| s.current.clone());
    match current {
        Some(e) => {
            b.set("enso.status", &e.status);
            b.set("enso.synopsis", &e.synopsis);
            b.set("enso.issued", &if e.issued.is_empty() { "NOAA Climate Prediction Center".into() } else { format!("CPC discussion, {}", e.issued) });
        }
        None => {
            b.set("enso.status", "Checking NOAA CPC");
            b.set("enso.synopsis", "The ENSO discussion hasn't loaded yet.");
            b.set("enso.issued", "NOAA Climate Prediction Center");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_status_synopsis_and_date() {
        let page = b"<html><b>issued by CLIMATE PREDICTION CENTER/NCEP/NWS</b> 12 November 2026 <p>ENSO Alert System Status: El Ni\xf1o Advisory</p><p>Synopsis: El Ni\xf1o continues to strengthen, with a strong El Ni\xf1o likely through January-March 2027 (a 79&#37; chance). Another sentence here. Third sentence.</p><p>Compared to last month, more text.</p></html>";
        let e = parse(page).expect("parsed");
        assert_eq!(e.status, "El Niño Advisory");
        assert!(e.synopsis.starts_with("El Niño continues to strengthen"), "{}", e.synopsis);
        assert!(!e.synopsis.contains("Third"), "{}", e.synopsis);
        assert!(e.synopsis.contains("79% chance"), "{}", e.synopsis);
        assert_eq!(e.issued, "November 12, 2026");
    }

    #[test]
    #[ignore = "network"]
    fn live_cpc_discussion_parses() {
        let bytes = crate::net::runtime().block_on(super::super::report_feed::get_bytes(URL, std::time::Duration::from_secs(30))).expect("CPC answered");
        let e = parse(&bytes).expect("parsed");
        eprintln!("{e:?}");
        assert!(e.status.contains("Advisory") || e.status.contains("Watch") || e.status.contains("Not Active") || e.status.contains("Warning"));
    }
}
