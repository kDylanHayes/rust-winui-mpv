//! Audio and subtitle track lists, read from mpv's `track-list` property.

use libmpv2::Mpv;

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

    /// Index of the selected audio track in `audio`.
    pub fn audio_index(&self) -> Option<usize> {
        self.audio.iter().position(|t| t.selected)
    }

    /// Index in the subtitle picker, where 0 is "Off".
    pub fn subtitle_index(&self) -> usize {
        self.subtitles
            .iter()
            .position(|t| t.selected)
            .map_or(0, |i| i + 1)
    }

    pub fn subtitle_labels(&self) -> Vec<String> {
        std::iter::once("Subtitles off".to_string())
            .chain(self.subtitles.iter().map(|t| t.label.clone()))
            .collect()
    }
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
