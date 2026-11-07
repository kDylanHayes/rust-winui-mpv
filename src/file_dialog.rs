//! The standard Windows "Open" dialog, via the classic common dialog API.

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::core::{PCWSTR, PWSTR, w};

/// Shows a modal Open dialog owned by `owner`. Returns the chosen path, or
/// `None` if the user cancelled.
pub fn pick_video(owner: Option<HWND>) -> Option<String> {
    let mut buffer = [0u16; 4096];
    // Pairs of "label\0pattern\0", ending with an extra \0.
    let filter = w!(
        "Videos\0*.mkv;*.mp4;*.webm;*.mov;*.avi;*.m4v;*.ts\0Audio\0*.mp3;*.flac;*.m4a;*.ogg;*.opus;*.wav\0All files\0*.*\0"
    );
    let mut ofn = OPENFILENAMEW {
        lStructSize: size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: owner.unwrap_or_default(),
        lpstrFilter: filter,
        lpstrFile: PWSTR(buffer.as_mut_ptr()),
        nMaxFile: buffer.len() as u32,
        lpstrTitle: PCWSTR::null(),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST,
        ..Default::default()
    };
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}
