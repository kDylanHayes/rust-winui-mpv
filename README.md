<img src="assets/logo.svg" alt="rust-winui-mpv logo: the letters DH with a purple play button over them" width="128" height="128">

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
3. Get the libmpv dev files: download the latest `mpv-dev-x86_64-*.7z` (not the `v3` variant) from
   [shinchiro's mpv builds](https://github.com/shinchiro/mpv-winbuild-cmake/releases) and
   extract it into a `mpv/` folder at the repo root (it's gitignored). You should have
   `mpv/libmpv-2.dll`.
4. Create the import library MSVC needs. Recent builds no longer ship `mpv.def`, so generate
   it from the DLL's exports. From a *Developer PowerShell for VS* (x64), in `mpv/`:

   ```powershell
   $names = dumpbin /exports libmpv-2.dll |
     Select-String '^\s+\d+\s+[0-9A-F]+\s+[0-9A-F]{8}\s+(\S+)' |
     ForEach-Object { $_.Matches[0].Groups[1].Value }
   @('EXPORTS') + $names | Set-Content mpv.def
   lib /def:mpv.def /name:libmpv-2.dll /out:mpv.lib /MACHINE:X64
   ```

5. Run it:

   ```powershell
   cargo run -- C:\path\to\video.mp4
   ```

The build script links against `mpv/mpv.lib` and copies `libmpv-2.dll` next to the exe.
To keep the mpv files elsewhere, set `MPV_DIR` to that folder.

## Controls

| Keys | Action |
| --- | --- |
| Space, K, media Play/Pause | Play / pause |
| Left / Right | Seek 5 s (Shift: 1 s, Ctrl: 1 min) |
| J / L | Seek 10 s back / forward |
| Home, 0-9 | Jump to the start, or to 0-90 % |
| , / . | Step one frame back / forward |
| [ / ], Backspace | Slower / faster, normal speed |
| Up / Down, mouse wheel | Volume |
| M | Mute |
| A / V | Next audio / subtitle track |
| F, Alt+Enter, double-click | Full screen (Esc leaves) |
| Page Up / Page Down, media keys | Previous / next file in the folder |
| Ctrl+O | Open a file (or drop one on the window) |
| Drag the video | Move the window |

Keys work while the video has focus; clicking it gives it focus.

## Tests and CI

```powershell
cargo test
```

- Unit tests sit next to the code they cover (for example `mpv_events::convert` and `clock`).
- `tests/mpv_headless.rs` runs real libmpv with `vo=null`/`ao=null` on a generated lavfi
  test pattern, and checks that `src/mpv_events.rs` reports loading, position, pause, volume,
  end of file and key-binding script messages. Test executables run from `target/debug/deps`,
  so put your `mpv/` folder on `PATH` first (`$env:PATH = "$PWD\mpv;$env:PATH"`).

GitHub Actions (`.github/workflows/ci.yml`) does the same on `windows-latest` for every push and
pull request: it downloads the latest shinchiro mpv dev build, generates `mpv.lib` as above, then
runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build` and `cargo test`. It can't
check the WinUI window itself, so UI changes still need a local run.
