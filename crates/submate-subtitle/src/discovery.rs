//! On-disk subtitle discovery, filename-language parsing, and the
//! internal-track language probe.
//!
//! The subtitle-discovery half: directory scanning, the
//! dot-boundary stem match, the reversed filename-language scan, the LRC path
//! helpers, plus the embedded-subtitle-stream language probe and the
//! internal-OR-external combinator. The filesystem helpers depend only on
//! `std` plus the already-ported [`submate_lang::LanguageCode`] table; the
//! internal probe shells out to `ffprobe` (the convention `submate-media`'s
//! audio-track probe already uses) instead of PyAV.
//!
//! Filename component semantics (`stem`/`suffix`/`with_suffix`) mirror Python
//! `pathlib.PurePath` exactly, so they agree with the conventions the rest of
//! the port (e.g. `submate-paths`) relies on. The implementation lives here
//! rather than importing `camino` to keep this crate on `std` only.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use submate_lang::LanguageCode;

/// Subtitle file extensions used for on-disk discovery (lowercased, dot
/// prefixed). Mirrors `submate.subtitle.SUBTITLE_EXTENSIONS` exactly — this is
/// the WIDE discovery set, distinct from the narrower translate-path set in
/// `cli/commands/translate.py`.
pub const SUBTITLE_EXTENSIONS: &[&str] = &[
    ".srt", ".vtt", ".sub", ".ass", ".ssa", ".idx", ".sbv", ".pgs", ".ttml", ".lrc",
];

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

    let Ok(entries) = std::fs::read_dir(&video_dir) else {
        // Python swallows OSError from a failed scan and returns what it has.
        return Vec::new();
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

/// Top-level shape of `ffprobe -show_streams -of json`: only the `streams`
/// array is consumed; every other key is ignored.
#[derive(Debug, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<RawStream>,
}

/// One ffprobe stream entry. Only the codec type (to keep subtitle streams)
/// and the `tags.language` tag are read; everything else is ignored.
#[derive(Debug, Deserialize)]
struct RawStream {
    codec_type: Option<String>,
    #[serde(default)]
    tags: StreamTags,
}

/// The `tags` object of a stream. An absent `tags` object deserializes to the
/// default (no language), matching Python's `stream.metadata.get(...)` on an
/// empty mapping.
#[derive(Debug, Default, Deserialize)]
struct StreamTags {
    language: Option<String>,
}

/// Parse the JSON payload of `ffprobe -show_streams -of json` into the language
/// of each subtitle stream, in stream order.
///
/// Split out from [`get_internal_subtitle_languages`] so the stream-filtering
/// and language-mapping logic is testable without invoking `ffprobe`. Returns
/// `None` when the JSON cannot be parsed, so the caller can fold that into the
/// swallow-all-errors empty fallback.
///
/// Each subtitle stream's `tags.language` is mapped through
/// [`LanguageCode::from_iso_639_2`], which already yields [`LanguageCode::None`]
/// for an absent, empty, or unmappable tag — mirroring the Python
/// `from_iso_639_2(lang_code) or LanguageCode.NONE`.
fn parse_internal_subtitle_languages(json: &str) -> Option<Vec<LanguageCode>> {
    let probe: ProbeOutput = serde_json::from_str(json).ok()?;
    Some(
        probe
            .streams
            .into_iter()
            .filter(|stream| stream.codec_type.as_deref() == Some("subtitle"))
            .map(|stream| LanguageCode::from_iso_639_2(stream.tags.language.as_deref()))
            .collect(),
    )
}

