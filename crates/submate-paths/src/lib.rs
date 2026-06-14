//! Path building and path-mapping.
//!
//! Pure string/path logic with no I/O: subtitle-path construction, the Docker
//! host/container path-mapping translation, and video/audio extension checks.
//! Output strings are kept byte-for-byte identical to the Python originals,
//! which is why the filename assembly works on string parts rather than going
//! through `Utf8Path` joins (Python's `Path(".")` parent must not leak a `./`).

use camino::Utf8Path;
use submate_lang::LanguageCode;
use submate_types::LanguageNamingType;

/// Video container extensions (lowercased, dot-prefixed), mirroring
/// `submate.paths.VIDEO_EXTENSIONS`.
pub const VIDEO_EXTENSIONS: &[&str] = &[
    ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".mpg", ".mpeg", ".3gp",
    ".ogv",
];

/// Audio container extensions (lowercased, dot-prefixed), mirroring
/// `submate.paths.AUDIO_EXTENSIONS`.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    ".mp3", ".flac", ".aac", ".m4a", ".wav", ".ogg", ".opus", ".wma", ".alac", ".ape",
];

/// Map a path from one location prefix to another.
///
/// Useful for Docker containers where the host path differs from the container
/// path. When `use_mapping` is false (or either prefix is empty) the path is
/// returned unchanged; otherwise a leading `path_from` is replaced once with
/// `path_to`. Ports `submate.paths.map_path`.
pub fn map_path(path: &str, use_mapping: bool, path_from: &str, path_to: &str) -> String {
    if !use_mapping {
        return path.to_string();
    }
    if path_from.is_empty() || path_to.is_empty() {
        return path.to_string();
    }
    if let Some(rest) = path.strip_prefix(path_from) {
        return format!("{path_to}{rest}");
    }
    path.to_string()
}

/// Format a language code according to `naming_type`.
///
/// Returns an empty string when `language` is `None`/empty, and falls back to
/// the original string when it cannot be parsed into a known [`LanguageCode`].
/// Ports `submate.paths.format_language_for_filename`.
pub fn format_language_for_filename(
    language: Option<&str>,
    naming_type: LanguageNamingType,
) -> String {
    let raw = match language {
        Some(s) if !s.is_empty() => s,
        _ => return String::new(),
    };

    let lang_code = LanguageCode::from_string(Some(raw));
    if lang_code == LanguageCode::None {
        // Fall back to the original string if we can't parse it.
        return raw.to_string();
    }

    format_language_code(lang_code, naming_type)
}

/// Like [`format_language_for_filename`] but starting from an already-resolved
/// [`LanguageCode`]. `None` formats to the empty string.
pub fn format_language_code_for_filename(
    language: LanguageCode,
    naming_type: LanguageNamingType,
) -> String {
    if language == LanguageCode::None {
        return String::new();
    }
    format_language_code(language, naming_type)
}

fn format_language_code(lang_code: LanguageCode, naming_type: LanguageNamingType) -> String {
    let formatted = match naming_type {
        LanguageNamingType::Iso6391 => lang_code.to_iso_639_1(),
        LanguageNamingType::Iso6392T => lang_code.to_iso_639_2_t(),
        LanguageNamingType::Iso6392B => lang_code.to_iso_639_2_b(),
        LanguageNamingType::Name => lang_code.to_name(true),
        LanguageNamingType::Native => lang_code.to_name(false),
    };
    formatted.unwrap_or("").to_string()
}

/// Options controlling [`build_subtitle_path`] naming, matching the keyword
/// arguments of the Python `build_subtitle_path`.
pub struct SubtitleNaming<'a> {
    /// How to format the language suffix.
    pub naming_type: LanguageNamingType,
    /// Insert a `.subgen` marker before the language.
    pub include_subgen_marker: bool,
    /// Insert the model name (when non-empty) before the language.
    pub include_model: bool,
    /// Whisper model name used when `include_model` is set.
    pub model_name: &'a str,
    /// Subtitle file extension; a leading dot is added if missing.
    pub extension: &'a str,
}

impl Default for SubtitleNaming<'_> {
    fn default() -> Self {
        Self {
            naming_type: LanguageNamingType::Iso6392B,
            include_subgen_marker: false,
            include_model: false,
            model_name: "",
            extension: ".srt",
        }
    }
}

