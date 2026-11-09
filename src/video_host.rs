//! A native child window inside the WinUI window that mpv renders into.
//!
//! windows-reactor 0.100 doesn't expose its window handle, so we find our own
//! top-level WinUI window on the UI thread, then create a child window over the
//! video area and hand its handle to mpv via the `wid` option. WinUI content
//! can't draw over this child (the "airspace" limit), so the controls live in a
//! bar below it. In fullscreen a window region clips the bottom off the child
//! instead, so the controls show through without resizing the video.
//!
//! Embedded mpv doesn't take mouse or keyboard input (clicks fall through its
//! window to ours, and focus stays with WinUI), so this window handles input
//! itself: it takes focus when clicked, handles player keys and double-click,
//! and accepts dropped files. Simple playback commands go straight to mpv;
//! actions the app owns (fullscreen, play/pause, opening a file) are sent as
//! mpv `script-message`s, which come back through the event pump as
//! `MpvUpdate::ClientMessage`.
//!
//! Fullscreen uses the classic Win32 approach on the WinUI window: drop the
//! frame styles and cover the monitor, then restore the saved placement.

use std::cell::Cell;
use std::sync::{Arc, Once};
use std::time::{Duration, Instant};

use libmpv2::Mpv;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, CreateRectRgn, GetMonitorInfoW, GetStockObject, HBRUSH, MONITOR_DEFAULTTONEAREST,
    MONITORINFO, MonitorFromWindow, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetFocus, VK_DOWN, VK_ESCAPE, VK_F, VK_LEFT, VK_M, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PREV_TRACK,
    VK_NEXT, VK_PRIOR, VK_RIGHT, VK_SPACE, VK_UP,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, EnumThreadWindows, FindWindowExW, GWL_STYLE,
    GWLP_USERDATA, GetClassNameW, GetClientRect, GetCursorPos, GetParent, GetWindowLongPtrW,
    GetWindowPlacement, GetWindowRect, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, IDC_ARROW,
    LoadCursorW, RegisterClassW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOOWNERZORDER,
    SetWindowLongPtrW, SetWindowPlacement, SetWindowPos, WINDOW_EX_STYLE, WINDOWPLACEMENT,
    WM_DROPFILES, WM_KEYDOWN, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_MOUSEMOVE, WM_NCDESTROY,
    WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS, WS_MAXIMIZE, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
    WindowFromPoint,
};
use windows::core::{BOOL, PCWSTR, w};

const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";
const HOST_CLASS: PCWSTR = w!("RustWinUiMpvVideoHost");

/// Seconds the arrow keys seek by, and the volume step for Up/Down.
const SEEK_STEP: &str = "5";
const VOLUME_STEP: f64 = 5.0;

pub struct VideoHost {
    parent: HWND,
    child: HWND,
    /// Window style and placement saved while fullscreen, for restoring.
    windowed: Option<(isize, WINDOWPLACEMENT)>,
}

