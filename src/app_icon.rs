//! Puts the app icon (resource ID 1, embedded by build.rs from
//! assets/icon.ico) on the WinUI window's title bar and taskbar button.
//!
//! The exe's own icon only covers Explorer and shortcuts; a window needs
//! `WM_SETICON` with a big and a small size to show it too.

use crate::bindings::{
    GetModuleHandleW, GetSystemMetrics, HWND, ICON_BIG, ICON_SMALL, IMAGE_ICON, LPARAM,
    LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CXSMICON, SendMessageW, WM_SETICON, WPARAM,
};
use windows_core::PCWSTR;

const APP_ICON_ID: u16 = 1;

pub fn apply(window: HWND) {
    let module = unsafe { GetModuleHandleW(None) };
    if module.0.is_null() {
        return;
    }
    for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
        let size = unsafe { GetSystemMetrics(metric) };
        // MAKEINTRESOURCE: a resource ID passed in place of a name.
        let name = PCWSTR(APP_ICON_ID as usize as *const u16);
        // Icons loaded without LR_SHARED live for the process, which is what
        // the window needs.
        let icon = unsafe {
            LoadImageW(
                Some(module),
                name,
                IMAGE_ICON as u32,
                size,
                size,
                LR_DEFAULTCOLOR as u32,
            )
        };
        if !icon.0.is_null() {
            unsafe {
                SendMessageW(
                    window,
                    WM_SETICON as u32,
                    WPARAM(kind as usize),
                    LPARAM(icon.0 as isize),
                )
            };
        }
    }
}
