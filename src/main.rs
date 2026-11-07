//! rust-winui-mpv: a prototype video player with a WinUI 3 frontend written
//! in Rust (windows-reactor) and libmpv for playback.
//!
//! mpv renders into a native child window over the top of the WinUI window
//! (see `video_host`), with Reactor controls in a bar underneath. mpv's
//! property changes arrive as messages (see `mpv_events`), so the UI always
//! reflects what mpv is actually doing.

mod file_dialog;
mod mpv_events;
mod tracks;
mod video_host;

use std::sync::Arc;

use libmpv2::Mpv;
use mpv_events::MpvUpdate;
use tracks::Tracks;
use video_host::VideoHost;
use windows_reactor::*;

/// Height of the seek row and the controls row under the video, in
/// device-independent pixels.
const SEEK_DIPS: f64 = 32.0;
const CONTROLS_DIPS: f64 = 48.0;
const BAR_DIPS: f64 = SEEK_DIPS + CONTROLS_DIPS;
/// Width of the audio and subtitle pickers.
const TRACK_PICKER_DIPS: f64 = 140.0;

#[derive(Clone)]
enum Message {
    OpenFile,
    PlayPause,
    Stop,
    Seek(f64),
    Volume(f64),
    ToggleFullscreen,
    AudioTrack(Option<usize>),
    SubtitleTrack(Option<usize>),
    Resized,
    Mpv(Vec<MpvUpdate>),
}

struct Player {
    mpv: Option<Arc<Mpv>>,
    video: Option<VideoHost>,
    file: Option<String>,
    error: Option<String>,
    // Mirrors of mpv's state, updated only from mpv's own events.
    idle: bool,
    paused: bool,
    position: f64,
    duration: f64,
    volume: f64,
    tracks: Tracks,
    /// Play the file from the command line once the video window exists.
    autoplay: bool,
}

impl Player {
    /// Creates the child window and points mpv at it, once the WinUI window
    /// exists. mpv picks `wid` up when it creates its video output, so this
    /// must run before the first `loadfile`.
    fn ensure_video_host(&mut self) {
        if self.video.is_some() {
            return;
        }
        let Some(mpv) = &self.mpv else { return };
        let Some(host) = VideoHost::create() else {
            self.error = Some("Couldn't find the WinUI window to embed video in".into());
            return;
        };
        if let Err(e) = mpv.set_property("wid", host.wid()) {
            self.error = Some(format!("mpv rejected wid: {e}"));
            return;
        }
        host.layout(BAR_DIPS);
        self.video = Some(host);
        if std::mem::take(&mut self.autoplay)
            && let Err(e) = self.load(mpv.clone().as_ref())
        {
            self.error = Some(format!("mpv error: {e}"));
        }
    }

    fn bar_dips(&self) -> f64 {
        match &self.video {
            Some(video) if video.is_fullscreen() => 0.0,
            _ => BAR_DIPS,
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        let bar = if fullscreen { 0.0 } else { BAR_DIPS };
        if let Some(video) = &mut self.video {
            video.set_fullscreen(fullscreen);
            video.layout(bar);
        }
    }

    fn load(&mut self, mpv: &Mpv) -> libmpv2::Result<()> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        mpv.set_property("pause", false)?;
        mpv.command("loadfile", &[file.as_str()])
    }

