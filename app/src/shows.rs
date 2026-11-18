//! Bridge for the mobile app: what to play. `/public/shows.json` on the viewer portal says whether
//! the station is live (OBS is streaming) with the YouTube live link, and lists the newest
//! YouTube uploads from the channel's public feed (no API key), so the app can show the live
//! stream or, when nothing is live, the latest show.

use std::sync::{OnceLock, RwLock};

use serde::Serialize;

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub url: String,
    pub published: String,
    pub thumbnail: String,
}

#[derive(Default)]
struct Feed {
    live: bool,
    videos: Vec<Video>,
    fetched: Option<std::time::Instant>,
    channel: String,
}

fn feed() -> &'static RwLock<Feed> {
    static F: OnceLock<RwLock<Feed>> = OnceLock::new();
    F.get_or_init(Default::default)
}

/// OBS streaming state (set whenever OBS health is polled).
pub fn set_live(live: bool) {
    if let Ok(mut f) = feed().write() {
        f.live = live;
    }
}

/// True when the channel's uploads should be fetched again (every 15 minutes, or the channel
/// changed).
pub fn due(channel: &str) -> bool {
    let channel = channel.trim();
    if channel.is_empty() {
        return false;
    }
    feed().read().map(|f| f.channel != channel || f.fetched.is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(15 * 60))).unwrap_or(false)
}

/// Fetch the channel's newest uploads (YouTube's public RSS feed).
pub async fn refresh(channel: String) {
    let channel = channel.trim().to_string();
    if let Ok(mut f) = feed().write() {
        f.fetched = Some(std::time::Instant::now());
        f.channel = channel.clone();
    }
    let url = format!("https://www.youtube.com/feeds/videos.xml?channel_id={channel}");
    match crate::net::get_bytes(&url).await {
        Ok(bytes) => {
            let videos = parse(&String::from_utf8_lossy(&bytes));
            if let Ok(mut f) = feed().write() {
                f.videos = videos;
            }
        }
        Err(e) => log::warn!("YouTube channel feed: {e}"),
    }
}

fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let a = s.find(open)? + open.len();
    let b = s[a..].find(close)? + a;
    Some(&s[a..b])
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'")
}

/// Entries of a YouTube channel RSS feed, newest first.
pub fn parse(xml: &str) -> Vec<Video> {
    xml.split("<entry>")
        .skip(1)
        .filter_map(|entry| {
            let id = between(entry, "<yt:videoId>", "</yt:videoId>")?.trim().to_string();
            Some(Video {
                url: format!("https://www.youtube.com/watch?v={id}"),
                thumbnail: format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"),
                title: unescape(between(entry, "<title>", "</title>").unwrap_or_default().trim()),
                published: between(entry, "<published>", "</published>").unwrap_or_default().trim().to_string(),
                id,
            })
        })
        .take(10)
        .collect()
}

/// The JSON for `/public/shows.json`.
pub fn json(live_url: &str, station: &str) -> String {
    let f = feed().read();
    let (live, videos, channel) = f.as_ref().map(|f| (f.live, f.videos.clone(), f.channel.clone())).unwrap_or_default();
    serde_json::json!({
        "station": station,
        "live": live && !live_url.trim().is_empty(),
        "live_url": live_url.trim(),
        "channel": channel,
        "latest": videos.first(),
        "videos": videos,
    })
    .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_a_channel_feed() {
        let xml = r#"<feed><title>Ch</title><entry><id>yt:video:abc123</id><yt:videoId>abc123</yt:videoId><title>Evening Weather &amp; Storms</title><published>2026-11-11T22:00:00+00:00</published></entry><entry><yt:videoId>def456</yt:videoId><title>Morning</title><published>2026-11-11T12:00:00+00:00</published></entry></feed>"#;
        let v = super::parse(xml);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].id, "abc123");
        assert_eq!(v[0].title, "Evening Weather & Storms");
        assert!(v[0].url.ends_with("v=abc123"));
    }
}
