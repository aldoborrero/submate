//! Video/audio extension checks.
//!
//! Pure string/path logic with no I/O.

use camino::Utf8Path;

/// Video container extensions (lowercased, dot-prefixed).
pub const VIDEO_EXTENSIONS: &[&str] = &[
    ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".mpg", ".mpeg", ".3gp",
    ".ogv",
];

/// Audio container extensions (lowercased, dot-prefixed).
pub const AUDIO_EXTENSIONS: &[&str] = &[
    ".mp3", ".flac", ".aac", ".m4a", ".wav", ".ogg", ".opus", ".wma", ".alac", ".ape",
];

/// Whether `path` has a known video extension (case-insensitive on the
/// extension).
pub fn is_video_file(path: &str) -> bool {
    has_extension(path, VIDEO_EXTENSIONS)
}

/// Whether `path` has a known audio extension (case-insensitive on the
/// extension).
pub fn is_audio_file(path: &str) -> bool {
    has_extension(path, AUDIO_EXTENSIONS)
}

fn has_extension(path: &str, extensions: &[&str]) -> bool {
    // The final `.ext` of the last component, including its leading dot,
    // lowercased.
    let suffix = match Utf8Path::new(path).extension() {
        Some(ext) => format!(".{}", ext.to_lowercase()),
        None => return false,
    };
    extensions.contains(&suffix.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_checks() {
        assert!(is_video_file("movie.MKV"));
        assert!(is_video_file("/a/b/movie.mp4"));
        assert!(!is_video_file("movie.srt"));
        assert!(!is_video_file("noext"));
        assert!(is_audio_file("track.FLAC"));
        assert!(!is_audio_file("track.mp4"));
    }
}
