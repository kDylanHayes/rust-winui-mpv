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

use crate::bindings::{
    BLACK_BRUSH, CS_DBLCLKS, ClientToScreen, CreateRectRgn, CreateWindowExW, DefWindowProcW,
    DragAcceptFiles, DragFinish, DragQueryFileW, EnumThreadWindows, FindWindowExW, GA_ROOTOWNER,
    GWL_STYLE, GWLP_USERDATA, GetAncestor, GetClassNameW, GetClientRect, GetCurrentThreadId,
    GetCursorPos, GetDpiForWindow, GetForegroundWindow, GetKeyState, GetModuleHandleW,
    GetMonitorInfoW, GetParent, GetStockObject, GetSystemMetrics, GetWindowLongPtrW,
    GetWindowPlacement, GetWindowRect, HBRUSH, HDROP, HTCAPTION, HWND, HWND_NOTOPMOST, HWND_TOP,
    HWND_TOPMOST, IDC_ARROW, LPARAM, LRESULT, LoadCursorW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow, POINT, RECT, RegisterClassW, ReleaseCapture, SM_CXDRAG, SM_CYDRAG,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SendMessageW, SetFocus, SetWindowLongPtrW,
    SetWindowPlacement, SetWindowPos, SetWindowRgn, VK_BACK, VK_CONTROL, VK_DOWN, VK_ESCAPE,
    VK_HOME, VK_LEFT, VK_MEDIA_NEXT_TRACK, VK_MEDIA_PLAY_PAUSE, VK_MEDIA_PREV_TRACK, VK_MEDIA_STOP,
    VK_NEXT, VK_NUMPAD0, VK_NUMPAD9, VK_OEM_4, VK_OEM_6, VK_OEM_COMMA, VK_OEM_PERIOD, VK_PRIOR,
    VK_RETURN, VK_RIGHT, VK_SHIFT, VK_SPACE, VK_UP, WINDOWPLACEMENT, WM_DROPFILES, WM_KEYDOWN,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_NCDESTROY,
    WM_NCLBUTTONDOWN, WM_SYSKEYDOWN, WNDCLASSW, WPARAM, WS_CAPTION, WS_CHILD, WS_CLIPSIBLINGS,
    WS_MAXIMIZE, WS_OVERLAPPEDWINDOW, WS_VISIBLE, WindowFromPoint,
};
use libmpv2::Mpv;
use windows_core::{BOOL, PCWSTR, PWSTR, w};

const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";
const HOST_CLASS: PCWSTR = w!("RustWinUiMpvVideoHost");

/// Seconds the arrow keys seek by, and the volume step for Up/Down.
const SEEK_STEP: &str = "5";
const VOLUME_STEP: f64 = 5.0;

