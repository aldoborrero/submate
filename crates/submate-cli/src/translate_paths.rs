//! Pure filename/language helpers for `submate translate`.
//!
//! These three functions decide *which* files to translate, *what* source
//! language to assume, and *where* to write output. They are pure-data and
//! depend only on the [`submate_lang`] table — no clap, no IO.
//!
//! The path semantics that matter here:
//!
//! * `suffix` — the trailing `.`-segment of the final path component,
//!   *including* the leading dot. A leading-dot name with no stem (e.g.
//!   `.srt`) has an empty suffix. `movie.tar.gz` has suffix `.gz`.
//! * `stem` — the final component without its `suffix`. For `movie.fr.srt`
//!   the stem is `movie.fr`; for `.srt` the stem is `.srt`.
//! * `parent` — the path minus its final component.
//!
//! We replicate these directly on the component string rather than leaning on
//! `std::path` extension rules, so the contract is explicit.

use std::path::{Path, PathBuf};

use submate_lang::LanguageCode;

/// Subtitle file extensions recognized by `submate translate`.
///
/// Values are lowercase and include the leading dot, matching `Path.suffix`.
const SUBTITLE_EXTENSIONS: [&str; 4] = [".srt", ".vtt", ".ass", ".ssa"];

/// Final path component (`Path.name`), or `""` if there is none.
fn file_name(path: &Path) -> &str {
    path.file_name().and_then(|s| s.to_str()).unwrap_or("")
}

/// The `suffix` of the final component: the substring from the last `.` to the
/// end, *including* the dot. Empty when the name has no interior dot — a leading
/// dot (dotfile like `.srt`) does not count.
fn suffix(path: &Path) -> String {
    let name = file_name(path);
    match name.rfind('.') {
        // A dot at index 0 (dotfile, no stem) yields no suffix. Otherwise the
        // suffix runs from the dot to the end.
        Some(idx) if idx > 0 => name[idx..].to_string(),
        _ => String::new(),
    }
}

/// The `stem` of the final component: the name with its [`suffix`] removed. For
/// `.srt` (suffixless dotfile) the stem is the whole name.
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
/// as `.en`, but also non-language ones like `.v2`) and rebuilds
/// `parent / "{base}.{target_lang}{suffix}"`, preserving the original extension
/// and its case.
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
mod parity {
    use super::*;

    use ::fixtures::{assert_json_eq, golden};
    use serde_json::{Value, json};

    /// Fixture-driven parity falsifier.
    ///
    /// `fixtures/cli/translate_filename_cases.json` is a list of
    /// `{file, source_lang, target_lang, is_subtitle, detected_source,
    /// output_path}` rows over the mandated case set. For each row we drive
    /// [`is_subtitle_file`], [`detect_source_language`], and [`output_path`] over
    /// the captured inputs, rebuild the full row from the outputs, and assert it
    /// equals the golden row exactly. A drift in suffix case-folding,
    /// non-language-token rejection, or trailing-segment replacement fails here.
    #[test]
    fn translate_filename_cases() {
        let cases = golden("cli/translate_filename_cases.json");
        let rows = cases.as_array().expect("golden is a JSON array of rows");

        for row in rows {
            let file_str = row["file"].as_str().expect("`file` is a string");
            let source_lang = row["source_lang"]
                .as_str()
                .expect("`source_lang` is a string");
            let target_lang = row["target_lang"]
                .as_str()
                .expect("`target_lang` is a string");
            let file = Path::new(file_str);

            let output = output_path(file, target_lang);
            let output_str = output.to_str().expect("output path is valid UTF-8");

            let actual: Value = json!({
                "file": file_str,
                "source_lang": source_lang,
                "target_lang": target_lang,
                "is_subtitle": is_subtitle_file(file),
                "detected_source": detect_source_language(file, source_lang),
                "output_path": output_str,
            });

            assert_json_eq(&actual, row);
        }
    }
}