    fn handle(
        &mut self,
        message: Message,
        context: &ComponentContext<Self>,
    ) -> libmpv2::Result<()> {
        let Some(mpv) = self.mpv.clone() else {
            return Ok(());
        };
        match message {
            Message::OpenFile => {
                let owner = self.video.as_ref().map(VideoHost::window);
                if let Some(path) = file_dialog::pick_video(owner) {
                    self.file = Some(path);
                    self.load(&mpv)?;
                }
            }
            Message::PlayPause if self.idle => self.load(&mpv)?,
            Message::PlayPause => mpv.command("cycle", &["pause"])?,
            Message::Stop => mpv.command("stop", &[])?,
            Message::Seek(target) => {
                // Programmatic slider updates don't echo back, so this is
                // a user drag; skip tiny moves to avoid flooding mpv.
                if !self.idle && (target - self.position).abs() >= 0.5 {
                    mpv.command("seek", &[&format!("{target:.2}"), "absolute"])?;
                }
            }
            Message::Volume(volume) => {
                if (volume - self.volume).abs() >= 0.5 {
                    mpv.set_property("volume", volume)?;
                }
            }
            Message::ToggleFullscreen => {
                let fullscreen = self.video.as_ref().is_some_and(VideoHost::is_fullscreen);
                self.set_fullscreen(!fullscreen);
            }
            Message::AudioTrack(Some(i)) => {
                if let Some(track) = self.tracks.audio.get(i) {
                    mpv.set_property("aid", track.id)?;
                    self.tracks = Tracks::load(&mpv);
                }
            }
            Message::SubtitleTrack(Some(0)) => {
                mpv.set_property("sid", "no")?;
                self.tracks = Tracks::load(&mpv);
            }
            Message::SubtitleTrack(Some(i)) => {
                if let Some(track) = self.tracks.subtitles.get(i - 1) {
                    mpv.set_property("sid", track.id)?;
                    self.tracks = Tracks::load(&mpv);
                }
            }
            Message::AudioTrack(None) | Message::SubtitleTrack(None) => {}
            Message::Resized => {
                if let Some(video) = &self.video {
                    video.layout(self.bar_dips());
                }
            }
            Message::Mpv(updates) => {
                for update in updates {
                    match update {
                        // Key bindings inside the video (see `create_mpv`).
                        MpvUpdate::ClientMessage(m) if m == "toggle-fullscreen" => {
                            let fullscreen =
                                self.video.as_ref().is_some_and(VideoHost::is_fullscreen);
                            self.set_fullscreen(!fullscreen);
                        }
                        MpvUpdate::ClientMessage(m) if m == "exit-fullscreen" => {
                            self.set_fullscreen(false)
                        }
                        other => self.apply(other),
                    }
                }
                self.poll_events(context);
            }
        }
        Ok(())
    }

    fn apply(&mut self, update: MpvUpdate) {
        match update {
            MpvUpdate::Position(v) => self.position = v,
            MpvUpdate::Duration(v) => self.duration = v,
            MpvUpdate::Paused(v) => self.paused = v,
            MpvUpdate::Volume(v) => self.volume = v,
            MpvUpdate::FileLoaded => {
                self.error = None;
                if let Some(mpv) = &self.mpv {
                    self.tracks = Tracks::load(mpv);
                }
            }
            MpvUpdate::Idle(v) => {
                self.idle = v;
                if v {
                    self.position = 0.0;
                    self.duration = 0.0;
                    self.tracks = Tracks::default();
                }
            }
            MpvUpdate::Error(e) => self.error = Some(e),
            MpvUpdate::ClientMessage(_) => {}
        }
    }

    /// Starts the background wait for mpv's next batch of events.
    fn poll_events(&self, context: &ComponentContext<Self>) {
        let Some(mpv) = self.mpv.clone() else { return };
        context.spawn_background(move |token| {
            Message::Mpv(mpv_events::next_batch(&mpv, || token.is_cancelled()))
        });
    }

