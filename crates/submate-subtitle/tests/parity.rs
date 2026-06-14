//! Parity tests for on-disk subtitle discovery, driven by
//! `fixtures/subtitle/discovery_cases.json` (captured from the live
//! Python `submate.subtitle` helpers).
//!
//! [`internal_probe`] additionally exercises the embedded-subtitle-stream
//! language probe against a real muxed clip, gated behind an `ffprobe`-on-PATH
//! check and the presence of the optional `subtitle/clipS.{mkv,subs.json}`
//! fixtures.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// The captured five-field view of a `LanguageCode`:
/// `[iso_639_1, iso_639_2_t, iso_639_2_b, name_en, name_native]`, each a JSON
/// string or null, matching how a language code serializes.
fn lang_tuple(value: submate_lang::LanguageCode) -> Vec<Value> {
    let opt = |s: Option<&str>| match s {
        Some(s) => Value::String(s.to_string()),
        None => Value::Null,
    };
    vec![
        opt(value.to_iso_639_1()),
        opt(value.to_iso_639_2_t()),
        opt(value.to_iso_639_2_b()),
        opt(value.name_en()),
        opt(value.name_native()),
    ]
}

/// Unique temp directory for one case, created under the system temp dir
/// without any external crate. Removed by [`TempDir::drop`].
struct TempDir(PathBuf);

impl TempDir {
    fn new(tag: &str) -> Self {
        // A monotonic counter plus pid keeps concurrent test cases distinct.
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "submate-discovery-{}-{}-{}",
            std::process::id(),
            tag,
            n
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .expect("utf-8 file name")
        .to_string()
}

/// Whether `ffprobe` is callable on `PATH`. Mirrors the binary-detection
/// posture of `submate-media`'s real-binary tests.
fn ffprobe_on_path() -> bool {
    std::process::Command::new("ffprobe")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

mod parity {
    use super::*;
    use std::collections::BTreeSet;

    use ::parity::{fixture_path, golden};
    use submate_subtitle::discovery::{
        get_external_subtitle_paths, get_internal_subtitle_languages, get_lrc_path,
        has_external_subtitle_language, has_subtitle_language, parse_subtitle_language,
    };

    #[test]
    fn discovery_fs() {
        let cases = golden("subtitle/discovery_cases.json");

        let discovery = cases["discovery"].as_object().expect("discovery object");
        for (tag, case) in discovery {
            let video_stem = case["video_stem"].as_str().expect("video_stem");
            let dir_files: Vec<&str> = case["dir_files"]
                .as_array()
                .expect("dir_files array")
                .iter()
                .map(|v| v.as_str().expect("dir file name"))
                .collect();
            let expect_external: BTreeSet<String> = case["expect_external"]
                .as_array()
                .expect("expect_external array")
                .iter()
                .map(|v| v.as_str().expect("external name").to_string())
                .collect();

            let tmp = TempDir::new(tag);
            for name in &dir_files {
                std::fs::write(tmp.path().join(name), b"").expect("write fixture file");
            }

            // The video file is the dir entry whose stem matches the video stem but
            // is not itself an expected subtitle (in every fixture case a `.mkv`).
            let video_name = dir_files
                .iter()
                .find(|name| {
                    let stem = submate_subtitle::discovery::path_stem(Path::new(name));
                    stem == video_stem && !expect_external.contains(**name)
                })
                .unwrap_or_else(|| panic!("{tag}: no video file in dir_files"));
            let video_path = tmp.path().join(video_name);

            // Set-equality on the discovered external subtitle file names.
            let actual: BTreeSet<String> = get_external_subtitle_paths(&video_path)
                .iter()
                .map(|p| file_name(p))
                .collect();
            assert_eq!(actual, expect_external, "{tag}: external subtitle set");

            // Exact language tuple per parsed subtitle filename.
            let expect_parse = case["expect_parse"]
                .as_object()
                .expect("expect_parse object");
            for (sub_name, expected) in expect_parse {
                let sub_path = tmp.path().join(sub_name);
                let lang = parse_subtitle_language(&sub_path, video_stem);
                let expected: Vec<Value> = expected.as_array().expect("lang tuple").clone();
                assert_eq!(
                    lang_tuple(lang),
                    expected,
                    "{tag}: parse_subtitle_language({sub_name:?})"
                );
            }
        }
    }

    #[test]
    fn lrc_paths() {
        let cases = golden("subtitle/discovery_cases.json");
        let lrc = cases["lrc"].as_object().expect("lrc object");
        for (audio, expected) in lrc {
            let expected = expected.as_str().expect("lrc path string");
            assert_eq!(
                get_lrc_path(Path::new(audio)),
                PathBuf::from(expected),
                "get_lrc_path({audio:?})"
            );
        }
    }

    /// Probe the real embedded-subtitle fixture and assert the returned
    /// `LanguageCode` list **exactly** equals the golden captured from the Python
    /// `get_internal_subtitle_languages` reference run, then check the
    /// internal-vs-external / `only_subgen` matrix of `has_subtitle_language`.
    ///
    /// Skipped (passes as a no-op) when `ffprobe` is absent or the optional
    /// `subtitle/clipS.{mkv,subs.json}` fixtures are missing — the test arms
    /// itself the moment those fixtures are present. The golden
    /// stores each stream's language as the five-field
    /// `[iso_639_1, iso_639_2_t, iso_639_2_b, name_en, name_native]` view, matching
    /// how the discovery golden serializes a `LanguageCode`.
    #[test]
    fn internal_probe() {
        if !ffprobe_on_path() {
            eprintln!("skipping internal_probe: ffprobe not available on PATH");
            return;
        }

        let clip = fixture_path("subtitle/clipS.mkv");
        let golden_path = fixture_path("subtitle/clipS.subs.json");
        if !clip.exists() || !golden_path.exists() {
            eprintln!(
                "skipping internal_probe: optional fixture fixtures/subtitle/clipS.{{mkv,subs.json}} is absent"
            );
            return;
        }

        let golden_bytes = std::fs::read(&golden_path).expect("read clipS.subs.json");
        let expected: Vec<Vec<Value>> = serde_json::from_slice(&golden_bytes)
            .expect("clipS.subs.json is a list of language tuples");

        let actual: Vec<Vec<Value>> = get_internal_subtitle_languages(&clip)
            .into_iter()
            .map(lang_tuple)
            .collect();
        assert_eq!(
            actual, expected,
            "internal subtitle languages must match golden subtitle/clipS.subs.json",
        );

        // The fixture is the documented two-tagged (eng/spa) + one-untagged clip,
        // so the internal half answers English/Spanish but not, say, German.
        assert!(
            has_subtitle_language(&clip, submate_lang::LanguageCode::ENGLISH, false),
            "internal English track must satisfy has_subtitle_language",
        );
        assert!(
            has_subtitle_language(&clip, submate_lang::LanguageCode::SPANISH, false),
            "internal Spanish track must satisfy has_subtitle_language",
        );
        assert!(
            !has_subtitle_language(&clip, submate_lang::LanguageCode::GERMAN, false),
            "absent German track must not satisfy has_subtitle_language",
        );

        // only_subgen=true skips the internal half entirely; with no external
        // subgen files next to the fixture clip the combinator reduces to the
        // external predicate (false here), never to the internal English match.
        assert_eq!(
            has_subtitle_language(&clip, submate_lang::LanguageCode::ENGLISH, true),
            has_external_subtitle_language(&clip, submate_lang::LanguageCode::ENGLISH, true),
            "only_subgen=true must defer entirely to the external half",
        );
    }
}
