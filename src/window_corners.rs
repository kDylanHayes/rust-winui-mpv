//! Asks DWM for Windows 11's rounded window corners.
//!
//! Windows rounds most top-level windows by itself, but the WinUI window
//! here came out square-cornered, so the preference is set explicitly. On
//! Windows 10 the attribute doesn't exist and the call fails harmlessly.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute,
};

pub fn round(window: HWND) {
    let preference = DWMWCP_ROUND;
    let _ = unsafe {
        DwmSetWindowAttribute(
            window,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const preference).cast(),
            size_of_val(&preference) as u32,
        )
    };
}
