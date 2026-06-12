//! Pure filename/language helpers ported from
//! `submate/cli/commands/translate.py`.
//!
//! These three functions decide *which* files to translate, *what* source
//! language to assume, and *where* to write output. They are pure-data and
//! depend only on the already-ported [`submate_lang`] table — no clap, no IO.
//!
//! Python semantics that matter here come from `pathlib.PurePath`:
//!
//! * `suffix` — the trailing `.`-segment of the final path component,
//!   *including* the leading dot. A leading-dot name with no stem (e.g.
//!   `.srt`) has an empty suffix. `movie.tar.gz` has suffix `.gz`.
//! * `stem` — the final component without its `suffix`. For `movie.fr.srt`
//!   the stem is `movie.fr`; for `.srt` the stem is `.srt`.
//! * `parent` — the path minus its final component.
//!
//! We replicate these directly on the component string rather than leaning on
//! `std::path` extension rules, so the byte-for-byte contract is explicit.

use std::path::{Path, PathBuf};

use submate_lang::LanguageCode;

/// Subtitle file extensions recognized by `submate translate`.
///
/// Mirrors the module constant `SUBTITLE_EXTENSIONS` in `translate.py`. Values
/// are lowercase and include the leading dot, matching `Path.suffix`.
const SUBTITLE_EXTENSIONS: [&str; 4] = [".srt", ".vtt", ".ass", ".ssa"];

/// Final path component (`Path.name`), or `""` if there is none.
fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("")
}

/// Python `PurePath.suffix` for the final component: the substring from the
/// last `.` to the end, *including* the dot. Empty when the name has no
/// interior dot — a leading dot (dotfile like `.srt`) does not count.
fn suffix(path: &Path) -> String {
    let name = file_name(path);
    match name.rfind('.') {
        // A dot at index 0 (dotfile, no stem) yields no suffix, matching
        // pathlib. Otherwise the suffix runs from the dot to the end.
        Some(idx) if idx > 0 => name[idx..].to_string(),
        _ => String::new(),
    }
}

/// Python `PurePath.stem` for the final component: the name with its
/// [`suffix`] removed. For `.srt` (suffixless dotfile) the stem is the whole
/// name, matching pathlib.
fn stem(path: &Path) -> String {
    let name = file_name(path);
    let suf = suffix(path);
    name[..name.len() - suf.len()].to_string()
}

/// `path.suffix.lower() in SUBTITLE_EXTENSIONS`.
///
/// `movie.SRT` matches (case-folded), `movie.tar.gz` does not (`.gz`), and a
/// dotfile like `.srt` with no stem has an empty suffix and does not match.
pub fn is_subtitle_file(path: &Path) -> bool {
    let suf = suffix(path).to_lowercase();
    SUBTITLE_EXTENSIONS.contains(&suf.as_str())
}

/// Resolve the source language for a subtitle file.
///
/// An explicit `source_lang` (anything other than `"auto"`) wins unchanged.
/// Otherwise, when the stem has an interior dot, the last dotted stem segment
/// is taken as a candidate and returned *only* if it is a recognized language
/// code; non-language tokens (`v2`, `01`, ...) fall back to `"en"` so garbage
/// is never handed to the translator as a source language.
pub fn detect_source_language(file: &Path, source_lang: &str) -> String {
    if source_lang != "auto" {
        return source_lang.to_string();
    }

    let file_stem = stem(file);
    if file_stem.contains('.') {
        let candidate = file_stem.rsplit('.').next().unwrap_or("");
        if LanguageCode::from_string(Some(candidate)) != LanguageCode::None {
            return candidate.to_string();
        }
    }

    "en".to_string()
}

/// Derive the default output path for a translated subtitle.
///
/// Strips any trailing dotted stem segment (an existing language suffix such
/// as `.en`, but also non-language ones like `.v2` — matching Python exactly)
/// and rebuilds `parent / "{base}.{target_lang}{suffix}"`, preserving the
/// original extension and its case.
///
/// `movie.srt` + `es` -> `movie.es.srt`; `movie.en.srt` + `es` ->
/// `movie.es.srt`; `movie.v2.srt` + `es` -> `movie.es.srt`.
///
/// This is the default-naming branch only; the explicit `--output` override
/// is trivial IO handled by the caller.
pub fn output_path(file: &Path, target_lang: &str) -> PathBuf {
    let file_stem = stem(file);
    let base = match file_stem.rsplit_once('.') {
        Some((head, _)) => head.to_string(),
        None => file_stem,
    };

    let new_name = format!("{base}.{target_lang}{}", suffix(file));
    match file.parent() {
        Some(parent) => parent.join(new_name),
        None => PathBuf::from(new_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Golden rows mirror the case set mandated by the backlog item. Once the
    // capture pre-pass lands `rust/fixtures/cli/translate_filename_cases.json`,
    // a fixture-driven `assert_json_eq` parity test should replace these
    // hardcoded expectations; until then these assert the Python contract
    // directly from spec.

    #[test]
    fn is_subtitle_file_matches_python_suffix_rules() {
        assert!(is_subtitle_file(Path::new("movie.srt")));
        assert!(is_subtitle_file(Path::new("movie.SRT")), "case-folded");
        assert!(is_subtitle_file(Path::new("movie.vtt")));
        assert!(is_subtitle_file(Path::new("movie.ass")));
        assert!(is_subtitle_file(Path::new("movie.ssa")));
        assert!(!is_subtitle_file(Path::new("movie.tar.gz")), "only last suffix");
        assert!(!is_subtitle_file(Path::new("movie.txt")));
        assert!(!is_subtitle_file(Path::new(".srt")), "suffixless dotfile");
        assert!(!is_subtitle_file(Path::new("movie")));
    }

    #[test]
    fn detect_source_language_auto_accepts_valid_tag() {
        assert_eq!(detect_source_language(Path::new("movie.fr.srt"), "auto"), "fr");
    }

    #[test]
    fn detect_source_language_auto_rejects_non_language_token() {
        assert_eq!(detect_source_language(Path::new("movie.v2.srt"), "auto"), "en");
        assert_eq!(detect_source_language(Path::new("episode.01.srt"), "auto"), "en");
    }

    #[test]
    fn detect_source_language_no_dotted_stem_falls_back() {
        assert_eq!(detect_source_language(Path::new("movie.srt"), "auto"), "en");
    }

    #[test]
    fn detect_source_language_explicit_wins() {
        assert_eq!(detect_source_language(Path::new("movie.fr.srt"), "es"), "es");
        // Explicit value is returned unchanged even if not a language tag.
        assert_eq!(detect_source_language(Path::new("movie.srt"), "xx"), "xx");
    }

    #[test]
    fn output_path_appends_when_no_existing_segment() {
        assert_eq!(output_path(Path::new("movie.srt"), "es"), PathBuf::from("movie.es.srt"));
    }

    #[test]
    fn output_path_replaces_existing_language_segment() {
        assert_eq!(output_path(Path::new("movie.en.srt"), "es"), PathBuf::from("movie.es.srt"));
    }

    #[test]
    fn output_path_replaces_any_trailing_segment() {
        // Non-language trailing segments are stripped too, matching Python.
        assert_eq!(output_path(Path::new("movie.v2.srt"), "es"), PathBuf::from("movie.es.srt"));
    }

    #[test]
    fn output_path_preserves_parent_and_extension_case() {
        assert_eq!(
            output_path(Path::new("subs/movie.SRT"), "es"),
            PathBuf::from("subs/movie.es.SRT"),
        );
    }
}
