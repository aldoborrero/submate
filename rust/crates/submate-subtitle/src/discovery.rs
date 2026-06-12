//! On-disk subtitle discovery and filename-language parsing.
//!
//! Pure-data port of the filesystem half of `submate/subtitle.py`: directory
//! scanning, the dot-boundary stem match, the reversed filename-language scan,
//! and the LRC path helpers. Depends only on `std` plus the already-ported
//! [`submate_lang::LanguageCode`] table — no media demux and no subtitle-format
//! parsing (the PyAV internal probe is a separate slice).
//!
//! Filename component semantics (`stem`/`suffix`/`with_suffix`) mirror Python
//! `pathlib.PurePath` exactly, so they agree with the conventions the rest of
//! the port (e.g. `submate-paths`) relies on. The implementation lives here
//! rather than importing `camino` to keep this crate on `std` only.

use std::path::{Path, PathBuf};

use submate_lang::LanguageCode;

/// Subtitle file extensions used for on-disk discovery (lowercased, dot
/// prefixed). Mirrors `submate.subtitle.SUBTITLE_EXTENSIONS` exactly — this is
/// the WIDE discovery set, distinct from the narrower translate-path set in
/// `cli/commands/translate.py`.
pub const SUBTITLE_EXTENSIONS: &[&str] =
    &[".srt", ".vtt", ".sub", ".ass", ".ssa", ".idx", ".sbv", ".pgs", ".ttml", ".lrc"];

/// Python `PurePath.name`: the final path component as a string.
fn path_name(path: &Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

/// Python `PurePath.stem`: the final component without its last suffix.
///
/// Matches CPython's rule: the suffix split point is the last `.` that is
/// neither the first character of the name nor the final character. So
/// `movie.en.srt` -> `movie.en`, `.hidden` -> `.hidden`, `trailing.` ->
/// `trailing.`.
pub fn path_stem(path: &Path) -> &str {
    let name = path_name(path);
    match suffix_split(name) {
        Some(i) => &name[..i],
        None => name,
    }
}

/// Python `PurePath.suffix`: the last `.`-segment of the final component
/// (including the leading dot), or `""` when there is none.
pub fn path_suffix(path: &Path) -> &str {
    let name = path_name(path);
    match suffix_split(name) {
        Some(i) => &name[i..],
        None => "",
    }
}

/// Index of the suffix-introducing `.` in `name`, per CPython
/// (`0 < i < len(name) - 1`), or `None` when `name` has no suffix.
fn suffix_split(name: &str) -> Option<usize> {
    let i = name.rfind('.')?;
    if i > 0 && i < name.len() - 1 {
        Some(i)
    } else {
        None
    }
}

/// Python `PurePath.with_suffix(suffix)`: replace the final component's last
/// suffix with `suffix`, or append it when the component has no suffix.
pub fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let name = path_name(path);
    let stem = match suffix_split(name) {
        Some(i) => &name[..i],
        None => name,
    };
    let new_name = format!("{stem}{suffix}");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(new_name),
        _ => PathBuf::from(new_name),
    }
}

/// Whether `file`'s lowercased suffix is a known subtitle extension.
fn has_subtitle_extension(file: &Path) -> bool {
    let suffix = path_suffix(file).to_lowercase();
    SUBTITLE_EXTENSIONS.contains(&suffix.as_str())
}

/// Whether `stem` matches `video_stem` exactly, or up to a dot boundary
/// (`video_stem.`). The dot boundary stops `Episode 10.en.srt` matching video
/// `Episode 1`.
fn stem_matches(stem: &str, video_stem: &str) -> bool {
    stem == video_stem || stem.starts_with(&format!("{video_stem}."))
}

/// Find external subtitle files for a video.
///
/// Returns `[]` when `video_path` does not exist. Otherwise scans the parent
/// directory, keeping regular files whose lowercased suffix is in
/// [`SUBTITLE_EXTENSIONS`] and whose stem matches the video stem at a dot
/// boundary. Scan order is not contractual (Python uses unordered `iterdir()`).
pub fn get_external_subtitle_paths(video_path: &Path) -> Vec<PathBuf> {
    if !video_path.exists() {
        return Vec::new();
    }

    let video_dir = match video_path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let video_stem = path_stem(video_path);

    let entries = match std::fs::read_dir(&video_dir) {
        Ok(entries) => entries,
        // Python swallows OSError from a failed scan and returns what it has.
        Err(_) => return Vec::new(),
    };

    let mut subtitle_paths = Vec::new();
    for entry in entries.flatten() {
        let file = entry.path();
        // `is_file()` follows symlinks, matching pathlib's `Path.is_file`.
        if !file.is_file() {
            continue;
        }
        if !has_subtitle_extension(&file) {
            continue;
        }
        if stem_matches(path_stem(&file), video_stem) {
            subtitle_paths.push(file);
        }
    }

    subtitle_paths
}

