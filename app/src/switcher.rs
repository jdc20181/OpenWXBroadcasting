//! OBS integration (program / preview
//! monitoring, studio-mode transitions), plus MIDI control (pads and keys trigger app actions).

use anyhow::Result;

// ---------------------------------------------------------------------------------------------
// OBS (obs-websocket v5): program / preview monitoring and studio-mode transitions
// ---------------------------------------------------------------------------------------------

/// OBS program and preview scene names (preview is empty outside studio mode).
pub async fn obs_buses(url: String, password: String) -> Result<(String, String)> {
    let program = crate::obs::raw_request(&url, &password, "GetCurrentProgramScene", serde_json::json!({})).await?;
    let preview = crate::obs::raw_request(&url, &password, "GetCurrentPreviewScene", serde_json::json!({}))
        .await
        .ok()
        .and_then(|v| v["currentPreviewSceneName"].as_str().map(String::from))
        .unwrap_or_default();
    Ok((program["currentProgramSceneName"].as_str().unwrap_or("").to_string(), preview))
}

/// Studio mode: send the preview scene to program with OBS's current transition.
pub async fn obs_transition(url: String, password: String) -> Result<String> {
    crate::obs::raw_request(&url, &password, "TriggerStudioModeTransition", serde_json::json!({})).await?;
    Ok("OBS transition".into())
}

/// Studio mode: put a scene in preview.
pub async fn obs_preview(url: String, password: String, scene: String) -> Result<String> {
    crate::obs::raw_request(&url, &password, "SetCurrentPreviewScene", serde_json::json!({ "sceneName": scene })).await?;
    Ok(format!("OBS preview: {scene}"))
}

/// Ask OBS to save its active replay buffer in its configured recording folder.
pub async fn obs_save_replay(url: String, password: String) -> Result<()> {
    let status = crate::obs::raw_request(&url, &password, "GetReplayBufferStatus", serde_json::json!({})).await?;
    anyhow::ensure!(
        status["outputActive"].as_bool() == Some(true),
        "Start the Replay Buffer in OBS before saving a replay."
    );
    crate::obs::raw_request(&url, &password, "SaveReplayBuffer", serde_json::json!({})).await?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// MIDI control
// ---------------------------------------------------------------------------------------------

/// Note -> action map for MIDI pads / keyboards (note numbers, middle C = 60):
/// C4 next, D4 previous, E4 black, F4 live map, G4 severe mode, A4 normal mode, B4 refresh radar,
/// C5 backup scene on, D5 backup scene off. Pads on most controllers start at 36 (C2); those
/// map the same way one, two or three octaves down.
pub const MIDI_MAP: &[(u8, &str)] = &[
    (0, "Next scene"),
    (2, "Previous scene"),
    (4, "Black"),
    (5, "Live map"),
    (7, "Severe Weather Mode"),
    (9, "Normal mode"),
    (11, "Refresh radar"),
];

fn command_for(note: u8) -> Option<crate::ops::Command> {
    use crate::ops::Command;
    Some(match note % 12 {
        0 if note >= 72 => Command::Backup(true),
        2 if note >= 72 => Command::Backup(false),
        0 => Command::Next,
        2 => Command::Prev,
        4 => Command::Black(true),
        5 => Command::Live,
        7 => Command::Severe("severe".into()),
        9 => Command::Severe("off".into()),
        11 => Command::Refresh,
        _ => return None,
    })
}

/// Listen on every MIDI input; note-on messages become app commands. Returns the receiver and
/// the names of the devices opened (empty when there is no MIDI device).
pub fn start_midi() -> (std::sync::mpsc::Receiver<crate::ops::Command>, Vec<String>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut names = Vec::new();
    let Ok(probe) = midir::MidiInput::new("OpenWX probe") else {
        return (rx, names);
    };
    for port in probe.ports() {
        let name = probe.port_name(&port).unwrap_or_else(|_| "MIDI device".into());
        let Ok(input) = midir::MidiInput::new("OpenWX") else { continue };
        let tx = tx.clone();
        let connected = input.connect(
            &port,
            "openwx-in",
            move |_, msg, _| {
                // Note on (0x9n) with velocity > 0.
                if msg.len() >= 3 && msg[0] & 0xF0 == 0x90 && msg[2] > 0 {
                    if let Some(cmd) = command_for(msg[1]) {
                        let _ = tx.send(cmd);
                    }
                }
            },
            (),
        );
        match connected {
            Ok(conn) => {
                log::info!("MIDI control: listening to {name}");
                names.push(name);
                // Keep the connection open for the life of the app.
                std::mem::forget(conn);
            }
            Err(e) => log::warn!("MIDI: could not open {name}: {e}"),
        }
    }
    (rx, names)
}

/// Audio inputs of the OBS mixer: (name, volume 0-1, muted).
pub async fn mixer_inputs(s: crate::config::Settings) -> anyhow::Result<Vec<(String, f32, bool)>> {
    let list = crate::obs::raw_request(&s.obs_url, &s.obs_password, "GetInputList", serde_json::json!({})).await?;
    let mut out = Vec::new();
    for input in list.get("inputs").and_then(|i| i.as_array()).into_iter().flatten() {
        let Some(name) = input.get("inputName").and_then(|n| n.as_str()) else { continue };
        let Ok(vol) = crate::obs::raw_request(&s.obs_url, &s.obs_password, "GetInputVolume", serde_json::json!({ "inputName": name })).await else { continue };
        let mul = vol.get("inputVolumeMul").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;
        let muted = crate::obs::raw_request(&s.obs_url, &s.obs_password, "GetInputMute", serde_json::json!({ "inputName": name })).await.ok().and_then(|m| m.get("inputMuted").and_then(|v| v.as_bool())).unwrap_or(false);
        out.push((name.to_string(), mul, muted));
    }
    Ok(out)
}

/// Set an input's volume (0-1) and / or mute in OBS.
pub async fn mixer_set(s: crate::config::Settings, name: String, volume: Option<f32>, mute: Option<bool>) -> anyhow::Result<()> {
    if let Some(v) = volume {
        crate::obs::raw_request(&s.obs_url, &s.obs_password, "SetInputVolume", serde_json::json!({ "inputName": name, "inputVolumeMul": v })).await?;
    }
    if let Some(m) = mute {
        crate::obs::raw_request(&s.obs_url, &s.obs_password, "SetInputMute", serde_json::json!({ "inputName": name, "inputMuted": m })).await?;
    }
    Ok(())
}

