//! rust-winui-mpv: a prototype video player with a WinUI 3 frontend written
//! in Rust (windows-reactor) and libmpv for playback.
//!
//! Step 1 spike: mpv renders into a native child window placed over the top
//! of the WinUI window (see `video_host`), with Reactor controls in a bar
//! underneath.

mod video_host;

use libmpv2::Mpv;
use video_host::VideoHost;
use windows_reactor::*;

/// Height of the control bar under the video, in device-independent pixels.
const BAR_DIPS: f64 = 56.0;

#[derive(Clone)]
enum Message {
    PlayPause,
    Stop,
    Resized,
}

struct Player {
    mpv: Option<Mpv>,
    video: Option<VideoHost>,
    file: Option<String>,
    status: String,
    loaded: bool,
    paused: bool,
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
            self.status = "Couldn't find the WinUI window to embed video in".into();
            return;
        };
        if let Err(e) = mpv.set_property("wid", host.wid()) {
            self.status = format!("mpv rejected wid: {e}");
            return;
        }
        host.layout(BAR_DIPS);
        self.video = Some(host);
    }

    fn run(&mut self, message: &Message) -> libmpv2::Result<()> {
        let (Some(mpv), Some(file)) = (&self.mpv, &self.file) else {
            return Ok(());
        };
        match message {
            Message::PlayPause if !self.loaded => {
                mpv.set_property("pause", false)?;
                mpv.command("loadfile", &[file.as_str()])?;
                self.loaded = true;
                self.paused = false;
            }
            Message::PlayPause => {
                mpv.command("cycle", &["pause"])?;
                self.paused = !self.paused;
            }
            Message::Stop => {
                mpv.command("stop", &[])?;
                self.loaded = false;
                self.paused = false;
            }
            Message::Resized => {}
        }
        Ok(())
    }
}

impl Component for Player {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
        let file = std::env::args().nth(1);
        let (mpv, status) = match Mpv::new() {
            Ok(mpv) => {
                let version: String = mpv
                    .get_property("mpv-version")
                    .unwrap_or_else(|_| "mpv".into());
                let status = match &file {
                    Some(f) => format!("{version} ready. File: {f}"),
                    None => format!("{version} ready. Pass a video path: cargo run -- <file>"),
                };
                (Some(mpv), status)
            }
            Err(e) => (None, format!("Failed to start mpv: {e}")),
        };
        Self {
            mpv,
            video: None,
            file,
            status,
            loaded: false,
            paused: false,
        }
    }

    fn update(&mut self, message: Message, _context: &ComponentContext<Self>) {
        self.ensure_video_host();
        if let Message::Resized = message
            && let Some(video) = &self.video
        {
            video.layout(BAR_DIPS);
        }
        if let Err(e) = self.run(&message) {
            self.status = format!("mpv error: {e}");
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        context.window_title("rust-winui-mpv");
        context.on_window_size(context.callback(|_: WindowSize| Message::Resized));

        let label = if self.loaded && !self.paused {
            "Pause"
        } else {
            "Play"
        };
        let controls = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .grid_row(1)
            .children((
                Button::new()
                    .on_click(context.message(Message::PlayPause))
                    .content(label),
                Button::new()
                    .on_click(context.message(Message::Stop))
                    .content("Stop"),
                TextBlock::new().text(self.status.clone()),
            ));

        // Row 0 is left empty: the native video window sits over it.
        Grid::new()
            .rows([GridLength::Star(1.0), GridLength::Pixel(BAR_DIPS)])
            .children((controls,))
    }
}

fn main() {
    App::run_component::<Player>(()).unwrap();
}
