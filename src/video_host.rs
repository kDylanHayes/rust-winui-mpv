//! A native child window inside the WinUI window that mpv renders into.
//!
//! windows-reactor 0.100 doesn't expose its window handle, so we find our own
//! top-level WinUI window on the UI thread, then create a child window over the
//! video area and hand its handle to mpv via the `wid` option. WinUI content
//! can't draw over this child (the "airspace" limit), so the controls live in a
//! bar below it.
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

use std::sync::{Arc, Once};

use libmpv2::Mpv;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BLACK_BRUSH, GetMonitorInfoW, GetStockObject, HBRUSH, MONITOR_DEFAULTTONEAREST, MONITORINFO,
    MonitorFromWindow,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetFocus, VK_DOWN, VK_ESCAPE, VK_F, VK_LEFT, VK_M, VK_RIGHT, VK_SPACE, VK_UP,
};
use windows::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_DBLCLKS, CreateWindowExW, DefWindowProcW, EnumThreadWindows, GWL_STYLE, GWLP_USERDATA,
    GetClassNameW, GetClientRect, GetParent, GetWindowLongPtrW, GetWindowPlacement, HWND_NOTOPMOST,
    HWND_TOP, HWND_TOPMOST, IDC_ARROW, LoadCursorW, RegisterClassW, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SetWindowLongPtrW,
    SetWindowPlacement, SetWindowPos, WINDOW_EX_STYLE, WINDOWPLACEMENT, WM_DROPFILES, WM_KEYDOWN,
    WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_NCDESTROY, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
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
                SetWindowLongPtrW(self.parent, GWL_STYLE, style);
                let _ = SetWindowPlacement(self.parent, &placement);
                let _ = SetWindowPos(
                    self.parent,
                    Some(HWND_NOTOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                );
            }
        }
    }

    /// The handle to pass to mpv's `wid` option.
    pub fn wid(&self) -> i64 {
        self.child.0 as i64
    }

    /// Fills the parent's client area except for a bottom bar of `bar_dips`.
    pub fn layout(&self, bar_dips: f64) {
        unsafe {
            let mut rect = RECT::default();
            if GetClientRect(self.parent, &mut rect).is_err() {
                return;
            }
            let scale = GetDpiForWindow(self.parent) as f64 / 96.0;
            let bar = (bar_dips * scale).round() as i32;
            let width = rect.right - rect.left;
            let height = (rect.bottom - rect.top - bar).max(0);
            // HWND_TOP keeps the video above WinUI's own content window.
            let _ = SetWindowPos(
                self.child,
                Some(HWND_TOP),
                0,
                0,
                width,
                height,
                SWP_NOACTIVATE,
            );
        }
    }
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
        k if k == VK_SPACE.0 || k == VK_F.0 || k == VK_ESCAPE.0 || k == VK_M.0 => Ok(()),
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