impl VideoHost {
    /// Creates the child window. Call on the UI thread after the window exists.
    pub fn create(mpv: Arc<Mpv>) -> Option<Self> {
        let parent = find_own_winui_window()?;
        let child = unsafe {
            let instance = GetModuleHandleW(None).ok()?;
            static REGISTER: Once = Once::new();
            REGISTER.call_once(|| {
                let class = WNDCLASSW {
                    style: CS_DBLCLKS,
                    lpfnWndProc: Some(host_proc),
                    hInstance: instance.into(),
                    hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                    hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                    lpszClassName: HOST_CLASS,
                    ..Default::default()
                };
                RegisterClassW(&class);
            });
            let child = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                HOST_CLASS,
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(instance.into()),
                None,
            )
            .ok()?;
            // Owned by the window from here on; released in WM_NCDESTROY.
            SetWindowLongPtrW(child, GWLP_USERDATA, Arc::into_raw(mpv) as isize);
            DragAcceptFiles(child, true);
            child
        };
        Some(Self {
            parent,
            child,
            windowed: None,
        })
    }

    /// The WinUI top-level window, e.g. to own dialogs.
    pub fn window(&self) -> HWND {
        self.parent
    }

    /// Gives the video keyboard focus, so player keys work and Space doesn't
    /// re-press whichever WinUI button was clicked last.
    pub fn focus(&self) {
        unsafe {
            let _ = SetFocus(Some(self.child));
        }
    }

    pub fn is_fullscreen(&self) -> bool {
        self.windowed.is_some()
    }

    /// Switches the WinUI window between fullscreen and its previous placement.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        if fullscreen == self.is_fullscreen() {
            return;
        }
        unsafe {
            if fullscreen {
                let style = GetWindowLongPtrW(self.parent, GWL_STYLE);
                let mut placement = WINDOWPLACEMENT {
                    length: size_of::<WINDOWPLACEMENT>() as u32,
                    ..Default::default()
                };
                let mut monitor = MONITORINFO {
                    cbSize: size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                let hmon = MonitorFromWindow(self.parent, MONITOR_DEFAULTTONEAREST);
                if GetWindowPlacement(self.parent, &mut placement).is_err()
                    || !GetMonitorInfoW(hmon, &mut monitor).as_bool()
                {
                    return;
                }
                SetWindowLongPtrW(
                    self.parent,
                    GWL_STYLE,
                    style & !(WS_OVERLAPPEDWINDOW.0 as isize),
                );
                let r = monitor.rcMonitor;
                // Topmost, or the taskbar stays drawn over the video.
                let _ = SetWindowPos(
                    self.parent,
                    Some(HWND_TOPMOST),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
                self.windowed = Some((style, placement));
            } else if let Some((style, placement)) = self.windowed.take() {
                // Restore the frame and the normal-position rectangle in one
                // SetWindowPos, so no frame shows the caption buttons on a
                // still-fullscreen window. SetWindowPlacement then restores
                // a maximized state, if the window had one. WS_MAXIMIZE is
                // left off until then: with it set, SetWindowPlacement sees an
                // already-maximized window and leaves the restored size.
                SetWindowLongPtrW(self.parent, GWL_STYLE, style & !(WS_MAXIMIZE.0 as isize));
                let r = placement.rcNormalPosition;
                let _ = SetWindowPos(
                    self.parent,
                    Some(HWND_NOTOPMOST),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
                let _ = SetWindowPlacement(self.parent, &placement);
            }
        }
    }

    /// Whether the mouse pointer is over the video (rather than the controls).
    /// WindowFromPoint honours the window region set by `layout`.
    pub fn cursor_over_video(&self) -> bool {
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point).is_ok() && WindowFromPoint(point) == self.child }
    }

    /// Height in DIPs of the Windows App SDK caption buttons (minimize,
    /// maximize, close), so the title bar row can match them exactly.
    pub fn caption_buttons_dips(&self) -> Option<f64> {
        unsafe {
            let buttons = FindWindowExW(
                Some(self.parent),
                None,
                w!("ReunionWindowingCaptionControls"),
                PCWSTR::null(),
            )
            .ok()?;
            let mut rect = RECT::default();
            GetWindowRect(buttons, &mut rect).ok()?;
            let height = rect.bottom - rect.top;
            let scale = GetDpiForWindow(self.parent) as f64 / 96.0;
            (height > 0 && scale > 0.0).then(|| height as f64 / scale)
        }
    }

    /// The handle to pass to mpv's `wid` option.
    pub fn wid(&self) -> i64 {
        self.child.0 as i64
    }

    /// Fills the parent's client area between a top inset (the title bar)
    /// and a bottom inset (the control bar), all in DIPs.
    ///
    /// `overlay_dips` hides the bottom of the video without resizing it:
    /// a window region clips it off, so the WinUI controls underneath show
    /// through as an overlay. Fullscreen uses this so the picture doesn't
    /// rescale and jump each time the controls show or hide.
    pub fn layout(&self, top_dips: f64, bottom_dips: f64, overlay_dips: f64) {
        unsafe {
            let mut rect = RECT::default();
            if GetClientRect(self.parent, &mut rect).is_err() {
                return;
            }
            let scale = GetDpiForWindow(self.parent) as f64 / 96.0;
            let px = |dips: f64| (dips * scale).round() as i32;
            let top = px(top_dips);
            let width = rect.right - rect.left;
            let height = (rect.bottom - rect.top - top - px(bottom_dips)).max(0);
            // HWND_TOP keeps the video above WinUI's own content window.
            let _ = SetWindowPos(
                self.child,
                Some(HWND_TOP),
                0,
                top,
                width,
                height,
                SWP_NOACTIVATE,
            );
            let overlay = px(overlay_dips);
            if overlay > 0 {
                // The window owns the region from here on.
                let region = CreateRectRgn(0, 0, width, (height - overlay).max(0));
                SetWindowRgn(self.child, Some(region), true);
            } else {
                SetWindowRgn(self.child, None, true);
            }
        }
    }
}

/// Tells the app the pointer moved over the video (it shows the controls in
/// fullscreen). Windows also sends WM_MOUSEMOVE when nothing moved, and the
/// app only needs a nudge every so often, so filter both.
fn on_mouse_move(mpv: &Mpv, lparam: LPARAM) {
    thread_local! {
        static LAST: Cell<(isize, Option<Instant>)> = const { Cell::new((-1, None)) };
    }
    let pos = lparam.0 & 0xFFFF_FFFF;
    LAST.with(|last| {
        let (last_pos, last_sent) = last.get();
        let due = last_sent.is_none_or(|t| t.elapsed() >= Duration::from_millis(250));
        if pos != last_pos && due {
            let _ = app_message(mpv, &["pointer-moved"]);
            last.set((pos, Some(Instant::now())));
        } else if pos != last_pos {
            last.set((pos, last_sent));
        }
    });
}

