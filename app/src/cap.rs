//! Common Alerting Protocol (CAP 1.2) XML: parse, validate, pick the language, verify the sender,
//! and turn messages into app alerts (Alert / Update / Cancel). Used for CAP files and CAP feeds
//! (state emergency management, IPAWS relays) beside the NWS API.

use anyhow::{anyhow, Result};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CapMessage {
    pub identifier: String,
    pub sender: String,
    pub sent: String,
    pub status: String,
    pub msg_type: String,
    pub scope: String,
    /// Identifiers this message updates or cancels.
    pub references: Vec<String>,
    pub infos: Vec<CapInfo>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CapInfo {
    pub language: String,
    pub event: String,
    pub urgency: String,
    pub severity: String,
    pub certainty: String,
    pub headline: String,
    pub description: String,
    pub instruction: String,
    pub expires: String,
    pub sender_name: String,
    pub area_desc: String,
    /// "lat,lon lat,lon ..." polygons.
    pub polygons: Vec<String>,
}

/// Text of the first <tag>…</tag> inside `xml` (namespace prefixes ignored).
fn tag(xml: &str, name: &str) -> Option<String> {
    let mut rest = xml;
    loop {
        let start = rest.find('<')?;
        let after = &rest[start + 1..];
        let end = after.find('>')?;
        let head = &after[..end];
        let local = head.split_whitespace().next().unwrap_or("").rsplit(':').next().unwrap_or("");
        if local == name && !head.starts_with('/') {
            let body = &after[end + 1..];
            // Find the matching close tag (with or without a prefix).
            let close = body.find(&format!(":{name}>")).map(|i| body[..i].rfind("</").unwrap_or(i)).or_else(|| body.find(&format!("</{name}>")))?;
            return Some(unescape(body[..close].trim()));
        }
        rest = &after[end + 1..];
    }
}

/// Every <tag> block's inner XML.
fn blocks(xml: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&format!("<{name}")).or_else(|| rest.find(&format!(":{name}>")).and_then(|j| rest[..j].rfind('<'))) {
        let after = &rest[i..];
        let Some(open_end) = after.find('>') else { break };
        let body = &after[open_end + 1..];
        let close = body.find(&format!("</{name}>")).or_else(|| body.find(&format!(":{name}>")).and_then(|j| body[..j].rfind("</")));
        let Some(close) = close else { break };
        out.push(body[..close].to_string());
        rest = &body[close + 2..];
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

pub fn parse(xml: &str) -> Result<CapMessage> {
    let alert = blocks(xml, "alert").into_iter().next().unwrap_or_else(|| xml.to_string());
    let msg = CapMessage {
        identifier: tag(&alert, "identifier").unwrap_or_default(),
        sender: tag(&alert, "sender").unwrap_or_default(),
        sent: tag(&alert, "sent").unwrap_or_default(),
        status: tag(&alert, "status").unwrap_or_default(),
        msg_type: tag(&alert, "msgType").unwrap_or_default(),
        scope: tag(&alert, "scope").unwrap_or_default(),
        references: tag(&alert, "references")
            .map(|r| r.split_whitespace().filter_map(|t| t.split(',').nth(1).map(String::from)).collect())
            .unwrap_or_default(),
        infos: blocks(&alert, "info")
            .into_iter()
            .map(|info| {
                let areas = blocks(&info, "area");
                CapInfo {
                    language: tag(&info, "language").unwrap_or_else(|| "en-US".into()),
                    event: tag(&info, "event").unwrap_or_default(),
                    urgency: tag(&info, "urgency").unwrap_or_default(),
                    severity: tag(&info, "severity").unwrap_or_default(),
                    certainty: tag(&info, "certainty").unwrap_or_default(),
                    headline: tag(&info, "headline").unwrap_or_default(),
                    description: tag(&info, "description").unwrap_or_default(),
                    instruction: tag(&info, "instruction").unwrap_or_default(),
                    expires: tag(&info, "expires").unwrap_or_default(),
                    sender_name: tag(&info, "senderName").unwrap_or_default(),
                    area_desc: areas.iter().filter_map(|a| tag(a, "areaDesc")).collect::<Vec<_>>().join("; "),
                    polygons: areas.iter().flat_map(|a| blocks(a, "polygon")).map(|p| p.trim().to_string()).collect(),
                }
            })
            .collect(),
    };
    if msg.identifier.is_empty() && msg.infos.is_empty() {
        return Err(anyhow!("not a CAP message"));
    }
    Ok(msg)
}

/// Problems with a message against the CAP 1.2 required fields and value lists.
pub fn validate(m: &CapMessage) -> Vec<String> {
    let mut out = Vec::new();
    for (name, v) in [("identifier", &m.identifier), ("sender", &m.sender), ("sent", &m.sent), ("status", &m.status), ("msgType", &m.msg_type), ("scope", &m.scope)] {
        if v.trim().is_empty() {
            out.push(format!("missing <{name}>"));
        }
    }
    if !["Actual", "Exercise", "System", "Test", "Draft"].contains(&m.status.as_str()) {
        out.push(format!("unknown status \"{}\"", m.status));
    }
    if !["Alert", "Update", "Cancel", "Ack", "Error"].contains(&m.msg_type.as_str()) {
        out.push(format!("unknown msgType \"{}\"", m.msg_type));
    }
    if chrono::DateTime::parse_from_rfc3339(&m.sent).is_err() {
        out.push("sent is not a valid date-time".into());
    }
    if matches!(m.msg_type.as_str(), "Update" | "Cancel") && m.references.is_empty() {
        out.push(format!("{} without <references>", m.msg_type));
    }
    if m.infos.is_empty() && m.msg_type != "Cancel" {
        out.push("no <info> block".into());
    }
    for i in &m.infos {
        for (name, v) in [("event", &i.event), ("urgency", &i.urgency), ("severity", &i.severity), ("certainty", &i.certainty)] {
            if v.trim().is_empty() {
                out.push(format!("info missing <{name}>"));
            }
        }
        for p in &i.polygons {
            let pts: Vec<&str> = p.split_whitespace().collect();
            if pts.len() < 4 || pts.first() != pts.last() {
                out.push("polygon is not closed or has fewer than 4 points".into());
            }
        }
    }
    out
}

/// Whether a sender is a known official source (NWS, IPAWS, FEMA, or one in `extra`).
pub fn trusted_sender(sender: &str, extra: &[String]) -> bool {
    let s = sender.to_lowercase();
    s.contains("weather.gov") || s.contains("noaa.gov") || s.contains("ipaws") || s.contains("fema.gov") || extra.iter().any(|e| !e.trim().is_empty() && s.contains(&e.trim().to_lowercase()))
}

/// The info block in the preferred language ("en", "es"), else the first.
pub fn pick_info<'a>(m: &'a CapMessage, language: &str) -> Option<&'a CapInfo> {
    m.infos.iter().find(|i| i.language.to_lowercase().starts_with(&language.to_lowercase())).or_else(|| m.infos.first())
}

