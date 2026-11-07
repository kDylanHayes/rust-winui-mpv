# rust-winui-mpv

A personal prototype: a Windows video player with a **WinUI 3 frontend written in Rust**
(Microsoft's [`windows-reactor`](https://github.com/microsoft/windows-rs/tree/master/crates/libs/reactor))
and **libmpv** for playback (via [`libmpv2`](https://crates.io/crates/libmpv2)).

`windows-reactor` is a preview crate, so expect API changes.

## Plan

1. **Video spike**: play a file in a Reactor window. Try mpv rendering natively into a
   child window first. If that doesn't work, fall back to bridging mpv's OpenGL output into a
   DirectX swapchain. *(In progress: Reactor controls drive mpv, which still opens its own window.)*
2. **Player UI**: open file, play/pause, seek, volume, fullscreen, with mpv events as messages.
3. **Polish**: subtitle/audio track menus, keyboard shortcuts, drag-and-drop, Windows 11 theming.
4. **Packaging**: self-contained folder with the Windows App Runtime and the mpv DLL.

## Setup (Windows)

1. Install Rust (MSVC toolchain) and Visual Studio Build Tools with the C++ workload.
2. Install the [Windows App Runtime](https://learn.microsoft.com/windows/apps/windows-app-sdk/downloads)
   (2.x), which Reactor uses by default.
3. Get the libmpv dev files: download the latest `mpv-dev-x86_64-*.7z` from
   [shinchiro's mpv builds](https://github.com/shinchiro/mpv-winbuild-cmake/releases) and
   extract it into a `mpv/` folder at the repo root (it's gitignored). You should have
   `mpv/libmpv-2.dll` and `mpv/mpv.def`.
4. Create the import library MSVC needs. From a *Developer PowerShell for VS*, in `mpv/`:

   ```powershell
   lib /def:mpv.def /name:libmpv-2.dll /out:mpv.lib /MACHINE:X64
   ```

5. Run it:

   ```powershell
   cargo run -- C:\path\to\video.mp4
   ```

The build script links against `mpv/mpv.lib` and copies `libmpv-2.dll` next to the exe.
To keep the mpv files elsewhere, set `MPV_DIR` to that folder.
