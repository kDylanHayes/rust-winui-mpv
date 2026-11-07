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
/// Height of the custom title bar (WinUI's "tall" title bar).
const TITLE_DIPS: f64 = 48.0;
const SEEK_DIPS: f64 = 40.0;
const CONTROLS_DIPS: f64 = 56.0;
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
    /// A pick from the Tracks menu, identified by its label.
    TrackMenu(String),
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
    muted: bool,
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
        let Some(host) = VideoHost::create(mpv.clone()) else {
            self.error = Some("Couldn't find the WinUI window to embed video in".into());
            return;
        };
        app_icon::apply(host.window());
        if let Err(e) = mpv.set_property("wid", host.wid()) {
            self.error = Some(format!("mpv rejected wid: {e}"));
            return;
        }
        host.layout(TITLE_DIPS, BAR_DIPS);
        self.video = Some(host);
        if std::mem::take(&mut self.autoplay)
            && let Err(e) = self.load(mpv.clone().as_ref())
        {
            self.error = Some(mpv_events::describe(&e));
        }
    }

    /// Space above and below the video: title bar and control bar, or none
    /// in fullscreen.
    fn insets(fullscreen: bool) -> (f64, f64) {
        if fullscreen {
            (0.0, 0.0)
        } else {
            (TITLE_DIPS, BAR_DIPS)
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        let (top, bottom) = Self::insets(fullscreen);
        if let Some(video) = &mut self.video {
            video.set_fullscreen(fullscreen);
            video.layout(top, bottom);
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
            Message::ToggleMute => mpv.command("cycle", &["mute"])?,
            Message::TrackMenu(label) => {
                if let Some((property, value)) = self.tracks.choice(&label) {
                    mpv.set_property(property, value.as_str())?;
                    self.tracks = Tracks::load(&mpv);
                }
            }
            Message::Resized => {
                if let Some(video) = &self.video {
                    let (top, bottom) = Self::insets(video.is_fullscreen());
                    video.layout(top, bottom);
                }
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
        self.file
            .as_deref()
            .map(|f| f.rsplit(['\\', '/']).next().unwrap_or(f))
            .unwrap_or("Open a file or drop one on the window")
            .to_string()
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
            tracks: Tracks::default(),
        };
        player.poll_events(context);
        player
    }

    fn update(&mut self, message: Message, context: &ComponentContext<Self>) {
        self.ensure_video_host();
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
        context.on_window_size(context.callback(|_: WindowSize| Message::Resized));

        let fullscreen = self.video.as_ref().is_some_and(VideoHost::is_fullscreen);
        let playing = !self.idle && !self.paused;

        // A Fluent icon button, the way Windows 11 media apps draw controls.
        let icon_button = |glyph: &str, tip: &str, message: Message, column: i32| {
            Button::new()
                .style(ButtonStyle::Subtle)
                .width(40.0)
                .height(40.0)
                .vertical_alignment(VerticalAlignment::Center)
                .grid_column(column)
                .on_click(context.message(message))
                .content(FontIcon::new().glyph(glyph))
                .tooltip(tip)
        };

        // Title bar: drawn by WinUI in the window's own theme (instead of the
        // white system caption), with the file name or error as subtitle.
        // Omitted in fullscreen, where the frame is gone too.
        let title_bar: Option<View> = (!fullscreen).then(|| {
            TitleBar::new()
                .title("rust-winui-mpv")
                .subtitle(self.subtitle())
                .preferred_height(WindowTitleBarHeight::Tall)
                .grid_row(0)
                .into()
        });

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
            .margin(Thickness::new(16.0, 0.0, 16.0, 0.0))
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
                    .width(40.0)
                    .height(40.0)
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
                    .style(ButtonStyle::Accent)
                    .width(48.0)
                    .height(48.0)
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
            ));

        let volume = Slider::new()
            .minimum(0.0)
            .maximum(100.0)
            .value(self.volume)
            .width(110.0)
            .vertical_alignment(VerticalAlignment::Center)
            .on_value_changed(context.callback(Message::Volume));
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
            .margin(Thickness::new(12.0, 0.0, 12.0, 4.0))
            .grid_row(3)
            .children((left, transport, right));

        // Row 1 is left empty: the native video window sits over it. In
        // fullscreen the title bar and both control rows collapse.
        let (title, seek_height, controls_height) = if fullscreen {
            (0.0, 0.0, 0.0)
        } else {
            (TITLE_DIPS, SEEK_DIPS, CONTROLS_DIPS)
        };
        Grid::new()
            .rows([
                GridLength::Pixel(title),
                GridLength::Star(1.0),
                GridLength::Pixel(seek_height),
                GridLength::Pixel(controls_height),
            ])
            .children((title_bar.unwrap_or_else(View::empty), seek_row, controls))
    }
}

fn main() {
    App::run_component::<Player>(()).unwrap();
}

#[cfg(test)]
mod tests {
    use super::clock;

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
