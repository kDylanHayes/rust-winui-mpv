//! A native child window inside the WinUI window that mpv renders into.
//!
//! windows-reactor 0.100 doesn't expose its window handle, so we find our own
//! top-level WinUI window on the UI thread, then create a plain child window
//! over the video area and hand its handle to mpv via the `wid` option.
//! WinUI content can't draw over this child (the "airspace" limit), so the
//! controls live in a bar below it.

use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, EnumThreadWindows, GetClassNameW, GetClientRect, GetParent, HWND_TOP,
    SWP_NOACTIVATE, SetWindowPos, WINDOW_EX_STYLE, WS_CHILD, WS_CLIPSIBLINGS, WS_DISABLED,
    WS_VISIBLE,
};
use windows::core::{BOOL, w};

const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";

pub struct VideoHost {
    parent: HWND,
    child: HWND,
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
                // Disabled so clicks fall through to the parent instead of a
                // STATIC control; mpv handles its own input window inside.
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_DISABLED,
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
        Some(Self { parent, child })
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