/// Build a subtitle file path with configurable naming options.
///
/// The filename is `<stem>[.subgen][.<model>][.<lang>]<ext>`, placed next to
/// `video_path`. Ports `submate.paths.build_subtitle_path`; see the Python
/// docstring for examples. `language` is a raw code string (e.g. `"eng"`).
pub fn build_subtitle_path(
    video_path: &str,
    language: Option<&str>,
    naming: &SubtitleNaming<'_>,
) -> String {
    let formatted_lang = format_language_for_filename(language, naming.naming_type);
    assemble_subtitle_path(video_path, &formatted_lang, naming)
}

/// Build a subtitle path from an already-resolved [`LanguageCode`].
pub fn build_subtitle_path_with_code(
    video_path: &str,
    language: LanguageCode,
    naming: &SubtitleNaming<'_>,
) -> String {
    let formatted_lang = format_language_code_for_filename(language, naming.naming_type);
    assemble_subtitle_path(video_path, &formatted_lang, naming)
}

fn assemble_subtitle_path(
    video_path: &str,
    formatted_lang: &str,
    naming: &SubtitleNaming<'_>,
) -> String {
    // `Path.stem`: the final component without its last suffix. Camino's
    // `file_stem` matches Python here (e.g. `show.s01e01` for `show.s01e01.mkv`).
    let stem = Utf8Path::new(video_path).file_stem().unwrap_or("");

    let mut parts: Vec<&str> = vec![stem];

    if naming.include_subgen_marker {
        parts.push("subgen");
    }

    if naming.include_model && !naming.model_name.is_empty() {
        parts.push(naming.model_name);
    }

    if !formatted_lang.is_empty() {
        parts.push(formatted_lang);
    }

    let mut extension = naming.extension.to_string();
    if !extension.starts_with('.') {
        extension = format!(".{extension}");
    }

    let subtitle_name = format!("{}{}", parts.join("."), extension);

    join_parent(video_path, &subtitle_name)
}

/// Join a new filename onto the parent directory of `video_path`, matching
/// `str(Path(video_path).parent / name)` exactly. When `video_path` has no
/// directory component, Python's parent is `.` and `str(Path(".") / name)` is
/// just `name`, so no `./` prefix is added.
fn join_parent(video_path: &str, name: &str) -> String {
    // pathlib drops "." (current-dir) and empty (double-slash) components when a
    // PurePosixPath is constructed; camino's `parent()` keeps them, so a leading
    // "./" leaks into the output. Normalize the parent the way pathlib does —
    // keep "..", keep the absolute root, drop "." and empties — before joining.
    let parent = Utf8Path::new(video_path)
        .parent()
        .map_or("", Utf8Path::as_str);
    let absolute = parent.starts_with('/');
    let parts: Vec<&str> = parent
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect();
    match (absolute, parts.is_empty()) {
        (true, true) => format!("/{name}"), // parent is "/"
        (true, false) => format!("/{}/{}", parts.join("/"), name),
        (false, true) => name.to_string(), // parent ".", "", "./."
        (false, false) => format!("{}/{}", parts.join("/"), name),
    }
}

/// Generate the subtitle path for a video using default naming. Ports
/// `submate.paths.get_subtitle_path`.
pub fn get_subtitle_path(video_path: &str, language: &str) -> String {
    let language = if language.is_empty() {
        None
    } else {
        Some(language)
    };
    build_subtitle_path(video_path, language, &SubtitleNaming::default())
}

/// Whether `path` has a known video extension (case-insensitive on the
/// extension). Ports `submate.paths.is_video_file`.
pub fn is_video_file(path: &str) -> bool {
    has_extension(path, VIDEO_EXTENSIONS)
}

/// Whether `path` has a known audio extension (case-insensitive on the
/// extension). Ports `submate.paths.is_audio_file`.
pub fn is_audio_file(path: &str) -> bool {
    has_extension(path, AUDIO_EXTENSIONS)
}

fn has_extension(path: &str, extensions: &[&str]) -> bool {
    // Python uses `Path(path).suffix.lower()`, i.e. the final `.ext` of the
    // last component, including its leading dot, lowercased.
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

    #[test]
    fn map_path_rules() {
        assert_eq!(
            map_path("/host/m.mkv", false, "/host", "/data"),
            "/host/m.mkv"
        );
        assert_eq!(
            map_path("/host/m.mkv", true, "/host", "/data"),
            "/data/m.mkv"
        );
        assert_eq!(
            map_path("/other/m.mkv", true, "/host", "/data"),
            "/other/m.mkv"
        );
        assert_eq!(map_path("/host/m.mkv", true, "", "/data"), "/host/m.mkv");
    }

    #[test]
    fn unparseable_language_falls_back() {
        let out = build_subtitle_path("movie.mp4", Some("und"), &SubtitleNaming::default());
        assert_eq!(out, "movie.und.srt");
    }
}