/// Parse the language from a subtitle filename, relative to the video stem.
///
/// Returns [`LanguageCode::None`] when the subtitle stem does not match the
/// video stem at a dot boundary, or carries no recognizable language tag. The
/// language tag is conventionally the last `.`-segment, so the scan runs
/// reversed: `movie.no.forced.en` resolves to English (not Norwegian `no`), and
/// `movie.en.forced` resolves to English via the earlier segment once the
/// trailing `forced` fails to parse.
pub fn parse_subtitle_language(subtitle_path: &Path, video_stem: &str) -> LanguageCode {
    let subtitle_stem = path_stem(subtitle_path);
    if !stem_matches(subtitle_stem, video_stem) {
        return LanguageCode::None;
    }

    // Part after the video name, e.g. ".en" or ".subgen.medium.en" -> the
    // language segments with the leading dot(s) stripped.
    let suffix = subtitle_stem[video_stem.len()..].trim_start_matches('.');
    if suffix.is_empty() {
        return LanguageCode::None;
    }

    for part in suffix.split('.').rev() {
        let lang = LanguageCode::from_string(Some(part));
        if lang != LanguageCode::None {
            return lang;
        }
    }

    LanguageCode::None
}

/// Whether the video has an external subtitle in `language`.
///
/// When `only_subgen` is set, files whose lowercased stem lacks `"subgen"` are
/// skipped.
pub fn has_external_subtitle_language(
    video_path: &Path,
    language: LanguageCode,
    only_subgen: bool,
) -> bool {
    let video_stem = path_stem(video_path);
    for sub_path in get_external_subtitle_paths(video_path) {
        if only_subgen && !path_stem(&sub_path).to_lowercase().contains("subgen") {
            continue;
        }
        if parse_subtitle_language(&sub_path, video_stem) == language {
            return true;
        }
    }
    false
}

/// Whether the video has any external subtitle file.
pub fn has_any_external_subtitle(video_path: &Path) -> bool {
    !get_external_subtitle_paths(video_path).is_empty()
}

/// The LRC path for an audio file: `audio_path` with its last suffix replaced
/// by `.lrc` (appended when there is none). The file need not exist.
pub fn get_lrc_path(audio_path: &Path) -> PathBuf {
    with_suffix(audio_path, ".lrc")
}

/// Whether an LRC file exists for `audio_path`.
pub fn has_lrc_file(audio_path: &Path) -> bool {
    get_lrc_path(audio_path).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pathlib_stem_and_suffix() {
        assert_eq!(path_stem(Path::new("movie.en.srt")), "movie.en");
        assert_eq!(path_suffix(Path::new("movie.en.srt")), ".srt");
        assert_eq!(path_stem(Path::new("movie")), "movie");
        assert_eq!(path_suffix(Path::new("movie")), "");
        assert_eq!(path_stem(Path::new(".hidden")), ".hidden");
        assert_eq!(path_suffix(Path::new(".hidden")), "");
        assert_eq!(path_stem(Path::new("archive.tar.gz")), "archive.tar");
        assert_eq!(path_suffix(Path::new("archive.tar.gz")), ".gz");
        assert_eq!(path_stem(Path::new("trailing.")), "trailing.");
        assert_eq!(path_suffix(Path::new("trailing.")), "");
    }

    #[test]
    fn with_suffix_replaces_or_appends() {
        assert_eq!(get_lrc_path(Path::new("song.mp3")), PathBuf::from("song.lrc"));
        assert_eq!(get_lrc_path(Path::new("noext")), PathBuf::from("noext.lrc"));
        assert_eq!(
            get_lrc_path(Path::new("archive.tar.gz")),
            PathBuf::from("archive.tar.lrc")
        );
        assert_eq!(
            get_lrc_path(Path::new("/media/audio/episode.m4a")),
            PathBuf::from("/media/audio/episode.lrc")
        );
    }

    #[test]
    fn dot_boundary_rejects_prefix_collision() {
        // "Episode 10.en" must not be treated as a subtitle for "Episode 1".
        assert!(stem_matches("Episode 1.en", "Episode 1"));
        assert!(!stem_matches("Episode 10.en", "Episode 1"));
        assert!(stem_matches("movie", "movie"));
    }

    #[test]
    fn reversed_scan_prefers_trailing_language_tag() {
        // `no` (Norwegian) appears earlier but `en` is the real trailing tag.
        assert_eq!(
            parse_subtitle_language(Path::new("movie.no.forced.en.srt"), "movie"),
            LanguageCode::ENGLISH
        );
        // Trailing `forced` fails; the earlier `en` resolves English.
        assert_eq!(
            parse_subtitle_language(Path::new("movie.en.forced.srt"), "movie"),
            LanguageCode::ENGLISH
        );
        // No language segment.
        assert_eq!(
            parse_subtitle_language(Path::new("movie.srt"), "movie"),
            LanguageCode::None
        );
        // Unrelated / prefix-collision stem.
        assert_eq!(
            parse_subtitle_language(Path::new("Episode 10.en.srt"), "Episode 1"),
            LanguageCode::None
        );
    }
}
