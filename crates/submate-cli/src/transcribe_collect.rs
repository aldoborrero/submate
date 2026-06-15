//! File-collection classifier + extension display helper for `submate
//! transcribe`.
//!
//! Carved out from the broader `transcribe` command — the I/O, console output,
//! queueing, recursion/confirm, and single-file branches belong to the clap
//! wiring in `main.rs`. What lives here is the in-memory decision layer,
//! deterministic given a directory listing:
//!
//! 1. [`format_supported_extensions`] — the "Supported video/audio: ..." hint
//!    renderer: strip *all* leading dots, sort lexicographically, join with
//!    `", "`.
//! 2. [`classify_dir_entries`] — the directory-listing classifier as a pure
//!    function over a listing of relative names, returning the
//!    `(files_to_process, skipped_files)` buckets in input/iteration order.
//!
//! Sibling of the other pure-data modules (`config_show`, `translate_paths`).

use std::path::Path;

use submate_paths::{is_audio_file, is_video_file};

/// Extensions the directory scan drops silently (counted in neither bucket),
/// compared against the *lowercased* suffix.
const IGNORE_EXTENSIONS: &[&str] = &[".txt", ".jpg", ".png", ".nfo", ".srt", ".vtt"];

/// Render a set of extensions for display.
///
/// Each token has *all* leading dots stripped (so `..srt` -> `srt`), the
/// stripped tokens are sorted lexicographically (default string ordering), and
/// the result is `", "`-joined. Used to render the "Supported video: ..." /
/// "Supported audio: ..." hints over [`submate_paths::VIDEO_EXTENSIONS`] /
/// [`submate_paths::AUDIO_EXTENSIONS`].
pub fn format_supported_extensions(extensions: &[&str]) -> String {
    let mut tokens: Vec<&str> = extensions
        .iter()
        .map(|ext| ext.trim_start_matches('.'))
        .collect();
    tokens.sort_unstable();
    tokens.join(", ")
}

/// Classify a directory listing into `(files_to_process, skipped_files)`.
///
/// For each name, in iteration order:
/// * `is_video_file || is_audio_file` -> **process** (the media test wins
///   *first*, so a dotfile media file like `.hidden.mkv` is processed, not
///   ignored — its leading dot never reaches the dotfile guard);
/// * else if the raw basename does not start with `"."` **and** the lowercased
///   suffix is not in [`IGNORE_EXTENSIONS`] -> **skipped**;
/// * otherwise (dotfile, or one of the 6 ignore extensions) -> **ignored**
///   (dropped silently).
///
/// Both buckets are returned in input order; neither is sorted.
pub fn classify_dir_entries(names: &[&str]) -> (Vec<String>, Vec<String>) {
    let mut files_to_process = Vec::new();
    let mut skipped_files = Vec::new();

    for &name in names {
        if is_video_file(name) || is_audio_file(name) {
            files_to_process.push(name.to_string());
        } else if !is_dotfile(name) && !has_ignored_extension(name) {
            skipped_files.push(name.to_string());
        }
    }

    (files_to_process, skipped_files)
}

/// Whether the raw basename starts with `"."`.
///
/// The guard is on the unmodified basename — case-sensitive, before any suffix
/// lowercasing — so `.HIDDEN.TXT` is ignored via this rule regardless of its
/// extension.
fn is_dotfile(name: &str) -> bool {
    Path::new(name)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|base| base.starts_with('.'))
}

/// Whether the lowercased suffix is in [`IGNORE_EXTENSIONS`].
///
/// `Path::extension` is the final `.ext` of the last component, sans dot, or
/// none for a name without one or a leading-dot-only name; the lowercase makes
/// the match case-insensitive, so `subs.SRT` is ignored like `subs.srt`.
fn has_ignored_extension(name: &str) -> bool {
    match Path::new(name).extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let suffix = format!(".{}", ext.to_lowercase());
            IGNORE_EXTENSIONS.contains(&suffix.as_str())
        }
        None => false,
    }
}

#[cfg(test)]
mod parity {
    use super::*;
    use ::parity::golden;
    use submate_paths::{AUDIO_EXTENSIONS, VIDEO_EXTENSIONS};

    /// Fixture-driven parity falsifier for the directory classifier.
    ///
    /// `fixtures/cli/transcribe_collect_cases.json` is a list of
    /// `{names, files_to_process, skipped_files}` triples over fixed listings
    /// covering every branch (media files, a dotfile media file, each of the 6
    /// ignore extensions, a mixed-case ignore ext, unknown exts that become
    /// skipped, a dotfile whose ext would otherwise skip, plus empty and
    /// interleaved listings). For each case we drive [`classify_dir_entries`] and
    /// assert the two buckets match the golden lists in order.
    #[test]
    fn transcribe_collect_cases() {
        let cases = golden("cli/transcribe_collect_cases.json");
        let rows = cases.as_array().expect("golden is a JSON array of cases");

        for row in rows {
            let names: Vec<&str> = row["names"]
                .as_array()
                .expect("`names` is an array")
                .iter()
                .map(|v| v.as_str().expect("name is a string"))
                .collect();
            let expected_process: Vec<&str> = row["files_to_process"]
                .as_array()
                .expect("`files_to_process` is an array")
                .iter()
                .map(|v| v.as_str().expect("entry is a string"))
                .collect();
            let expected_skipped: Vec<&str> = row["skipped_files"]
                .as_array()
                .expect("`skipped_files` is an array")
                .iter()
                .map(|v| v.as_str().expect("entry is a string"))
                .collect();

            let (process, skipped) = classify_dir_entries(&names);
            assert_eq!(process, expected_process, "files_to_process for {names:?}");
            assert_eq!(skipped, expected_skipped, "skipped_files for {names:?}");
        }
    }

    /// Pin the dot-strip + sort + `", "`-join against the real extension sets.
    ///
    /// `fixtures/cli/transcribe_supported_extensions.json` holds the expected
    /// rendering for the video / audio extension sets; we render the same over
    /// [`VIDEO_EXTENSIONS`] / [`AUDIO_EXTENSIONS`] and assert equality.
    #[test]
    fn supported_extensions_match_golden() {
        let golden = golden("cli/transcribe_supported_extensions.json");

        let video = golden["video"].as_str().expect("`video` is a string");
        let audio = golden["audio"].as_str().expect("`audio` is a string");

        assert_eq!(format_supported_extensions(VIDEO_EXTENSIONS), video);
        assert_eq!(format_supported_extensions(AUDIO_EXTENSIONS), audio);
    }
}
