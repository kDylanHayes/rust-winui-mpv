//! Audio and subtitle track lists, read from mpv's `track-list` property.

use libmpv2::Mpv;
use windows_reactor::MenuItem;

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub id: i64,
    pub label: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Tracks {
    pub audio: Vec<Track>,
    pub subtitles: Vec<Track>,
}

impl Tracks {
    /// Reads the current file's tracks. Call after mpv reports `FileLoaded`
    /// (its default track choice is made by then) or after changing tracks.
    pub fn load(mpv: &Mpv) -> Self {
        let mut tracks = Self::default();
        let count: i64 = mpv.get_property("track-list/count").unwrap_or(0);
        for i in 0..count {
            let get = |field: &str| -> Option<String> {
                mpv.get_property(&format!("track-list/{i}/{field}")).ok()
            };
            let Some(id) = mpv.get_property::<i64>(&format!("track-list/{i}/id")).ok() else {
                continue;
            };
            let selected = mpv
                .get_property::<bool>(&format!("track-list/{i}/selected"))
                .unwrap_or(false);
            let label = label(id, get("lang"), get("title"), get("codec"));
            let track = Track {
                id,
                label,
                selected,
            };
            match get("type").as_deref() {
                Some("audio") => tracks.audio.push(track),
                Some("sub") => tracks.subtitles.push(track),
                _ => {}
            }
        }
        tracks
    }

    pub fn is_empty(&self) -> bool {
        self.audio.is_empty() && self.subtitles.is_empty()
    }

    /// Items for the Tracks menu: an Audio and a Subtitles submenu, with the
    /// current choice marked.
    pub fn menu(&self) -> Vec<MenuItem> {
        let item = |key: String, label: String, selected: bool| {
            let mark = if selected { CHECK } else { NO_CHECK };
            MenuItem::item(key, format!("{mark}{label}"))
        };
        let audio = self
            .audio
            .iter()
            .map(|t| item(format!("a{}", t.id), audio_label(t), t.selected));
        let subtitles_off = !self.subtitles.iter().any(|t| t.selected);
        let subtitles = std::iter::once(item("s-off".into(), SUBTITLES_OFF.into(), subtitles_off))
            .chain(
                self.subtitles
                    .iter()
                    .map(|t| item(format!("s{}", t.id), subtitle_label(t), t.selected)),
            );
        vec![
            MenuItem::submenu("audio", "Audio", audio),
            MenuItem::submenu("subtitles", "Subtitles", subtitles),
        ]
    }

    /// Maps a clicked menu label back to the mpv property and value to set.
    pub fn choice(&self, clicked: &str) -> Option<(&'static str, String)> {
        let label = clicked
            .strip_prefix(CHECK)
            .or_else(|| clicked.strip_prefix(NO_CHECK))
            .unwrap_or(clicked);
        if label == SUBTITLES_OFF {
            return Some(("sid", "no".into()));
        }
        if let Some(t) = self.audio.iter().find(|t| audio_label(t) == label) {
            return Some(("aid", t.id.to_string()));
        }
        self.subtitles
            .iter()
            .find(|t| subtitle_label(t) == label)
            .map(|t| ("sid", t.id.to_string()))
    }
}

const CHECK: &str = "\u{2713}  ";
const NO_CHECK: &str = "    ";
const SUBTITLES_OFF: &str = "Off";

// The menu reports clicks by label, so labels carry the track id to stay
// unique even when two tracks share a language and codec.
// Audio and subtitle ids overlap (both start at 1), so the kind is part of
// the label too.
fn audio_label(t: &Track) -> String {
    format!("Audio {}: {}", t.id, t.label)
}

fn subtitle_label(t: &Track) -> String {
    format!("Subtitle {}: {}", t.id, t.label)
}

/// "English: Commentary (aac)", falling back to "Track 2".
fn label(id: i64, lang: Option<String>, title: Option<String>, codec: Option<String>) -> String {
    let name = match (lang, title) {
        (Some(lang), Some(title)) => format!("{lang}: {title}"),
        (Some(text), None) | (None, Some(text)) => text,
        (None, None) => format!("Track {id}"),
    };
    match codec {
        Some(codec) => format!("{name} ({codec})"),
        None => name,
    }
}
