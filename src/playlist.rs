//! Previous/next track: the other media files in the opened file's folder,
//! in name order, become mpv's playlist.

use std::path::Path;

use libmpv2::Mpv;

const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "webm", "mov", "avi", "m4v", "ts", "wmv", "flv", "mpg", "mpeg", "mp3", "flac",
    "m4a", "ogg", "opus", "wav", "aac", "wma",
];

/// Folders bigger than this just play the one file.
const MAX_FILES: usize = 1000;

fn is_media(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| MEDIA_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// The media files next to `file` (including it), sorted case-insensitively,
/// and the index of `file` among them.
pub fn siblings(file: &str) -> (Vec<String>, usize) {
    let path = Path::new(file);
    let single = || (vec![file.to_string()], 0);
    let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) else {
        return single();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return single();
    };
    let mut files: Vec<_> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && is_media(p))
        .collect();
    if files.len() > MAX_FILES {
        return single();
    }
    files.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    let index = files.iter().position(|p| p == path);
    match index {
        Some(index) => (
            files
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            index,
        ),
        // Not found (e.g. a URL or unusual extension): just that file.
        None => single(),
    }
}

/// Replaces mpv's playlist with `file`'s folder and starts playing `file`.
pub fn play(mpv: &Mpv, file: &str) -> libmpv2::Result<()> {
    let (files, index) = siblings(file);
    // `stop` empties the playlist and idles, so the appends below don't start
    // anything until playlist-play-index picks the file.
    mpv.command("stop", &[])?;
    for f in &files {
        mpv.command("loadfile", &[f, "append"])?;
    }
    mpv.command("playlist-play-index", &[&index.to_string()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_media_siblings_in_order() {
        let dir = std::env::temp_dir().join(format!("rwm-playlist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.mkv", "A.mp4", "c.txt", "d.MP3"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        let b = dir.join("b.mkv").to_string_lossy().into_owned();
        let (files, index) = siblings(&b);
        let names: Vec<_> = files
            .iter()
            .map(|f| {
                Path::new(f)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, ["A.mp4", "b.mkv", "d.MP3"]);
        assert_eq!(index, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_file_plays_alone() {
        assert_eq!(
            siblings("av://lavfi:testsrc"),
            (vec!["av://lavfi:testsrc".to_string()], 0)
        );
    }
}