/// Window procedure for the video host: player keys, double-click, and drops.
unsafe extern "system" fn host_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let mpv_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Mpv;
    if msg == WM_NCDESTROY && !mpv_ptr.is_null() {
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Arc::from_raw(mpv_ptr));
        }
    } else if let Some(mpv) = unsafe { mpv_ptr.as_ref() } {
        let handled = match msg {
            WM_LBUTTONDOWN => {
                let _ = unsafe { SetFocus(Some(hwnd)) };
                true
            }
            WM_MOUSEMOVE => {
                on_mouse_move(mpv, lparam);
                false
            }
            WM_LBUTTONDBLCLK => {
                let _ = app_message(mpv, &["toggle-fullscreen"]);
                true
            }
            WM_KEYDOWN => on_key(mpv, wparam.0 as u16, lparam.0 & (1 << 30) != 0),
            WM_DROPFILES => {
                let drop = HDROP(wparam.0 as _);
                let mut path = vec![0u16; 32768];
                let len = unsafe { DragQueryFileW(drop, 0, Some(&mut path)) } as usize;
                unsafe { DragFinish(drop) };
                if len > 0 {
                    let path = String::from_utf16_lossy(&path[..len]);
                    let _ = app_message(mpv, &["open-file", &path]);
                }
                true
            }
            _ => false,
        };
        if handled {
            return LRESULT(0);
        }
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Handles a key press in the video and returns whether it was a player key.
/// `repeat` is set for auto-repeat, which only seek and volume act on.
fn on_key(mpv: &Mpv, key: u16, repeat: bool) -> bool {
    let result = match key {
        k if k == VK_SPACE.0 && !repeat => app_message(mpv, &["play-pause"]),
        k if k == VK_F.0 && !repeat => app_message(mpv, &["toggle-fullscreen"]),
        k if k == VK_ESCAPE.0 && !repeat => app_message(mpv, &["exit-fullscreen"]),
        k if k == VK_M.0 && !repeat => mpv.command("cycle", &["mute"]),
        k if (k == VK_PRIOR.0 || k == VK_MEDIA_PREV_TRACK.0) && !repeat => {
            app_message(mpv, &["previous"])
        }
        k if (k == VK_NEXT.0 || k == VK_MEDIA_NEXT_TRACK.0) && !repeat => {
            app_message(mpv, &["next"])
        }
        k if k == VK_LEFT.0 => mpv.command("seek", &[&format!("-{SEEK_STEP}"), "relative"]),
        k if k == VK_RIGHT.0 => mpv.command("seek", &[SEEK_STEP, "relative"]),
        k if k == VK_UP.0 || k == VK_DOWN.0 => {
            let step = if k == VK_UP.0 {
                VOLUME_STEP
            } else {
                -VOLUME_STEP
            };
            let volume = mpv.get_property::<f64>("volume").unwrap_or(100.0);
            // The volume slider tops out at 100, so keep mpv there too.
            mpv.set_property("volume", (volume + step).clamp(0.0, 100.0))
        }
        k if k == VK_SPACE.0
            || k == VK_F.0
            || k == VK_ESCAPE.0
            || k == VK_M.0
            || k == VK_PRIOR.0
            || k == VK_NEXT.0
            || k == VK_MEDIA_PREV_TRACK.0
            || k == VK_MEDIA_NEXT_TRACK.0 =>
        {
            Ok(())
        }
        _ => return false,
    };
    // Seeking with nothing loaded fails harmlessly; there's nothing to report.
    let _ = result;
    true
}

/// Asks the app to do something it owns. Arrives as `MpvUpdate::ClientMessage`.
fn app_message(mpv: &Mpv, args: &[&str]) -> libmpv2::Result<()> {
    mpv.command("script-message", args)
}

fn find_own_winui_window() -> Option<HWND> {
    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> BOOL {
        let mut name = [0u16; 64];
        let len = unsafe { GetClassNameW(hwnd, &mut name) } as usize;
        let is_top_level = unsafe { GetParent(hwnd) }.is_err();
        if is_top_level && String::from_utf16_lossy(&name[..len]) == WINUI_WINDOW_CLASS {
            unsafe { *(found.0 as *mut HWND) = hwnd };
            return false.into();
        }
        true.into()
    }

    let mut found = HWND::default();
    unsafe {
        let _ = EnumThreadWindows(
            GetCurrentThreadId(),
            Some(visit),
            LPARAM(&mut found as *mut HWND as isize),
        );
    }
    (!found.is_invalid()).then_some(found)
}
