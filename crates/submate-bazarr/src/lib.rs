//! Bazarr provider glue.
//!
//! Bazarr posts raw s16le (signed 16-bit little-endian), mono, 16 kHz PCM with
//! no container. [`pcm_s16le_to_f32`] is the boundary decode that turns those
//! bytes into the mono f32 samples whisper-rs expects.
//!
//! It also hosts the Bazarr detect-language naming: [`detect_language`] turns a
//! raw Whisper language code into the `{detected_language, language_code}` pair
//! the detect-language endpoint returns, via the deliberately narrow
//! [`LANGUAGE_NAMES`] table (NOT the broader `submate-lang` `name_en` table).

/// Canonical PCM WAV header length.
const WAV_HEADER_LEN: usize = 44;

/// Decode raw s16le PCM (or a canonical-WAV-wrapped clip) into mono f32 samples.
///
/// Bazarr posts s16le / mono / 16 kHz PCM (`encode=false`); whisper-rs's
/// `transcribe_pcm` takes `Vec<f32>` in `-1.0..=1.0`, so this is the decode that
/// bridges them — the only place the synchronous Bazarr path converts samples,
/// so it must be byte-exact (token-set tolerance applies to transcription
/// *output*, never to the sample decode feeding it).
///
/// Each little-endian `i16` is divided by `32768.0` — the standard s16→float
/// scale (`i16::MIN / 32768 == -1.0`, `i16::MAX / 32768 == 32767/32768`). A
/// trailing odd byte (an incomplete final sample) is dropped (`chunks_exact(2)`).
/// If `bytes` begins with `b"RIFF"` — a clip wrapped in a canonical WAV/RIFF
/// container — the 44-byte header is skipped first.
pub fn pcm_s16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    let pcm = if bytes.starts_with(b"RIFF") && bytes.len() >= WAV_HEADER_LEN {
        &bytes[WAV_HEADER_LEN..]
    } else {
        bytes
    };
    pcm.chunks_exact(2)
        .map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0)
        .collect()
}

/// The detected-language placeholder for a missing/empty detection.
///
/// An empty or absent Whisper language collapses to `"und"`.
pub const UNDETERMINED_CODE: &str = "und";

/// The display name for any code outside [`LANGUAGE_NAMES`].
///
/// Note `"und"` itself is NOT a key, so a no-detection result names to
/// `"Unknown"`.
pub const UNKNOWN_NAME: &str = "Unknown";

/// The deliberately NARROW Bazarr language-code → display-name table (the
/// `en..uk` set).
///
/// This is the narrow Bazarr language-name set (`en`..`uk`) — NOT the broader
/// `submate-lang` `name_en` table. Bazarr's detect-language response is keyed
/// off this exact set: any code outside it (including valid ISO-639-1 codes the
/// full table *would* name, e.g. `ca`/`be`/`fa`, and `"und"` itself) must name
/// to [`UNKNOWN_NAME`]. Routing through `submate-lang` would name those and
/// silently diverge the wire contract, so the table is intentionally not
/// derived from it.
const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("en", "English"),
    ("es", "Spanish"),
    ("fr", "French"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("ru", "Russian"),
    ("ja", "Japanese"),
    ("zh", "Chinese"),
    ("ko", "Korean"),
    ("ar", "Arabic"),
    ("hi", "Hindi"),
    ("nl", "Dutch"),
    ("pl", "Polish"),
    ("tr", "Turkish"),
    ("vi", "Vietnamese"),
    ("th", "Thai"),
    ("sv", "Swedish"),
    ("da", "Danish"),
    ("fi", "Finnish"),
    ("no", "Norwegian"),
    ("cs", "Czech"),
    ("el", "Greek"),
    ("he", "Hebrew"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("ms", "Malay"),
    ("ro", "Romanian"),
    ("sk", "Slovak"),
    ("uk", "Ukrainian"),
];

/// Normalize a Whisper-detected language code.
///
/// `None` and `Some("")` both collapse to [`UNDETERMINED_CODE`]; any non-empty
/// code passes through unchanged.
pub fn normalize_detected_code(whisper_lang: Option<&str>) -> String {
    match whisper_lang {
        Some(code) if !code.is_empty() => code.to_string(),
        _ => UNDETERMINED_CODE.to_string(),
    }
}

/// Map a language code to its Bazarr display name.
///
/// An in-set code yields its mapped name, anything else (including `"und"`)
/// yields [`UNKNOWN_NAME`].
pub fn detect_language_name(code: &str) -> &'static str {
    LANGUAGE_NAMES
        .iter()
        .find_map(|&(k, name)| (k == code).then_some(name))
        .unwrap_or(UNKNOWN_NAME)
}

