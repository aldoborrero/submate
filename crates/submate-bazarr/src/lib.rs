//! Bazarr provider glue.
//!
//! Bazarr posts raw s16le (signed 16-bit little-endian), mono, 16 kHz PCM with
//! no container. The downstream f32 decode assumes a parseable WAV, so this is
//! the boundary normalization: [`wrap_pcm_as_wav`] prepends the canonical
//! 44-byte WAV/RIFF header.
//!
//! It also hosts the Bazarr detect-language naming: [`detect_language`] turns a
//! raw Whisper language code into the `{detected_language, language_code}` pair
//! the detect-language endpoint returns, via the deliberately narrow
//! [`LANGUAGE_NAMES`] table (NOT the broader `submate-lang` `name_en` table).

/// Bazarr's wire format: mono.
const CHANNELS: u16 = 1;
/// Bazarr's wire format: 16 kHz.
const SAMPLE_RATE: u32 = 16_000;
/// Bazarr's wire format: 16-bit samples (2 bytes).
const BITS_PER_SAMPLE: u16 = 16;
/// Canonical PCM WAV header length.
const WAV_HEADER_LEN: usize = 44;

/// Wrap raw Bazarr PCM in a canonical WAV/RIFF container.
///
/// Two deterministic, content-only branches:
///
/// 1. **RIFF passthrough** — if `pcm` already begins with `b"RIFF"`, it is
///    already a WAV container and is returned unchanged.
/// 2. **Raw-PCM wrap** — otherwise `pcm` is treated as s16le mono 16 kHz and a
///    canonical 44-byte WAV/RIFF header is prepended.
pub fn wrap_pcm_as_wav(pcm: &[u8]) -> Vec<u8> {
    if pcm.starts_with(b"RIFF") {
        return pcm.to_vec();
    }

    let block_align: u16 = CHANNELS * (BITS_PER_SAMPLE / 8);
    let byte_rate: u32 = SAMPLE_RATE * u32::from(block_align);
    // The WAV `data` chunk size is a u32, so the container caps at 4 GiB of PCM
    // (~37 h of 16 kHz mono s16le) by format. Bazarr clips are seconds long, so
    // the cast never truncates in practice; truncation here would mean a clip
    // beyond what WAV can represent at all.
    let data_len = pcm.len() as u32;
    // RIFF chunk size covers everything after the first 8 bytes: the WAVE tag,
    // the 24-byte fmt chunk, the 8-byte data chunk header, and the payload.
    let riff_size = 36u32.saturating_add(data_len);

    let mut out = Vec::with_capacity(WAV_HEADER_LEN + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());

    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(pcm);

    out
}

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
/// If `bytes` begins with `b"RIFF"` — a clip run back through
/// [`wrap_pcm_as_wav`] — the canonical 44-byte header is skipped first, so the
/// two functions are inverses on that header.
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

/// Byte-for-byte parity against the WAV-header goldens under
/// `fixtures/bazarr/pcm/`.
///
/// When a golden is absent the test skips with an `eprintln` so it arms itself
/// the moment the fixture appears. The header bytes are also pinned inline so a
/// regression is caught even without the fixtures.
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
    fn assert_bytes_eq(actual: &[u8], golden: &[u8]) {
        assert_eq!(
            actual.len(),
            golden.len(),
            "byte length mismatch: {} vs {}",
            actual.len(),
            golden.len()
        );
        if let Some(i) = actual.iter().zip(golden).position(|(a, g)| a != g) {
            panic!(
                "byte parity mismatch at offset {i}: actual=0x{:02x} golden=0x{:02x}",
                actual[i], golden[i]
            );
        }
    }

    /// Wrap branch: raw PCM golden in, WAV golden out, exact.
    #[test]
    fn wav_wrap_matches_python_wave() {
        let (Some(pcm), Some(wav)) = (golden("sine440.pcm"), golden("sine440.wav")) else {
            return;
        };
        assert_bytes_eq(&wrap_pcm_as_wav(&pcm), &wav);
    }

    /// RIFF passthrough: bytes already starting with `b"RIFF"` (the WAV golden
    /// fed back in) come out unchanged.
    #[test]
    fn wav_wrap_riff_passthrough() {
        let Some(wav) = golden("sine440.wav") else {
            return;
        };
        assert_eq!(&wav[..4], b"RIFF");
        assert_bytes_eq(&wrap_pcm_as_wav(&wav), &wav);
    }

    /// Header bytes pinned inline so a byte_rate/block_align off-by-one fails
    /// even when the fixtures are absent.
    #[test]
    fn wav_header_layout_is_canonical() {
        // 4 bytes of arbitrary, non-RIFF PCM.
        let pcm = [0x01u8, 0x00, 0xff, 0x7f];
        let out = wrap_pcm_as_wav(&pcm);
        assert_eq!(out.len(), WAV_HEADER_LEN + pcm.len());

        let expected_header: [u8; WAV_HEADER_LEN] = [
            b'R', b'I', b'F', b'F', // RIFF
            0x28, 0x00, 0x00, 0x00, // 36 + 4 = 40
            b'W', b'A', b'V', b'E', // WAVE
            b'f', b'm', b't', b' ', // "fmt "
            0x10, 0x00, 0x00, 0x00, // fmt chunk size = 16
            0x01, 0x00, // audio format = PCM
            0x01, 0x00, // channels = 1
            0x80, 0x3e, 0x00, 0x00, // sample rate = 16000
            0x00, 0x7d, 0x00, 0x00, // byte rate = 32000
            0x02, 0x00, // block align = 2
            0x10, 0x00, // bits per sample = 16
            b'd', b'a', b't', b'a', // "data"
            0x04, 0x00, 0x00, 0x00, // data len = 4
        ];
        assert_eq!(&out[..WAV_HEADER_LEN], &expected_header);
        assert_eq!(&out[WAV_HEADER_LEN..], &pcm);
    }

    /// Empty PCM still yields a valid 44-byte header with zero data length.
    #[test]
    fn wav_wrap_empty_pcm() {
        let out = wrap_pcm_as_wav(&[]);
        assert_eq!(out.len(), WAV_HEADER_LEN);
        assert_eq!(&out[..4], b"RIFF");
        assert_eq!(&out[40..44], &[0x00, 0x00, 0x00, 0x00]); // data len = 0
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

    /// wrap/decode are inverses: RIFF-prefixed input header-strips to the same
    /// samples as the raw PCM, so a clip survives a wrap→decode round-trip.
    #[test]
    fn pcm_decode_riff_roundtrip() {
        let raw = [0x01, 0x00, 0xff, 0x7f, 0x00, 0x80, 0x34, 0x12];
        let wrapped = wrap_pcm_as_wav(&raw);
        assert_eq!(&wrapped[..4], b"RIFF");
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