    fn status(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        let name = self
            .file
            .as_deref()
            .map(|f| f.rsplit(['\\', '/']).next().unwrap_or(f))
            .unwrap_or("No file. Click Open.");
        if self.idle {
            name.to_string()
        } else {
            format!(
                "{} / {}  {name}",
                clock(self.position),
                clock(self.duration)
            )
        }
    }
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn create_mpv() -> libmpv2::Result<Mpv> {
    let mpv = Mpv::with_initializer(|init| {
        // Let mpv's own window take keys and mouse (Space, arrows, and so on).
        init.set_property("input-default-bindings", true)?;
        init.set_property("input-vo-keyboard", true)?;
        init.set_property("keep-open", false)?;
        Ok(())
    })?;
    // Embedded mpv can't resize our window, so route its fullscreen keys back
    // to the app as script messages, which arrive as `MpvUpdate::ClientMessage`.
    for (key, command) in [
        ("f", "script-message toggle-fullscreen"),
        ("MBTN_LEFT_DBL", "script-message toggle-fullscreen"),
        ("ESC", "script-message exit-fullscreen"),
    ] {
        mpv.command("keybind", &[key, command])?;
    }
    mpv_events::observe(&mpv)?;
    Ok(mpv)
}

impl Component for Player {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), context: &ComponentContext<Self>) -> Self {
        let (mpv, error) = match create_mpv() {
            Ok(mpv) => (Some(Arc::new(mpv)), None),
            Err(e) => (None, Some(format!("Failed to start mpv: {e}"))),
        };
        let file = std::env::args().nth(1);
        let player = Self {
            mpv,
            video: None,
            autoplay: file.is_some(),
            file,
            error,
            idle: true,
            paused: false,
            position: 0.0,
            duration: 0.0,
            volume: 100.0,
            tracks: Tracks::default(),
        };
        player.poll_events(context);
        player
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        self.ensure_video_host();
        if let Err(e) = self.handle(message, context) {
            self.error = Some(format!("mpv error: {e}"));
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title("rust-winui-mpv");
        context.window_visuals(WindowVisuals::new().backdrop(WindowBackdrop::Mica));
        context.on_window_size(context.callback(|_: WindowSize| Message::Resized));

        let fullscreen = self.video.as_ref().is_some_and(VideoHost::is_fullscreen);
        let play_label = if !self.idle && !self.paused {
            "Pause"
        } else {
            "Play"
        };

        let buttons = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0)
            .children((
                Button::new()
                    .on_click(context.message(Message::OpenFile))
                    .content("Open"),
                Button::new()
                    .on_click(context.message(Message::PlayPause))
                    .content(play_label),
                Button::new()
                    .on_click(context.message(Message::Stop))
                    .content("Stop"),
            ));

        let seek = Slider::new()
            .minimum(0.0)
            .maximum(self.duration.max(1.0))
            .value(self.position)
            .step_frequency(1.0)
            .is_enabled(!self.idle && self.duration > 0.0)
            .vertical_alignment(VerticalAlignment::Center)
            .margin(Thickness::new(12.0, 0.0, 12.0, 0.0))
            .grid_row(1)
            .on_value_changed(context.callback(Message::Seek));

        let audio = ComboBox::new()
            .placeholder_text("Audio")
            .items_source(self.tracks.audio.iter().map(|t| t.label.clone()))
            .selected_index(self.tracks.audio_index())
            .is_enabled(!self.tracks.audio.is_empty())
            .width(TRACK_PICKER_DIPS)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1)
            .on_selection_changed(context.callback(Message::AudioTrack));

        let subtitles = ComboBox::new()
            .placeholder_text("Subtitles")
            .items_source(self.tracks.subtitle_labels())
            .selected_index(self.tracks.subtitle_index())
            .is_enabled(!self.tracks.subtitles.is_empty())
            .width(TRACK_PICKER_DIPS)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(2)
            .on_selection_changed(context.callback(Message::SubtitleTrack));

        let status = TextBlock::new()
            .text(self.status())
            .text_trimming(TextTrimming::CharacterEllipsis)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(3);

        let volume = Slider::new()
            .minimum(0.0)
            .maximum(100.0)
            .value(self.volume)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(4)
            .on_value_changed(context.callback(Message::Volume));

        let fullscreen_button = Button::new()
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(5)
            .on_click(context.message(Message::ToggleFullscreen))
            .content(if fullscreen {
                "Exit fullscreen"
            } else {
                "Fullscreen"
            });

        let bar = Grid::new()
            .column_spacing(12.0)
            // The status column takes what's left and trims, so the buttons
            // and volume always fit; the seek bar has its own full-width row.
            .columns([
                GridLength::Auto,
                GridLength::Auto,
                GridLength::Auto,
                GridLength::Star(1.0),
                GridLength::Pixel(110.0),
                GridLength::Auto,
            ])
            .margin(Thickness::new(12.0, 0.0, 12.0, 0.0))
            .grid_row(2)
            .children((buttons, audio, subtitles, status, volume, fullscreen_button));

        // Row 0 is left empty: the native video window sits over it. In
        // fullscreen the seek and control rows collapse so video fills the
        // screen.
        let (seek_height, controls_height) = if fullscreen {
            (0.0, 0.0)
        } else {
            (SEEK_DIPS, CONTROLS_DIPS)
        };
        Grid::new()
            .rows([
                GridLength::Star(1.0),
                GridLength::Pixel(seek_height),
                GridLength::Pixel(controls_height),
            ])
            .children((seek, bar))
    }
}

fn main() {
    App::run_component::<Player>(()).unwrap();
}