/// The `{detected_language, language_code}` pair Bazarr's detect-language
/// endpoint returns.
///
/// Both fields are sourced from this one crate so the queue detect path and the
/// detect-language error-envelope default (the no-detection
/// `{"Unknown", "und"}`) cannot drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedLanguage {
    /// Display name (in-set code → its name, otherwise `"Unknown"`).
    pub detected_language: &'static str,
    /// The normalized language code (a falsy detection → `"und"`).
    pub language_code: String,
}

/// Compute the full detect-language pair from a raw Whisper language code.
///
/// This is the single source for both the queue bazarr-service detect path and
/// the detect-language error-envelope default: it applies the `or "und"`
/// normalization, then the narrow table lookup.
pub fn detect_language(whisper_lang: Option<&str>) -> DetectedLanguage {
    let language_code = normalize_detected_code(whisper_lang);
    DetectedLanguage {
        detected_language: detect_language_name(&language_code),
        language_code,
    }
}

/// Byte-for-byte parity against the goldens under `fixtures/bazarr/pcm/`.
///
/// When a golden is absent the test skips with an `eprintln` so it arms itself
/// the moment the fixture appears.
#[cfg(test)]
mod parity {
    use super::*;
    use std::path::{Path, PathBuf};

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/bazarr/pcm")
    }

    /// Read a binary golden, or `None` (with an `eprintln` skip note) when the
    /// fixture is absent.
    fn golden(name: &str) -> Option<Vec<u8>> {
        let path = fixtures_dir().join(name);
        match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(_) => {
                eprintln!("skipping golden assertion: fixtures/bazarr/pcm/{name} is absent");
                None
            }
        }
    }

    #[track_caller]
    fn assert_f32_close(actual: &[f32], expected: &[f32]) {
        assert_eq!(actual.len(), expected.len(), "sample count mismatch");
        for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
            assert!((a - e).abs() <= 1e-7, "sample {i}: actual={a} expected={e}");
        }
    }

    /// Exact s16→f32 scale (`/32768.0`): the endpoints and a few interior values
    /// pinned so a 32767-vs-32768 divisor or endianness flip fails here.
    #[test]
    fn pcm_decode_scale_and_endianness() {
        // i16 LE: 0, +32767 (max), -32768 (min), +16384 (0.5), -16384 (-0.5).
        let bytes = [
            0x00, 0x00, // 0
            0xff, 0x7f, // 32767
            0x00, 0x80, // -32768
            0x00, 0x40, // 16384
            0x00, 0xc0, // -16384
        ];
        assert_f32_close(
            &pcm_s16le_to_f32(&bytes),
            &[0.0, 32767.0 / 32768.0, -1.0, 0.5, -0.5],
        );
    }

    /// A trailing odd byte (incomplete final sample) is dropped, not padded.
    #[test]
    fn pcm_decode_drops_trailing_odd_byte() {
        let bytes = [0x00, 0x40, 0x7f]; // one full sample (16384) + a dangling byte
        assert_f32_close(&pcm_s16le_to_f32(&bytes), &[0.5]);
    }

    /// RIFF-prefixed input header-strips to the same samples as the raw PCM:
    /// a canonical 44-byte WAV header in front of the payload is skipped.
    #[test]
    fn pcm_decode_riff_roundtrip() {
        let raw = [0x01, 0x00, 0xff, 0x7f, 0x00, 0x80, 0x34, 0x12];
        let mut wrapped = vec![0u8; WAV_HEADER_LEN];
        wrapped[..4].copy_from_slice(b"RIFF");
        wrapped.extend_from_slice(&raw);
        assert_f32_close(&pcm_s16le_to_f32(&wrapped), &pcm_s16le_to_f32(&raw));
    }

    /// Empty input → no samples.
    #[test]
    fn pcm_decode_empty() {
        assert!(pcm_s16le_to_f32(&[]).is_empty());
    }

    /// Golden cross-check against the reference floats, when present.
    /// The f32 golden is optional; when absent this skips (same pattern as the
    /// WAV goldens above).
    #[test]
    fn pcm_decode_golden() {
        let (Some(pcm), Some(f32_bytes)) = (golden("sine440.pcm"), golden("sine440.f32")) else {
            return;
        };
        let expected: Vec<f32> = f32_bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect();
        assert_f32_close(&pcm_s16le_to_f32(&pcm), &expected);
    }

    /// The in-set codes and their exact names, pinned inline.
    ///
    /// A typo or a dropped/added entry fails here even without the JSON golden.
    const GOLDEN_PAIRS: &[(&str, &str)] = &[
        ("en", "English"),
        ("es", "Spanish"),
        ("fr", "French"),
        ("de", "German"),
        ("it", "Italian"),
        ("pt", "Portuguese"),
        ("ru", "Russian"),
        ("ja", "Japanese"),
        ("zh", "Chinese"),
        ("ko", "Korean"),
        ("ar", "Arabic"),
        ("hi", "Hindi"),
        ("nl", "Dutch"),
        ("pl", "Polish"),
        ("tr", "Turkish"),
        ("vi", "Vietnamese"),
        ("th", "Thai"),
        ("sv", "Swedish"),
        ("da", "Danish"),
        ("fi", "Finnish"),
        ("no", "Norwegian"),
        ("cs", "Czech"),
        ("el", "Greek"),
        ("he", "Hebrew"),
        ("hu", "Hungarian"),
        ("id", "Indonesian"),
        ("ms", "Malay"),
        ("ro", "Romanian"),
        ("sk", "Slovak"),
        ("uk", "Ukrainian"),
    ];

    /// Every in-set code names to its mapped value, and the table holds exactly
    /// the `en..uk` list — 30 codes.
    #[test]
    fn language_name_lookup_in_set() {
        assert_eq!(
            LANGUAGE_NAMES.len(),
            30,
            "table must stay the narrow en..uk set"
        );
        assert_eq!(GOLDEN_PAIRS.len(), 30);
        for &(code, name) in GOLDEN_PAIRS {
            assert_eq!(detect_language_name(code), name, "code {code:?}");
            // A non-empty code passes through untouched, naming to its value.
            assert_eq!(
                detect_language(Some(code)),
                DetectedLanguage {
                    detected_language: name,
                    language_code: code.to_string(),
                },
                "detect pair for {code:?}",
            );
        }
    }

    /// Valid-but-out-of-set ISO codes the broader `submate-lang` table *would*
    /// name still resolve to `"Unknown"` — the parity trap.
    #[test]
    fn language_name_lookup_out_of_set() {
        for code in ["ca", "fa", "be", "xx"] {
            assert_eq!(detect_language_name(code), UNKNOWN_NAME, "code {code:?}");
            assert_eq!(
                detect_language(Some(code)),
                DetectedLanguage {
                    detected_language: UNKNOWN_NAME,
                    language_code: code.to_string(),
                },
                "detect pair for {code:?}",
            );
        }
    }

    /// The absent-detection cases: `None` and `Some("")` both collapse to the
    /// `{"Unknown", "und"}` envelope default, and `"und"` is itself not a key.
    #[test]
    fn language_name_lookup_absent() {
        let expected = DetectedLanguage {
            detected_language: UNKNOWN_NAME,
            language_code: UNDETERMINED_CODE.to_string(),
        };
        assert_eq!(detect_language(None), expected);
        assert_eq!(detect_language(Some("")), expected);
        assert_eq!(detect_language_name(UNDETERMINED_CODE), UNKNOWN_NAME);
    }

    /// Golden cross-check against `fixtures/queue/bazarr_language_names.json`,
    /// when present.
    ///
    /// The fixture is a flat `{ "<code>": "<detected_language>", ... }` object
    /// (the value `LANGUAGE_NAMES.get(code, "Unknown")` produces). When absent
    /// this skips with an `eprintln`, matching the WAV goldens above.
    #[test]
    fn language_name_lookup_golden() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/queue/bazarr_language_names.json");
        let Ok(raw) = std::fs::read_to_string(&path) else {
            eprintln!(
                "skipping golden assertion: fixtures/queue/bazarr_language_names.json is absent"
            );
            return;
        };
        for (code, expected_name) in parse_flat_json_object(&raw) {
            assert_eq!(
                detect_language_name(&code),
                expected_name,
                "golden code {code:?}",
            );
        }
    }

    /// Minimal parser for a flat `{ "k": "v", ... }` JSON object of string
    /// values. The crate is intentionally dependency-free, and the golden has
    /// no nesting/escapes/numbers, so this stays a few lines rather than
    /// pulling `serde` into a pure-data crate.
    fn parse_flat_json_object(raw: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let body = raw.trim().trim_start_matches('{').trim_end_matches('}');
        for entry in body.split(',') {
            let entry = entry.trim();
            if entry.is_empty() {
                continue;
            }
            let (k, v) = entry.split_once(':').expect("malformed golden entry");
            let key = k.trim().trim_matches('"').to_string();
            let val = v.trim().trim_matches('"').to_string();
            out.push((key, val));
        }
        out
    }
}
