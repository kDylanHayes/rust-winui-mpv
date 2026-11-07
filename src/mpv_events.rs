//! Turns libmpv's event queue into owned updates the UI can apply.
//!
//! A background task blocks on `wait_event`, gathers what arrives over a short
//! window, and returns it as one message. `update` applies the batch and starts
//! the next poll, so mpv's events flow through the same message queue as
//! button clicks.

use std::sync::Arc;
use std::time::{Duration, Instant};

use libmpv2::events::{Event, PropertyData};
use libmpv2::{Format, Mpv};

/// Upper bound on how long one batch collects before it's delivered, so the
/// seek bar moves smoothly without re-rendering for every frame.
const BATCH_WINDOW: Duration = Duration::from_millis(200);

#[derive(Clone, Debug)]
pub enum MpvUpdate {
    Position(f64),
    Duration(f64),
    Paused(bool),
    Idle(bool),
    Volume(f64),
    FileLoaded,
    /// A `script-message` from the video window (see `video_host`): the name,
    /// e.g. "toggle-fullscreen", followed by its arguments.
    ClientMessage(Vec<String>),
    Error(String),
}

/// Asks mpv to report the properties the UI shows.
pub fn observe(mpv: &Mpv) -> libmpv2::Result<()> {
    mpv.observe_property("time-pos", Format::Double, 0)?;
    mpv.observe_property("duration", Format::Double, 0)?;
    mpv.observe_property("pause", Format::Flag, 0)?;
    mpv.observe_property("idle-active", Format::Flag, 0)?;
    mpv.observe_property("volume", Format::Double, 0)?;
    Ok(())
}

/// Blocks until at least one update arrives (or `cancelled` returns true),
/// then returns everything gathered within [`BATCH_WINDOW`].
pub fn next_batch(mpv: &Arc<Mpv>, cancelled: impl Fn() -> bool) -> Vec<MpvUpdate> {
    let mut batch = Vec::new();
    let mut first_at: Option<Instant> = None;
    while !cancelled() {
        match mpv.wait_event(0.05) {
            Some(Ok(event)) => {
                if let Some(update) = convert(event) {
                    batch.push(update);
                    first_at.get_or_insert_with(Instant::now);
                }
            }
            Some(Err(e)) => batch.push(MpvUpdate::Error(describe(&e))),
            None if !batch.is_empty() => break,
            None => {}
        }
        if first_at.is_some_and(|t| t.elapsed() >= BATCH_WINDOW) {
            break;
        }
    }
    batch
}

/// Readable text for mpv's numeric error codes, which libmpv2 shows as
/// `Raw(-16)`. The texts follow mpv's `client.h`.
pub fn describe(e: &libmpv2::Error) -> String {
    let libmpv2::Error::Raw(code) = e else {
        return e.to_string();
    };
    let text = match code {
        -13 => "Loading the file failed",
        -14 => "Couldn't initialize audio output",
        -15 => "Couldn't initialize video output",
        -16 => "Nothing to play (no audio or video in this file)",
        -17 => "Unrecognized file format",
        -18 => "Not supported",
        _ => return format!("mpv error {code}"),
    };
    text.to_string()
}

fn convert(event: Event<'_>) -> Option<MpvUpdate> {
    match event {
        Event::FileLoaded => Some(MpvUpdate::FileLoaded),
        Event::ClientMessage(args) if !args.is_empty() => Some(MpvUpdate::ClientMessage(
            args.iter().map(|a| a.to_string()).collect(),
        )),
        Event::PropertyChange { name, change, .. } => match (name, change) {
            ("time-pos", PropertyData::Double(v)) => Some(MpvUpdate::Position(v)),
            ("duration", PropertyData::Double(v)) => Some(MpvUpdate::Duration(v)),
            ("pause", PropertyData::Flag(v)) => Some(MpvUpdate::Paused(v)),
            ("idle-active", PropertyData::Flag(v)) => Some(MpvUpdate::Idle(v)),
            ("volume", PropertyData::Double(v)) => Some(MpvUpdate::Volume(v)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn property(name: &str, change: PropertyData<'_>) -> Option<MpvUpdate> {
        convert(Event::PropertyChange {
            name,
            change,
            reply_userdata: 0,
        })
    }

    #[test]
    fn converts_observed_properties() {
        assert!(matches!(
            property("time-pos", PropertyData::Double(1.5)),
            Some(MpvUpdate::Position(v)) if v == 1.5
        ));
        assert!(matches!(
            property("duration", PropertyData::Double(90.0)),
            Some(MpvUpdate::Duration(v)) if v == 90.0
        ));
        assert!(matches!(
            property("pause", PropertyData::Flag(true)),
            Some(MpvUpdate::Paused(true))
        ));
        assert!(matches!(
            property("idle-active", PropertyData::Flag(false)),
            Some(MpvUpdate::Idle(false))
        ));
        assert!(matches!(
            property("volume", PropertyData::Double(55.0)),
            Some(MpvUpdate::Volume(v)) if v == 55.0
        ));
    }

    #[test]
    fn ignores_unknown_properties_and_mismatched_formats() {
        assert!(property("speed", PropertyData::Double(1.0)).is_none());
        // e.g. time-pos is unavailable while idle and arrives as None, not a number.
        assert!(property("time-pos", PropertyData::Flag(true)).is_none());
    }

    #[test]
    fn converts_file_loaded_and_client_messages() {
        assert!(matches!(
            convert(Event::FileLoaded),
            Some(MpvUpdate::FileLoaded)
        ));
        assert!(matches!(
            convert(Event::ClientMessage(vec!["open-file", "C:\\a b.mkv"])),
            Some(MpvUpdate::ClientMessage(args)) if args == ["open-file", "C:\\a b.mkv"]
        ));
        assert!(convert(Event::ClientMessage(vec![])).is_none());
        assert!(convert(Event::Seek).is_none());
    }
}
