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
    /// A `script-message` from a key binding, e.g. "toggle-fullscreen".
    ClientMessage(String),
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
            Some(Err(e)) => batch.push(MpvUpdate::Error(e.to_string())),
            None if !batch.is_empty() => break,
            None => {}
        }
        if first_at.is_some_and(|t| t.elapsed() >= BATCH_WINDOW) {
            break;
        }
    }
    batch
}

fn convert(event: Event<'_>) -> Option<MpvUpdate> {
    match event {
        Event::FileLoaded => Some(MpvUpdate::FileLoaded),
        Event::ClientMessage(args) => args
            .first()
            .map(|m| MpvUpdate::ClientMessage(m.to_string())),
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