/// Convert to an app alert.
pub fn to_alert(m: &CapMessage, language: &str) -> Option<crate::data::Alert> {
    let info = pick_info(m, language)?;
    let polygons = info
        .polygons
        .iter()
        .map(|p| {
            p.split_whitespace()
                .filter_map(|pt| {
                    let (la, lo) = pt.split_once(',')?;
                    Some((lo.trim().parse().ok()?, la.trim().parse().ok()?))
                })
                .collect::<Vec<(f64, f64)>>()
        })
        .filter(|r| r.len() >= 3)
        .collect();
    Some(crate::data::Alert {
        id: format!("CAP-{}", m.identifier),
        event: info.event.clone(),
        headline: info.headline.clone(),
        severity: info.severity.clone(),
        urgency: info.urgency.clone(),
        area_desc: info.area_desc.clone(),
        sender: if info.sender_name.is_empty() { m.sender.clone() } else { info.sender_name.clone() },
        sent: Some(m.sent.clone()),
        expires: (!info.expires.is_empty()).then(|| info.expires.clone()),
        description: info.description.clone(),
        instruction: info.instruction.clone(),
        polygons,
        motion_text: None,
        message_type: m.msg_type.clone(),
        fips: Vec::new(),
        county_based: false,
    })
}

/// Messages in a CAP Atom/RSS feed: inline <alert> elements, or links to CAP files fetched one by one.
pub async fn fetch_feed(url: String) -> Result<Vec<CapMessage>> {
    let text = crate::net::get_logged(&url, std::time::Duration::from_secs(20)).await?.text().await?;
    let mut out: Vec<CapMessage> = blocks(&text, "alert").into_iter().filter_map(|a| parse(&format!("<alert>{a}</alert>")).ok()).collect();
    if out.is_empty() {
        // Atom entries that link to CAP documents.
        let links: Vec<String> = text
            .split("href=\"")
            .skip(1)
            .filter_map(|s| s.split('"').next().map(|l| l.replace("&amp;", "&")))
            .filter(|l| l.ends_with(".xml") || l.contains("cap") || l.contains("CAP"))
            .take(25)
            .collect();
        for l in links {
            if let Ok(resp) = crate::net::get_logged(&l, std::time::Duration::from_secs(15)).await {
                if let Ok(body) = resp.text().await {
                    if let Ok(m) = parse(&body) {
                        out.push(m);
                    }
                }
            }
        }
    }
    Ok(out)
}
