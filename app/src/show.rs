//! Shows: an ordered rundown of map scenes and graphics, played to Program.

use serde::{Deserialize, Serialize};

use crate::{gfx::model::{new_id, GraphicDoc}, map::scene::MapScene};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Transition {
    Cut,
    Fade,
    Wipe,
    /// Map → map: animate the camera from the current view (MapLibre-style flyTo).
    Fly,
}

impl Transition {
    pub const ALL: [Transition; 4] = [Transition::Cut, Transition::Fade, Transition::Wipe, Transition::Fly];

    pub fn label(self) -> &'static str {
        match self {
            Transition::Cut => "Cut",
            Transition::Fade => "Fade",
            Transition::Wipe => "Wipe",
            Transition::Fly => "Fly",
        }
    }

    pub fn seconds(self) -> f32 {
        match self {
            Transition::Cut => 0.0,
            Transition::Fade => 0.6,
            Transition::Wipe => 0.5,
            Transition::Fly => 2.2,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum SceneKind {
    Map { map: MapScene },
    /// A graphic; `doc` is a snapshot, refreshed from the library when the show plays.
    Graphic { graphic_id: String, doc: GraphicDoc },
    Clip { media: String },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Scene {
    pub id: String,
    pub title: String,
    pub kind: SceneKind,
    /// Seconds before auto-advancing (0 = hold until Next).
    pub duration: f32,
    pub transition: Transition,
    #[serde(default)]
    pub notes: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub soundtrack: Option<String>,
}

impl Scene {
    pub fn map(title: &str, map: MapScene) -> Self {
        Self {
            id: new_id(),
            title: title.into(),
            kind: SceneKind::Map { map },
            duration: 12.0,
            transition: Transition::Fly,
            notes: String::new(),
            soundtrack: None,
        }
    }

    pub fn graphic(doc: &GraphicDoc) -> Self {
        Self {
            id: new_id(),
            title: doc.name.clone(),
            kind: SceneKind::Graphic {
                graphic_id: doc.id.clone(),
                doc: doc.clone(),
            },
            duration: if doc.duration > 0.0 { doc.duration } else { 8.0 },
            transition: Transition::Fade,
            notes: String::new(),
            soundtrack: None,
        }
    }

    pub fn clip(media: String) -> Self {
        Self {
            id: new_id(), title: media.clone(), kind: SceneKind::Clip { media },
            duration: 0.0, transition: Transition::Fade, notes: String::new(),
            soundtrack: None,
        }
    }

    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            SceneKind::Map { ref map } if map.data.play_on_take || map.radar_replay.is_some() || map.site_replay.as_ref().is_some_and(|replay| replay.scans.len() > 1) => "LOOP",
            SceneKind::Map { .. } => "MAP",
            SceneKind::Graphic { .. } => "GFX",
            SceneKind::Clip { .. } => "CLIP",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Show {
    pub id: String,
    pub name: String,
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub loop_show: bool,
    #[serde(default)]
    pub updated: i64,
    /// Show soundtrack: a Media file that loops under every scene while the show runs.
    /// Older saved shows have none.
    #[serde(default)]
    pub soundtrack: Option<String>,
    #[serde(default = "default_bed_volume")]
    pub soundtrack_volume: f32,
}

fn default_bed_volume() -> f32 {
    0.6
}

impl Show {
    pub fn new(name: &str) -> Self {
        Self {
            id: new_id(),
            name: name.into(),
            scenes: Vec::new(),
            loop_show: false,
            updated: chrono::Utc::now().timestamp(),
            soundtrack: None,
            soundtrack_volume: default_bed_volume(),
        }
    }

    pub fn total_seconds(&self) -> f32 {
        self.scenes.iter().map(|s| s.duration.max(0.0)).sum()
    }
}
