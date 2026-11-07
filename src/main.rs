//! rust-winui-mpv: a prototype video player with a WinUI 3 frontend written
//! in Rust (windows-reactor) and libmpv for playback.
//!
//! mpv renders into a native child window over the top of the WinUI window
//! (see `video_host`), with Reactor controls in a bar underneath. mpv's
//! property changes arrive as messages (see `mpv_events`), so the UI always
//! reflects what mpv is actually doing.

mod file_dialog;
mod mpv_events;
mod video_host;

use std::sync::Arc;

use libmpv2::Mpv;
use mpv_events::MpvUpdate;
use video_host::VideoHost;
use windows_reactor::*;

/// Height of the control bar under the video, in device-independent pixels.
const BAR_DIPS: f64 = 56.0;

#[derive(Clone)]
enum Message {
    OpenFile,
    PlayPause,
    Stop,
    Seek(f64),
    Volume(f64),
    ToggleFullscreen,
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
        // Keep mpv's property in step, so toggling from the button and from
        // inside the video (double-click, f, Esc) agree.
        if let Some(mpv) = &self.mpv {
            let _ = mpv.set_property("fullscreen", fullscreen);
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
            Message::Resized => {
                if let Some(video) = &self.video {
                    video.layout(self.bar_dips());
                }
            }
            Message::Mpv(updates) => {
                for update in updates {
                    self.apply(update);
                }
                // Mirror mpv's own fullscreen toggle (double-click, f, Esc
                // inside the video) onto the window.
                if let Ok(fullscreen) = mpv.get_property::<bool>("fullscreen") {
                    let current = self.video.as_ref().is_some_and(VideoHost::is_fullscreen);
                    if fullscreen != current {
                        self.set_fullscreen(fullscreen);
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
            MpvUpdate::FileLoaded => self.error = None,
            MpvUpdate::Idle(v) => {
                self.idle = v;
                if v {
                    self.position = 0.0;
                    self.duration = 0.0;
                }
            }
            MpvUpdate::Error(e) => self.error = Some(e),
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
        // Let mpv's own window take keys and mouse: Space, arrows, f,
        // double-click for fullscreen, and so on.
        init.set_property("input-default-bindings", true)?;
        init.set_property("input-vo-keyboard", true)?;
        init.set_property("keep-open", false)?;
        Ok(())
    })?;
    mpv_events::observe(&mpv)?;
    mpv.observe_property("fullscreen", libmpv2::Format::Flag, 0)?;
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
        let player = Self {
            mpv,
            video: None,
            file: std::env::args().nth(1),
            error,
            idle: true,
            paused: false,
            position: 0.0,
            duration: 0.0,
            volume: 100.0,
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
            .grid_column(1)
            .on_value_changed(context.callback(Message::Seek));

        let status = TextBlock::new()
            .text(self.status())
            .text_trimming(TextTrimming::CharacterEllipsis)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(2);

        let volume = Slider::new()
            .minimum(0.0)
            .maximum(100.0)
            .value(self.volume)
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(3)
            .on_value_changed(context.callback(Message::Volume));

        let fullscreen_button = Button::new()
            .vertical_alignment(VerticalAlignment::Center)
            .grid_column(4)
            .on_click(context.message(Message::ToggleFullscreen))
            .content(if fullscreen {
                "Exit fullscreen"
            } else {
                "Fullscreen"
            });

        let bar = Grid::new()
            .column_spacing(12.0)
            .columns([
                GridLength::Auto,
                GridLength::Star(1.0),
                GridLength::Pixel(220.0),
                GridLength::Pixel(110.0),
                GridLength::Auto,
            ])
            .margin(Thickness::new(12.0, 0.0, 12.0, 0.0))
            .grid_row(1)
            .children((buttons, seek, status, volume, fullscreen_button));

        // Row 0 is left empty: the native video window sits over it. In
        // fullscreen the bar row collapses so video fills the screen.
        let bar_height = if fullscreen { 0.0 } else { BAR_DIPS };
        Grid::new()
            .rows([GridLength::Star(1.0), GridLength::Pixel(bar_height)])
            .children((bar,))
    }
}

fn main() {
    App::run_component::<Player>(()).unwrap();
}
