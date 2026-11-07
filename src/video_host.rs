//! A native child window inside the WinUI window that mpv renders into.
//!
//! windows-reactor 0.100 doesn't expose its window handle, so we find our own
//! top-level WinUI window on the UI thread, then create a plain child window
//! over the video area and hand its handle to mpv via the `wid` option.
//! WinUI content can't draw over this child (the "airspace" limit), so the
//! controls live in a bar below it.
//!
//! Fullscreen uses the classic Win32 approach on the WinUI window: drop the
//! frame styles and cover the monitor, then restore the saved placement.

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, EnumThreadWindows, GWL_STYLE, GetClassNameW, GetClientRect, GetParent,
    GetWindowLongPtrW, GetWindowPlacement, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SetWindowLongPtrW,
    SetWindowPlacement, SetWindowPos, WINDOW_EX_STYLE, WINDOWPLACEMENT, WS_CHILD, WS_CLIPSIBLINGS,
    WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};
use windows::core::{BOOL, w};

const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";

pub struct VideoHost {
    parent: HWND,
    child: HWND,
    /// Window style and placement saved while fullscreen, for restoring.
    windowed: Option<(isize, WINDOWPLACEMENT)>,
}

impl VideoHost {
    /// Creates the child window. Call on the UI thread after the window exists.
    pub fn create() -> Option<Self> {
        let parent = find_own_winui_window()?;
        let child = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
                // Left enabled: mpv creates its own window inside this one and
                // handles mouse and keyboard input there.
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                0,
                0,
                Some(parent),
                None,
                None,
                None,
            )
            .ok()?
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
