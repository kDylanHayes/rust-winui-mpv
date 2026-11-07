//! Puts the app icon (resource ID 1, embedded by build.rs from
//! assets/icon.ico) on the WinUI window's title bar and taskbar button.
//!
//! The exe's own icon only covers Explorer and shortcuts; a window needs
//! `WM_SETICON` with a big and a small size to show it too.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, ICON_BIG, ICON_SMALL, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON,
    SM_CXSMICON, SendMessageW, WM_SETICON,
};
use windows::core::PCWSTR;

const APP_ICON_ID: u16 = 1;

pub fn apply(window: HWND) {
    let Ok(module) = (unsafe { GetModuleHandleW(None) }) else {
        return;
    };
    for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
        let size = unsafe { GetSystemMetrics(metric) };
        // MAKEINTRESOURCE: a resource ID passed in place of a name.
        let name = PCWSTR(APP_ICON_ID as usize as *const u16);
        // Icons loaded without LR_SHARED live for the process, which is what
        // the window needs.
        if let Ok(icon) = unsafe {
            LoadImageW(
                Some(module.into()),
                name,
                IMAGE_ICON,
                size,
                size,
                LR_DEFAULTCOLOR,
            )
        } {
            unsafe {
                SendMessageW(
                    window,
                    WM_SETICON,
                    Some(WPARAM(kind as usize)),
                    Some(LPARAM(icon.0 as isize)),
                )
            };
        }
    }
}
