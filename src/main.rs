//! rust-winui-mpv: a prototype video player with a WinUI 3 frontend written
//! in Rust (windows-reactor) and libmpv for playback.
//!
//! mpv renders into a native child window over the top of the WinUI window
//! (see `video_host`), with Reactor controls in a bar underneath. mpv's
//! property changes arrive as messages (see `mpv_events`), so the UI always
//! reflects what mpv is actually doing.

mod app_icon;
mod file_dialog;
mod glyph;
mod mpv_events;
mod playlist;
mod tracks;
mod video_host;

use std::sync::Arc;
use std::time::Instant;

use libmpv2::Mpv;
use mpv_events::MpvUpdate;
use tracks::Tracks;
use video_host::VideoHost;
use windows_reactor::*;

/// Heights in device-independent pixels: the title bar (WinUI's standard
/// height), then the seek row and the controls row under the video.
/// Title bar height until the caption buttons can be measured.
const TITLE_DIPS: f64 = 32.0;
const SEEK_DIPS: f64 = 30.0;
const CONTROLS_DIPS: f64 = 44.0;
/// How long the controls stay up in fullscreen after the pointer stops.
const CONTROLS_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(2500);
const BAR_DIPS: f64 = SEEK_DIPS + CONTROLS_DIPS;

#[derive(Clone)]
enum Message {
    OpenFile,
    PlayPause,
    Stop,
    Seek(f64),
    Volume(f64),
    ToggleFullscreen,
    ToggleMute,
    Previous,
    Next,
    /// Fullscreen controls timer: hide them if the pointer has been still.
    HideControlsCheck,
    /// A pick from the Tracks menu, identified by its label.
    TrackMenu(String),
    Resized(WindowSize),
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
    muted: bool,
    playlist_pos: i64,
    playlist_count: i64,
    tracks: Tracks,
    /// Fullscreen only: whether the seek and control rows are showing, when
    /// the pointer last moved over the video, and whether a hide check is
    /// already scheduled.
    controls_shown: bool,
    last_pointer: Instant,
    hide_pending: bool,
    /// Play the file from the command line once the video window exists.
    autoplay: bool,
    /// Window width in DIPs, for dropping or trimming content when narrow.
    width: f64,
    /// Title bar height in DIPs, matched to the caption buttons.
    title_dips: f64,
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
        let Some(host) = VideoHost::create(mpv.clone()) else {
            self.error = Some("Couldn't find the WinUI window to embed video in".into());
            return;
        };
        app_icon::apply(host.window());
        if let Err(e) = mpv.set_property("wid", host.wid()) {
            self.error = Some(format!("mpv rejected wid: {e}"));
            return;
        }
        host.layout(TITLE_DIPS, BAR_DIPS, 0.0);
        self.video = Some(host);
        if std::mem::take(&mut self.autoplay)
            && let Err(e) = self.load(mpv.clone().as_ref())
        {
            self.error = Some(mpv_events::describe(&e));
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.video.as_ref().is_some_and(VideoHost::is_fullscreen)
    }

    /// The title row height, the bar height reserved below the video, and the
    /// bar height overlaid on the video. Windowed: title and reserved bar.
    /// Fullscreen: the video fills the screen, and the bar is overlaid only
    /// while `controls_shown`, so the picture never rescales.
    fn chrome(&self) -> (f64, f64, f64) {
        match (self.is_fullscreen(), self.controls_shown) {
            (false, _) => (self.title_dips, BAR_DIPS, 0.0),
            (true, true) => (0.0, 0.0, BAR_DIPS),
            (true, false) => (0.0, 0.0, 0.0),
        }
    }

    fn relayout(&self) {
        if let Some(video) = &self.video {
            let (top, bottom, overlay) = self.chrome();
            video.layout(top, bottom, overlay);
        }
    }

