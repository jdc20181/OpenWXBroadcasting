//! OBS Studio control over obs-websocket v5 (built into OBS 28+): switch the program scene when
//! severe weather starts or ends. Settings > Broadcast control holds the address, password and
//! scene names.

use anyhow::{anyhow, Context as _, Result};
use base64::Engine as _;
use futures_util::{SinkExt as _, StreamExt as _};
use sha2::{Digest, Sha256};
use tokio_tungstenite::tungstenite::Message;

fn b64_sha256(data: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha256::digest(data.as_bytes()))
}

/// Connect, authenticate, send one request, return its response data.
async fn request(url: &str, password: &str, request_type: &str, data: serde_json::Value) -> Result<serde_json::Value> {
    let (mut ws, _) = tokio::time::timeout(std::time::Duration::from_secs(5), tokio_tungstenite::connect_async(url))
        .await
        .map_err(|_| anyhow!("OBS did not answer at {url} - is OBS open with WebSocket server enabled?"))?
        .with_context(|| format!("could not connect to OBS at {url}"))?;

    let next_json = |msg: Message| -> Option<serde_json::Value> {
        match msg {
            Message::Text(text) => serde_json::from_str(text.as_str()).ok(),
            _ => None,
        }
    };

    // Hello (op 0) → Identify (op 1), with authentication when OBS asks for it.
    let hello = loop {
        let msg = ws.next().await.ok_or_else(|| anyhow!("OBS closed the connection"))??;
        if let Some(v) = next_json(msg) {
            if v["op"] == 0 {
                break v;
            }
        }
    };
    let mut identify = serde_json::json!({ "op": 1, "d": { "rpcVersion": 1 } });
    if let Some(auth) = hello["d"].get("authentication") {
        let challenge = auth["challenge"].as_str().unwrap_or_default();
        let salt = auth["salt"].as_str().unwrap_or_default();
        if password.is_empty() {
            return Err(anyhow!("OBS needs a WebSocket password - add it in Settings"));
        }
        let secret = b64_sha256(&format!("{password}{salt}"));
        identify["d"]["authentication"] = serde_json::Value::String(b64_sha256(&format!("{secret}{challenge}")));
    }
    ws.send(Message::Text(identify.to_string().into())).await?;
    loop {
        let msg = ws.next().await.ok_or_else(|| anyhow!("OBS rejected the login (wrong password?)"))??;
        if let Some(v) = next_json(msg) {
            if v["op"] == 2 {
                break;
            }
        }
    }

    let request = serde_json::json!({
        "op": 6,
        "d": { "requestType": request_type, "requestId": "cwx", "requestData": data }
    });
    ws.send(Message::Text(request.to_string().into())).await?;
    let response = loop {
        let msg = ws.next().await.ok_or_else(|| anyhow!("OBS closed the connection"))??;
        if let Some(v) = next_json(msg) {
            if v["op"] == 7 {
                break v;
            }
        }
    };
    let _ = ws.close(None).await;
    let status = &response["d"]["requestStatus"];
    if status["result"].as_bool() != Some(true) {
        return Err(anyhow!(
            "OBS: {}",
            status["comment"].as_str().unwrap_or("request failed")
        ));
    }
    Ok(response["d"]["responseData"].clone())
}

/// Listen to OBS audio meters for `seconds`; returns the loudest input peak in dBFS.
pub async fn meter_peak(url: &str, password: &str, seconds: u64) -> Result<Option<f32>> {
    let (mut ws, _) = tokio::time::timeout(std::time::Duration::from_secs(5), tokio_tungstenite::connect_async(url))
        .await
        .map_err(|_| anyhow!("OBS did not answer at {url}"))?
        .with_context(|| format!("could not connect to OBS at {url}"))?;
    let json = |msg: Message| -> Option<serde_json::Value> {
        match msg {
            Message::Text(text) => serde_json::from_str(text.as_str()).ok(),
            _ => None,
        }
    };
    let hello = loop {
        let msg = ws.next().await.ok_or_else(|| anyhow!("OBS closed the connection"))??;
        if let Some(v) = json(msg) {
            if v["op"] == 0 {
                break v;
            }
        }
    };
    // InputVolumeMeters is a high-volume event (bit 16) and must be asked for.
    let mut identify = serde_json::json!({ "op": 1, "d": { "rpcVersion": 1, "eventSubscriptions": 1u64 << 16 } });
    if let Some(auth) = hello["d"].get("authentication") {
        let secret = b64_sha256(&format!("{password}{}", auth["salt"].as_str().unwrap_or_default()));
        identify["d"]["authentication"] = serde_json::Value::String(b64_sha256(&format!("{secret}{}", auth["challenge"].as_str().unwrap_or_default())));
    }
    ws.send(Message::Text(identify.to_string().into())).await?;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(seconds.max(1));
    let mut peak: Option<f32> = None;
    while let Ok(Some(Ok(msg))) = tokio::time::timeout_at(deadline, ws.next()).await {
        let Some(v) = json(msg) else { continue };
        if v["op"] == 5 && v["d"]["eventType"] == "InputVolumeMeters" {
            for input in v["d"]["eventData"]["inputs"].as_array().into_iter().flatten() {
                for channel in input["inputLevelsMul"].as_array().into_iter().flatten() {
                    if let Some(p) = channel.get(1).and_then(|x| x.as_f64()) {
                        let db = if p > 0.0 { 20.0 * (p as f32).log10() } else { -100.0 };
                        peak = Some(peak.map_or(db, |m: f32| m.max(db)));
                    }
                }
            }
        }
    }
    let _ = ws.close(None).await;
    Ok(peak)
}

/// Any obs-websocket request (used by the switcher module).
pub async fn raw_request(url: &str, password: &str, request_type: &str, data: serde_json::Value) -> Result<serde_json::Value> {
    request(url, password, request_type, data).await
}

/// Switch OBS's program scene.
pub async fn switch_scene(url: String, password: String, scene: String) -> Result<String> {
    request(&url, &password, "SetCurrentProgramScene", serde_json::json!({ "sceneName": scene })).await?;
    Ok(format!("OBS switched to \"{scene}\""))
}

/// Scene names in OBS (Settings > Test connection).
pub async fn scene_list(url: String, password: String) -> Result<Vec<String>> {
    let data = request(&url, &password, "GetSceneList", serde_json::json!({})).await?;
    Ok(data["scenes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|s| s["sceneName"].as_str().map(String::from))
        .collect())
}

/// Play a Windows system sound for new warnings without opening a console window.
pub fn play_alert_sound(urgent: bool) {
    if let Some(file) = crate::config::Settings::load_cached_alert_sound() {
        crate::audio::play(file, crate::audio::Bus::Alert, if urgent { 1.0 } else { 0.8 }, 0);
        return;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let sound = if urgent { "Hand" } else { "Exclamation" };
        let script = format!(
            "1..{} | ForEach-Object {{ [System.Media.SystemSounds]::{sound}.Play(); Start-Sleep -Milliseconds 650 }}",
            if urgent { 3 } else { 1 }
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let sound = if urgent { "/System/Library/Sounds/Sosumi.aiff" } else { "/System/Library/Sounds/Glass.aiff" };
        let _ = std::process::Command::new("afplay").arg(sound).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = urgent;
        let _ = std::process::Command::new("paplay").arg("/usr/share/sounds/freedesktop/stereo/dialog-warning.oga").spawn();
    }
}
