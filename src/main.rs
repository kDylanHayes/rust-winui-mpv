//! rust-winui-mpv: a prototype video player with a WinUI 3 frontend written
//! in Rust (windows-reactor) and libmpv for playback.
//!
//! This is the starting point for step 1 of the plan. Reactor owns the window
//! and controls; mpv still opens its own video window. Embedding mpv's video
//! inside the Reactor window is the next part of step 1.

use libmpv2::Mpv;
use windows_reactor::*;

#[derive(Clone)]
enum Message {
    PlayPause,
    Stop,
}

struct Player {
    mpv: Option<Mpv>,
    file: Option<String>,
    status: String,
    playing: bool,
}

impl Component for Player {
    type Input = ();
    type Message = Message;

    fn create(_input: &(), _context: &ComponentContext<Self>) -> Self {
        let file = std::env::args().nth(1);
        match Mpv::new() {
            Ok(mpv) => {
                let version: String = mpv
                    .get_property("mpv-version")
                    .unwrap_or_else(|_| "unknown version".into());
                let status = match &file {
                    Some(f) => format!("{version} ready. File: {f}"),
                    None => format!("{version} ready. Pass a video path: cargo run -- <file>"),
                };
                Self {
                    mpv: Some(mpv),
                    file,
                    status,
                    playing: false,
                }
            }
            Err(e) => Self {
                mpv: None,
                file,
                status: format!("Failed to start mpv: {e}"),
                playing: false,
            },
        }
    }

    fn update(&mut self, message: Message, _context: &ComponentContext<Self>) {
        let (Some(mpv), Some(file)) = (&self.mpv, &self.file) else {
            return;
        };
        let result = match message {
            Message::PlayPause if !self.playing && mpv.get_property::<String>("path").is_err() => {
                mpv.command("loadfile", &[file.as_str()])
            }
            Message::PlayPause => mpv.command("cycle", &["pause"]),
            Message::Stop => mpv.command("stop", &[]),
        };
        match result {
            Ok(()) => {
                self.playing = match message {
                    Message::PlayPause => !self.playing,
                    Message::Stop => false,
                };
            }
            Err(e) => self.status = format!("mpv error: {e}"),
        }
    }

    fn view(&self, _input: &(), context: &mut ViewContext<Self>) -> View {
        let label = if self.playing { "Pause" } else { "Play" };
        context.window_title("rust-winui-mpv");
        StackPanel::new().spacing(8.0).children((
            TextBlock::new().text(self.status.clone()),
            Button::new()
                .on_click(context.message(Message::PlayPause))
                .content(label),
            Button::new()
                .on_click(context.message(Message::Stop))
                .content("Stop"),
        ))
    }
}

fn main() {
    App::run_component::<Player>(()).unwrap();
}