/// Run `ffprobe -show_streams -of json <path>` and capture its stdout.
///
/// Returns `None` on any failure (binary missing, spawn error, non-zero exit,
/// non-UTF-8 output) so the public probe can fold every error into the empty
/// fallback. Uses a synchronous `std::process::Command` to keep the subtitle
/// crate off an async runtime — the Python helper is synchronous and the
/// downstream queue skip-decision calls it as a plain predicate.
fn run_ffprobe_streams(file_path: &Path) -> Option<String> {
    let output = std::process::Command::new("ffprobe")
        .args(["-show_streams", "-of", "json"])
        .arg(file_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// Language of every embedded (internal) subtitle stream, in stream order.
///
/// Ports `get_internal_subtitle_languages`. Python
/// opens the file with PyAV and reads each `stream.metadata["language"]` for
/// `stream.type == "subtitle"`; the port shells out to `ffprobe` instead,
/// filtering `codec_type == "subtitle"` and mapping `tags.language` the same
/// way. Stream order is contractual — one [`LanguageCode`] per subtitle stream.
///
/// Every error (missing file, missing `ffprobe`, demux/parse failure) is
/// swallowed and yields `[]`, mirroring the Python blanket
/// `except Exception: return []`.
pub fn get_internal_subtitle_languages(file_path: &Path) -> Vec<LanguageCode> {
    run_ffprobe_streams(file_path)
        .and_then(|json| parse_internal_subtitle_languages(&json))
        .unwrap_or_default()
}

/// Whether the video has an internal subtitle stream in `language`.
///
/// Ports `has_internal_subtitle_language` — `language in
/// get_internal_subtitle_languages(video_path)`.
pub fn has_internal_subtitle_language(video_path: &Path, language: LanguageCode) -> bool {
    get_internal_subtitle_languages(video_path).contains(&language)
}

/// Whether the video has any internal (embedded) subtitle stream.
///
/// Ports `has_any_internal_subtitle` —
/// `len(get_internal_subtitle_languages(video_path)) > 0`.
pub fn has_any_internal_subtitle(video_path: &Path) -> bool {
    !get_internal_subtitle_languages(video_path).is_empty()
}

/// Whether the video has a subtitle in `language`, internal OR external.
///
/// Ports `has_subtitle_language`: the predicate the
/// queue skip decision calls. Internal streams are checked first, but **only
/// when** `!only_subgen` (internal tracks can never be "subgen"); then the
/// external half ([`has_external_subtitle_language`]) is consulted with the
/// same `only_subgen` flag.
pub fn has_subtitle_language(video_path: &Path, language: LanguageCode, only_subgen: bool) -> bool {
    if !only_subgen && has_internal_subtitle_language(video_path, language) {
        return true;
    }
    has_external_subtitle_language(video_path, language, only_subgen)
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
        assert_eq!(
            get_lrc_path(Path::new("song.mp3")),
            PathBuf::from("song.lrc")
        );
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

    /// Representative `ffprobe -show_streams -of json` output: two tagged
    /// subtitle streams (`eng`, `spa`), one untagged subtitle stream, plus a
    /// video and an audio stream that must be filtered out. Exercises the
    /// parser without invoking `ffprobe`; the non-read keys ffprobe emits per
    /// stream are trimmed to what the parser consumes.
    const SAMPLE_STREAMS_JSON: &str = r#"{
        "streams": [
            { "index": 0, "codec_type": "video", "codec_name": "h264" },
            { "index": 1, "codec_type": "audio", "tags": { "language": "eng" } },
            { "index": 2, "codec_type": "subtitle", "tags": { "language": "eng" } },
            { "index": 3, "codec_type": "subtitle", "tags": { "language": "spa" } },
            { "index": 4, "codec_type": "subtitle" }
        ]
    }"#;

    #[test]
    fn internal_probe_keeps_subtitle_streams_in_order() {
        assert_eq!(
            parse_internal_subtitle_languages(SAMPLE_STREAMS_JSON),
            Some(vec![
                LanguageCode::ENGLISH,
                LanguageCode::SPANISH,
                LanguageCode::None,
            ]),
        );
    }

    #[test]
    fn internal_probe_maps_unmappable_and_missing_tag_to_none() {
        // Empty tag, garbage tag, and absent `tags` all collapse to None,
        // matching `from_iso_639_2(...) or LanguageCode.NONE`.
        let json = r#"{
            "streams": [
                { "codec_type": "subtitle", "tags": { "language": "" } },
                { "codec_type": "subtitle", "tags": { "language": "zzz" } },
                { "codec_type": "subtitle" }
            ]
        }"#;
        assert_eq!(
            parse_internal_subtitle_languages(json),
            Some(vec![
                LanguageCode::None,
                LanguageCode::None,
                LanguageCode::None
            ]),
        );
    }

    #[test]
    fn internal_probe_handles_no_subtitle_streams() {
        let json = r#"{ "streams": [ { "codec_type": "audio" } ] }"#;
        assert_eq!(parse_internal_subtitle_languages(json), Some(Vec::new()));
        assert_eq!(parse_internal_subtitle_languages("{}"), Some(Vec::new()));
    }

    #[test]
    fn internal_probe_rejects_invalid_json() {
        assert_eq!(parse_internal_subtitle_languages("not json"), None);
    }

    #[test]
    fn get_internal_subtitle_languages_swallows_missing_file() {
        // No ffprobe stdout for a nonexistent path -> empty list, never panics.
        let missing = Path::new("/nonexistent/submate-subtitle/does-not-exist.mkv");
        assert!(get_internal_subtitle_languages(missing).is_empty());
        assert!(!has_any_internal_subtitle(missing));
        assert!(!has_internal_subtitle_language(
            missing,
            LanguageCode::ENGLISH
        ));
    }

    #[test]
    fn combinator_skips_internal_when_only_subgen() {
        // With no real media, internal probing yields []; the combinator must
        // not panic and must defer entirely to the external half. A missing
        // video also has no external subs, so both branches are false.
        let missing = Path::new("/nonexistent/submate-subtitle/clip.mkv");
        assert!(!has_subtitle_language(
            missing,
            LanguageCode::ENGLISH,
            false
        ));
        assert!(!has_subtitle_language(missing, LanguageCode::ENGLISH, true));
    }
}