    /// Matches the title row to the caption buttons, which the Windows App
    /// SDK doesn't always size to the title bar height it was asked for
    /// (48 px rather than 56 px at 175% scaling, leaving a band under them).
    fn match_caption_buttons(&mut self) {
        let Some(height) = self
            .video
            .as_ref()
            .and_then(VideoHost::caption_buttons_dips)
        else {
            return;
        };
        if (height - self.title_dips).abs() > 0.1 {
            self.title_dips = height;
            self.relayout();
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.controls_shown = false;
        if let Some(video) = &mut self.video {
            video.set_fullscreen(fullscreen);
        }
        self.relayout();
    }

    /// Fullscreen: the pointer moved over the video, so show the controls
    /// and (re)start the timer that hides them again.
    fn pointer_moved(&mut self, context: &ComponentContext<Self>) {
        if !self.is_fullscreen() {
            return;
        }
        self.last_pointer = Instant::now();
        if !self.controls_shown {
            self.controls_shown = true;
            self.relayout();
        }
        self.schedule_hide(context);
    }

    fn schedule_hide(&mut self, context: &ComponentContext<Self>) {
        if std::mem::replace(&mut self.hide_pending, true) {
            return;
        }
        let wait = CONTROLS_TIMEOUT.saturating_sub(self.last_pointer.elapsed());
        context.spawn_background(move |_| {
            std::thread::sleep(wait);
            Message::HideControlsCheck
        });
    }

    fn hide_controls_check(&mut self, context: &ComponentContext<Self>) {
        self.hide_pending = false;
        if !self.is_fullscreen() || !self.controls_shown {
            return;
        }
        let over_controls = self.video.as_ref().is_some_and(|v| !v.cursor_over_video());
        if over_controls || self.last_pointer.elapsed() < CONTROLS_TIMEOUT {
            // Still in use: check again later.
            if over_controls {
                self.last_pointer = Instant::now();
            }
            self.schedule_hide(context);
        } else {
            self.controls_shown = false;
            self.relayout();
        }
    }

    fn load(&mut self, mpv: &Mpv) -> libmpv2::Result<()> {
        let Some(file) = &self.file else {
            return Ok(());
        };
        mpv.set_property("pause", false)?;
        playlist::play(mpv, file)
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
            Message::ToggleMute => mpv.command("cycle", &["mute"])?,
            Message::TrackMenu(label) => {
                if let Some((property, value)) = self.tracks.choice(&label) {
                    mpv.set_property(property, value.as_str())?;
                    self.tracks = Tracks::load(&mpv);
                }
            }
            Message::Previous => mpv.command("playlist-prev", &[])?,
            Message::Next => mpv.command("playlist-next", &[])?,
            Message::HideControlsCheck => self.hide_controls_check(context),
            Message::Resized(size) => {
                self.width = size.width;
                self.relayout();
            }
            Message::Mpv(updates) => {
                for update in updates {
                    match update {
                        // Input in the video window (see `video_host`).
                        MpvUpdate::ClientMessage(args) => {
                            match args.iter().map(String::as_str).collect::<Vec<_>>()[..] {
                                ["toggle-fullscreen"] => {
                                    let fullscreen =
                                        self.video.as_ref().is_some_and(VideoHost::is_fullscreen);
                                    self.set_fullscreen(!fullscreen);
                                }
                                ["exit-fullscreen"] => self.set_fullscreen(false),
                                ["pointer-moved"] => self.pointer_moved(context),
                                ["previous"] => mpv.command("playlist-prev", &[])?,
                                ["next"] => mpv.command("playlist-next", &[])?,
                                ["play-pause"] if self.idle => self.load(&mpv)?,
                                ["play-pause"] => mpv.command("cycle", &["pause"])?,
                                ["open-file", path] => {
                                    self.file = Some(path.to_string());
                                    self.load(&mpv)?;
                                }
                                _ => {}
                            }
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
            MpvUpdate::Muted(v) => self.muted = v,
            MpvUpdate::Path(v) => self.file = Some(v),
            MpvUpdate::PlaylistPos(v) => self.playlist_pos = v,
            MpvUpdate::PlaylistCount(v) => self.playlist_count = v,
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

    /// Title bar subtitle: the current error, else the file name.
    fn subtitle(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        let name = self
            .file
            .as_deref()
            .map(|f| f.rsplit(['\\', '/']).next().unwrap_or(f))
            .unwrap_or("Open a file or drop one on the window");
        let max_chars = subtitle_chars(self.width);
        if name.chars().count() > max_chars {
            let short: String = name.chars().take(max_chars - 1).collect();
            format!("{short}\u{2026}")
        } else {
            name.to_string()
        }
    }
}

/// How many characters of subtitle fit in the title bar at this window width.
/// The TitleBar doesn't trim its subtitle, and a long one runs under the
/// caption buttons. Roughly: the logo, title text and caption buttons take
/// ~320 DIPs, and a subtitle character averages ~7 DIPs.
fn subtitle_chars(width: f64) -> usize {
    ((width - 320.0) / 7.0).clamp(8.0, 120.0) as usize
}

/// Below this width (DIPs) the volume slider is dropped so the speaker and
/// full screen buttons keep their room; the speaker still mutes, and the
/// Up/Down keys still change the volume.
const VOLUME_SLIDER_MIN_WIDTH: f64 = 560.0;

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

fn create_mpv() -> libmpv2::Result<Mpv> {
    // Embedded mpv gets no mouse or keyboard input; `video_host` handles the
    // player keys, double-click and drag-and-drop itself.
    let mpv = Mpv::with_initializer(|init| {
        init.set_property("keep-open", false)?;
        Ok(())
    })?;
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
            muted: false,
            playlist_pos: -1,
            playlist_count: 0,
            controls_shown: false,
            last_pointer: Instant::now(),
            hide_pending: false,
            tracks: Tracks::default(),
            width: 1000.0,
            title_dips: TITLE_DIPS,
        };
        player.poll_events(context);
        player
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        self.ensure_video_host();
        self.match_caption_buttons();
        // After a bar button, hand keyboard focus back to the video so the
        // player keys work and Space doesn't re-press the same button.
        let refocus = matches!(
            message,
            Message::OpenFile
                | Message::PlayPause
                | Message::Stop
                | Message::ToggleFullscreen
                | Message::ToggleMute
        );
        if let Err(e) = self.handle(message, context) {
            self.error = Some(mpv_events::describe(&e));
        }
        if refocus && let Some(video) = &self.video {
            video.focus();
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title("rust-winui-mpv");
        context.window_visuals(WindowVisuals::new().backdrop(WindowBackdrop::Mica));
        context.on_window_size(context.callback(Message::Resized));

        let fullscreen = self.video.as_ref().is_some_and(VideoHost::is_fullscreen);
        let playing = !self.idle && !self.paused;

        // A Fluent icon button, the way Windows 11 media apps draw controls.
        let icon_button = |glyph: &str, tip: &str, message: Message, column: i32| {
            // Natural size: a fixed 40x40 left too little room inside the
            // default padding and clipped the glyphs.
            Button::new()
                .style(ButtonStyle::Subtle)
                .automation_name(tip)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(column)
                .on_click(context.message(message))
                .content(FontIcon::new().glyph(glyph))
                .tooltip(tip)
        };

        // Title bar: drawn by WinUI in the window's own theme (instead of the
        // white system caption), with the file name or error as subtitle.
        // It stays in the tree in fullscreen (its row collapses): removing it
        // makes reactor clear the window's title bar, which WinUI rejects
        // once the frame styles are gone, and reactor treats that as fatal.
        let title_bar = TitleBar::new()
            .title("rust-winui-mpv")
            .subtitle(self.subtitle())
            .preferred_height(WindowTitleBarHeight::Standard)
            .grid_column(1);
        // TitleBar in reactor 0.100 has no icon property, so the logo sits in
        // its own column just left of it.
        let logo = Image::new()
            .source_data(EncodedImage::from_static(include_bytes!(
                "../assets/logo-64.png"
            )))
            .width(16.0)
            .height(16.0)
            .margin(Thickness::new(12.0, 0.0, 0.0, 0.0))
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0);
        let title_row = Grid::new()
            .columns([GridLength::Auto, GridLength::Star(1.0)])
            .grid_row(0)
            .children((logo, title_bar));

        // Seek row: position, seek bar, duration.
        let time = |seconds: f64, column: i32| {
            TextBlock::new()
                .text(clock(seconds))
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(column)
        };
        let seek = Slider::new()
            .minimum(0.0)
            .maximum(self.duration.max(1.0))
            .value(self.position)
            .step_frequency(1.0)
            .is_enabled(!self.idle && self.duration > 0.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1)
            .on_value_changed(context.callback(Message::Seek));
        let seek_row = Grid::new()
            .column_spacing(12.0)
            .columns([GridLength::Auto, GridLength::Star(1.0), GridLength::Auto])
            .margin(Thickness::new(12.0, 0.0, 12.0, 0.0))
            .grid_row(2)
            .children((time(self.position, 0), seek, time(self.duration, 2)));

        // Controls row: open and tracks on the left, transport in the middle,
        // volume and fullscreen on the right.
        let left = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(4.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(0)
            .children((
                icon_button(glyph::OPEN, "Open file", Message::OpenFile, 0),
                // A MenuFlyout on a button rather than ComboBoxes: it fits
                // narrow windows and isn't rebuilt under the pointer by the
                // periodic re-renders from mpv's position updates.
                Button::new()
                    .style(ButtonStyle::Subtle)
                    .automation_name("Audio and subtitle tracks")
                    .is_enabled(!self.tracks.is_empty())
                    .content(FontIcon::new().glyph(glyph::TRACKS))
                    .menu(Menu::new(
                        self.tracks.menu(),
                        context.callback(Message::TrackMenu),
                    )),
            ));

        let transport = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(1)
            .children((
                icon_button(glyph::STOP, "Stop", Message::Stop, 0),
                Button::new()
                    .style(ButtonStyle::Subtle)
                    .automation_name("Previous")
                    .is_enabled(self.playlist_pos > 0)
                    .on_click(context.message(Message::Previous))
                    .content(FontIcon::new().glyph(glyph::PREVIOUS))
                    .tooltip("Previous (Page Up)"),
                Button::new()
                    .style(ButtonStyle::Accent)
                    .automation_name(if playing { "Pause" } else { "Play" })
                    .min_width(44.0)
                    .on_click(context.message(Message::PlayPause))
                    .content(FontIcon::new().glyph(if playing {
                        glyph::PAUSE
                    } else {
                        glyph::PLAY
                    }))
                    .tooltip(if playing {
                        "Pause (Space)"
                    } else {
                        "Play (Space)"
                    }),
                Button::new()
                    .style(ButtonStyle::Subtle)
                    .automation_name("Next")
                    .is_enabled(self.playlist_pos + 1 < self.playlist_count)
                    .on_click(context.message(Message::Next))
                    .content(FontIcon::new().glyph(glyph::NEXT))
                    .tooltip("Next (Page Down)"),
            ));

        let volume = Slider::new()
            .minimum(0.0)
            .maximum(100.0)
            .value(self.volume)
            .automation_name("Volume")
            .width(72.0)
            .vertical_alignment(VerticalAlignment::Center)
            .on_value_changed(context.callback(Message::Volume));
        let volume = if self.width >= VOLUME_SLIDER_MIN_WIDTH {
            View::from(volume)
        } else {
            View::empty()
        };
        let right = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(4.0)
            .horizontal_alignment(HorizontalAlignment::Right)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(2)
            .children((
                icon_button(
                    glyph::volume(self.volume, self.muted),
                    if self.muted { "Unmute (M)" } else { "Mute (M)" },
                    Message::ToggleMute,
                    0,
                ),
                volume,
                icon_button(
                    if fullscreen {
                        glyph::EXIT_FULLSCREEN
                    } else {
                        glyph::FULLSCREEN
                    },
                    if fullscreen {
                        "Exit full screen (F)"
                    } else {
                        "Full screen (F)"
                    },
                    Message::ToggleFullscreen,
                    0,
                ),
            ));

        let controls = Grid::new()
            .columns([
                GridLength::Star(1.0),
                GridLength::Auto,
                GridLength::Star(1.0),
            ])
            .margin(Thickness::new(8.0, 0.0, 8.0, 2.0))
            .grid_row(3)
            .children((left, transport, right));

        // Row 1 is left empty: the native video window sits over it. In
        // fullscreen the title bar collapses, and the control rows show only
        // while the pointer is moving.
        let (title, bar, overlay) = self.chrome();
        let (seek_height, controls_height) = if bar + overlay > 0.0 {
            (SEEK_DIPS, CONTROLS_DIPS)
        } else {
            (0.0, 0.0)
        };
        Grid::new()
            .rows([
                GridLength::Pixel(title),
                GridLength::Star(1.0),
                GridLength::Pixel(seek_height),
                GridLength::Pixel(controls_height),
            ])
            .children((title_row, seek_row, controls))
    }
}

fn main() {
    App::run_component::<Player>(()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::{clock, subtitle_chars};

    #[test]
    fn subtitle_room_scales_with_width() {
        assert_eq!(subtitle_chars(200.0), 8);
        assert_eq!(subtitle_chars(460.0), 20);
        assert!(subtitle_chars(1200.0) > 100);
        assert_eq!(subtitle_chars(5000.0), 120);
    }

    #[test]
    fn clock_formats_minutes_and_hours() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(59.9), "0:59");
        assert_eq!(clock(61.0), "1:01");
        assert_eq!(clock(3599.0), "59:59");
        assert_eq!(clock(3600.0), "1:00:00");
        assert_eq!(clock(3725.0), "1:02:05");
    }

    #[test]
    fn clock_clamps_negative_and_nan_to_zero() {
        assert_eq!(clock(-5.0), "0:00");
        assert_eq!(clock(f64::NAN), "0:00");
    }
}
