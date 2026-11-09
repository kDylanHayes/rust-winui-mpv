//! Segoe Fluent Icons code points (the default `FontIcon` font on Windows 11)
//! for the player's controls.

pub const PLAY: &str = "\u{E768}";
pub const PAUSE: &str = "\u{E769}";
pub const STOP: &str = "\u{E71A}";
pub const PREVIOUS: &str = "\u{E892}";
pub const NEXT: &str = "\u{E893}";
pub const OPEN: &str = "\u{E8E5}";
/// "ClosedCaption": the audio and subtitle tracks menu.
pub const TRACKS: &str = "\u{E7F0}";
pub const FULLSCREEN: &str = "\u{E740}";
pub const EXIT_FULLSCREEN: &str = "\u{E73F}";

const MUTE: &str = "\u{E74F}";
const VOLUME_0: &str = "\u{E992}";
const VOLUME_1: &str = "\u{E993}";
const VOLUME_2: &str = "\u{E994}";
const VOLUME_3: &str = "\u{E995}";

/// The speaker icon for a volume level, as in the Windows 11 volume flyout.
pub fn volume(level: f64, muted: bool) -> &'static str {
    match level {
        _ if muted => MUTE,
        l if l <= 0.0 => VOLUME_0,
        l if l < 34.0 => VOLUME_1,
        l if l < 67.0 => VOLUME_2,
        _ => VOLUME_3,
    }
}