/// Letter and digit keys' virtual-key codes are their ASCII characters; the
/// Win32 metadata has no constants for them.
const VK_0: i32 = b'0' as i32;
const VK_1: i32 = b'1' as i32;
const VK_2: i32 = b'2' as i32;
const VK_3: i32 = b'3' as i32;
const VK_4: i32 = b'4' as i32;
const VK_5: i32 = b'5' as i32;
const VK_6: i32 = b'6' as i32;
const VK_7: i32 = b'7' as i32;
const VK_8: i32 = b'8' as i32;
const VK_9: i32 = b'9' as i32;
const VK_A: i32 = b'A' as i32;
const VK_F: i32 = b'F' as i32;
const VK_J: i32 = b'J' as i32;
const VK_K: i32 = b'K' as i32;
const VK_L: i32 = b'L' as i32;
const VK_M: i32 = b'M' as i32;
const VK_O: i32 = b'O' as i32;
const VK_V: i32 = b'V' as i32;
/// One notch of a mouse wheel.
const WHEEL_DELTA: i32 = 120;
/// WM_MOUSEMOVE's wParam flag for the left button.
const MK_LBUTTON: usize = 0x0001;

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
            let instance = GetModuleHandleW(None);
            if instance.0.is_null() {
                return None;
            }
            static REGISTER: Once = Once::new();
            REGISTER.call_once(|| {
                let class = WNDCLASSW {
                    style: CS_DBLCLKS as u32,
                    lpfnWndProc: Some(host_proc),
                    hInstance: instance,
                    hCursor: LoadCursorW(None, IDC_ARROW),
                    hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                    lpszClassName: HOST_CLASS,
                    ..Default::default()
                };
                RegisterClassW(&class);
            });
            let child = CreateWindowExW(
                0,
                HOST_CLASS,
                w!(""),
                (WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS) as u32,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                Some(instance),
                None,
            );
            if child.0.is_null() {
                return None;
            }
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
                let hmon = MonitorFromWindow(self.parent, MONITOR_DEFAULTTONEAREST as u32);
                if !GetWindowPlacement(self.parent, &mut placement).as_bool()
                    || !GetMonitorInfoW(hmon, &mut monitor).as_bool()
                {
                    return;
                }
                SetWindowLongPtrW(
                    self.parent,
                    GWL_STYLE,
                    style & !(WS_OVERLAPPEDWINDOW as isize),
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
                    (SWP_NOOWNERZORDER | SWP_FRAMECHANGED) as u32,
                );
                self.windowed = Some((style, placement));
            } else if let Some((style, placement)) = self.windowed.take() {
                // Restore the frame and the normal-position rectangle in one
                // SetWindowPos, so no frame shows the caption buttons on a
                // still-fullscreen window. SetWindowPlacement then restores
                // a maximized state, if the window had one. WS_MAXIMIZE is
                // left off until then: with it set, SetWindowPlacement sees an
                // already-maximized window and leaves the restored size.
                SetWindowLongPtrW(self.parent, GWL_STYLE, style & !(WS_MAXIMIZE as isize));
                let r = placement.rcNormalPosition;
                let _ = SetWindowPos(
                    self.parent,
                    Some(HWND_NOTOPMOST),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    (SWP_NOOWNERZORDER | SWP_FRAMECHANGED) as u32,
                );
                let _ = SetWindowPlacement(self.parent, &placement);
            }
        }
    }

    /// Whether the fullscreen controls are in use: our window is in the
    /// foreground and the pointer is over our own content other than the
    /// video, including its owned popups such as the Tracks menu. A pointer
    /// on another window or monitor, or focus in another app, doesn't count.
    /// WindowFromPoint honours the window region set by `layout`.
    pub fn cursor_over_controls(&self) -> bool {
        let mut point = POINT::default();
        unsafe {
            if GetForegroundWindow() != self.parent || !GetCursorPos(&mut point).as_bool() {
                return false;
            }
            let hit = WindowFromPoint(point);
            hit != self.child && GetAncestor(hit, GA_ROOTOWNER as u32) == self.parent
        }
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
            );
            let mut rect = RECT::default();
            if buttons.0.is_null() || !GetWindowRect(buttons, &mut rect).as_bool() {
                return None;
            }
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
            if !GetClientRect(self.parent, &mut rect).as_bool() {
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
                SWP_NOACTIVATE as u32,
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
    if msg == WM_NCDESTROY as u32 && !mpv_ptr.is_null() {
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Arc::from_raw(mpv_ptr));
        }
    } else if let Some(mpv) = unsafe { mpv_ptr.as_ref() } {
        let handled = match msg as i32 {
            WM_LBUTTONDOWN => {
                let _ = unsafe { SetFocus(Some(hwnd)) };
                DRAG_FROM.set(Some(screen_point(hwnd, lparam)));
                true
            }
            WM_LBUTTONUP => {
                DRAG_FROM.set(None);
                false
            }
            WM_MOUSEMOVE => {
                if wparam.0 & MK_LBUTTON != 0 {
                    drag_window(hwnd, lparam);
                }
                on_mouse_move(mpv, lparam);
                false
            }
            WM_LBUTTONDBLCLK => {
                let _ = app_message(mpv, &["toggle-fullscreen"]);
                true
            }
            WM_KEYDOWN => on_key(mpv, wparam.0 as i32, lparam.0 & (1 << 30) != 0),
            // Alt+Enter, the Windows convention for fullscreen. Other Alt
            // keys go on to DefWindowProc (e.g. Alt+F4, Alt+Space).
            WM_SYSKEYDOWN if wparam.0 as i32 == VK_RETURN => {
                if lparam.0 & (1 << 30) == 0 {
                    let _ = app_message(mpv, &["toggle-fullscreen"]);
                }
                true
            }
            // The wheel over the video changes the volume, a step per notch.
            WM_MOUSEWHEEL => {
                let notches = (wparam.0 >> 16) as u16 as i16 as i32 / WHEEL_DELTA;
                let _ = change_volume(mpv, notches as f64 * VOLUME_STEP);
                true
            }
            WM_DROPFILES => {
                let drop = HDROP(wparam.0 as _);
                let mut path = vec![0u16; 32768];
                let len = unsafe {
                    DragQueryFileW(drop, 0, Some(PWSTR(path.as_mut_ptr())), path.len() as u32)
                } as usize;
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

thread_local! {
    /// Where the left button went down on the video, until it's released or
    /// a window drag starts.
    static DRAG_FROM: Cell<Option<POINT>> = const { Cell::new(None) };
}

fn screen_point(hwnd: HWND, lparam: LPARAM) -> POINT {
    let mut point = POINT {
        x: (lparam.0 & 0xFFFF) as i16 as i32,
        y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    };
    let _ = unsafe { ClientToScreen(hwnd, &mut point) };
    point
}

/// Moves the window when the video is dragged, like dragging the title bar.
/// The drag starts only past the system drag distance, so a click or a
/// double-click (fullscreen) still works. Not in fullscreen, where the
/// window has no frame to move.
fn drag_window(hwnd: HWND, lparam: LPARAM) {
    let Some(from) = DRAG_FROM.get() else { return };
    let now = screen_point(hwnd, lparam);
    let (dx, dy) = unsafe { (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)) };
    if (now.x - from.x).abs() < dx && (now.y - from.y).abs() < dy {
        return;
    }
    DRAG_FROM.set(None);
    unsafe {
        let parent = GetParent(hwnd);
        if parent.0.is_null() || GetWindowLongPtrW(parent, GWL_STYLE) & WS_CAPTION as isize == 0 {
            return;
        }
        // Hand the press to the window's own move loop, as if it were on
        // the caption.
        let _ = ReleaseCapture();
        let packed = ((from.y as u16 as isize) << 16) | from.x as u16 as isize;
        SendMessageW(
            parent,
            WM_NCLBUTTONDOWN as u32,
            WPARAM(HTCAPTION as usize),
            LPARAM(packed),
        );
    }
}

fn key_down(key: i32) -> bool {
    (unsafe { GetKeyState(key) }) < 0
}

/// Handles a key press in the video and returns whether it was a player key.
/// `repeat` is set for auto-repeat, which only seeking, frame steps and
/// volume act on. The keys follow mpv and YouTube where they agree.
fn on_key(mpv: &Mpv, key: i32, repeat: bool) -> bool {
    let ctrl = key_down(VK_CONTROL);
    let shift = key_down(VK_SHIFT);
    let once = !repeat;
    let seek = |seconds: &str| mpv.command("seek", &[seconds, "relative+exact"]);
    let result = match key {
        // Playback
        k if (k == VK_SPACE || k == VK_K || k == VK_MEDIA_PLAY_PAUSE) && once => {
            app_message(mpv, &["play-pause"])
        }
        k if k == VK_MEDIA_STOP && once => mpv.command("stop", &[]),
        k if k == VK_O && ctrl && once => app_message(mpv, &["open-dialog"]),
        // Seeking: arrows 5 s (Shift 1 s, Ctrl 1 min), J/L 10 s,
        // Home to the start, 0-9 to 0-90 %.
        k if k == VK_LEFT || k == VK_RIGHT => {
            let step = match (ctrl, shift) {
                (true, _) => "60",
                (false, true) => "1",
                _ => SEEK_STEP,
            };
            if k == VK_LEFT {
                seek(&format!("-{step}"))
            } else {
                seek(step)
            }
        }
        k if k == VK_J => seek("-10"),
        k if k == VK_L => seek("10"),
        k if k == VK_HOME && once => mpv.command("seek", &["0", "absolute"]),
        k if (VK_0..=VK_9).contains(&k) && once => {
            let percent = ((k - VK_0) * 10).to_string();
            mpv.command("seek", &[&percent, "absolute-percent"])
        }
        k if (VK_NUMPAD0..=VK_NUMPAD9).contains(&k) && once => {
            let percent = ((k - VK_NUMPAD0) * 10).to_string();
            mpv.command("seek", &[&percent, "absolute-percent"])
        }
        // Frame by frame (pauses), and speed.
        k if k == VK_OEM_PERIOD => mpv.command("frame-step", &[]),
        k if k == VK_OEM_COMMA => mpv.command("frame-back-step", &[]),
        k if k == VK_OEM_4 || k == VK_OEM_6 => {
            let factor = if k == VK_OEM_6 { "1.1" } else { "0.9091" };
            mpv.command("multiply", &["speed", factor])
                .and_then(|()| show_speed(mpv))
        }
        k if k == VK_BACK && once => mpv
            .set_property("speed", 1.0)
            .and_then(|()| show_speed(mpv)),
        // Volume
        k if k == VK_UP || k == VK_DOWN => change_volume(
            mpv,
            if k == VK_UP {
                VOLUME_STEP
            } else {
                -VOLUME_STEP
            },
        ),
        k if k == VK_M && once => mpv.command("cycle", &["mute"]),
        // Tracks: A cycles audio, V cycles subtitles (including off).
        k if k == VK_A && once => mpv.command("cycle", &["audio"]).and_then(|()| {
            mpv.command(
                "show-text",
                &["Audio: ${current-tracks/audio/title:${aid}}"],
            )
        }),
        k if k == VK_V && once => mpv.command("cycle", &["sub"]).and_then(|()| {
            mpv.command(
                "show-text",
                &["Subtitles: ${current-tracks/sub/title:${sid}}"],
            )
        }),
        // Window and playlist
        k if k == VK_F && once => app_message(mpv, &["toggle-fullscreen"]),
        k if k == VK_ESCAPE && once => app_message(mpv, &["exit-fullscreen"]),
        k if (k == VK_PRIOR || k == VK_MEDIA_PREV_TRACK) && once => app_message(mpv, &["previous"]),
        k if (k == VK_NEXT || k == VK_MEDIA_NEXT_TRACK) && once => app_message(mpv, &["next"]),
        // A held key's repeats are still ours, just ignored.
        k if PLAYER_KEYS.contains(&k) => Ok(()),
        _ => return false,
    };
    // Seeking with nothing loaded fails harmlessly; there's nothing to report.
    let _ = result;
    true
}

/// Every key `on_key` handles, so their auto-repeats are swallowed too.
const PLAYER_KEYS: &[i32] = &[
    VK_SPACE,
    VK_K,
    VK_MEDIA_PLAY_PAUSE,
    VK_MEDIA_STOP,
    VK_O,
    VK_HOME,
    VK_BACK,
    VK_M,
    VK_A,
    VK_V,
    VK_F,
    VK_ESCAPE,
    VK_PRIOR,
    VK_NEXT,
    VK_MEDIA_PREV_TRACK,
    VK_MEDIA_NEXT_TRACK,
    VK_0,
    VK_1,
    VK_2,
    VK_3,
    VK_4,
    VK_5,
    VK_6,
    VK_7,
    VK_8,
    VK_9,
    VK_NUMPAD0,
    VK_NUMPAD0 + 1,
    VK_NUMPAD0 + 2,
    VK_NUMPAD0 + 3,
    VK_NUMPAD0 + 4,
    VK_NUMPAD0 + 5,
    VK_NUMPAD0 + 6,
    VK_NUMPAD0 + 7,
    VK_NUMPAD0 + 8,
    VK_NUMPAD9,
];

fn show_speed(mpv: &Mpv) -> libmpv2::Result<()> {
    mpv.command("show-text", &["Speed: ${speed}x"])
}

/// Changes the volume by `step`, within the 0-100 the volume slider shows.
fn change_volume(mpv: &Mpv, step: f64) -> libmpv2::Result<()> {
    let volume = mpv.get_property::<f64>("volume").unwrap_or(100.0);
    mpv.set_property("volume", (volume + step).clamp(0.0, 100.0))
}

/// Asks the app to do something it owns. Arrives as `MpvUpdate::ClientMessage`.
fn app_message(mpv: &Mpv, args: &[&str]) -> libmpv2::Result<()> {
    mpv.command("script-message", args)
}

fn find_own_winui_window() -> Option<HWND> {
    unsafe extern "system" fn visit(hwnd: HWND, found: LPARAM) -> BOOL {
        let mut name = [0u16; 64];
        let len =
            unsafe { GetClassNameW(hwnd, PWSTR(name.as_mut_ptr()), name.len() as i32) } as usize;
        let is_top_level = unsafe { GetParent(hwnd) }.0.is_null();
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
    (!found.0.is_null()).then_some(found)
}
