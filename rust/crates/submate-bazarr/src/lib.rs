//! Bazarr provider glue (ports submate/bazarr/).
//!
//! Bazarr posts raw s16le (signed 16-bit little-endian), mono, 16 kHz PCM with
//! no container. Every downstream decoder (PyAV in Python, the f32 decode in
//! the Rust topology) assumes a parseable WAV, so this is the boundary
//! normalization: [`wrap_pcm_as_wav`] prepends the canonical 44-byte WAV/RIFF
//! header, byte-for-byte matching Python's `wave.open(...).writeframes(...)`
//! (see `WhisperModelWrapper._save_audio_with_wav_headers` in
//! `submate/whisper.py`).

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
///    44-byte WAV/RIFF header is prepended exactly as Python's `wave` module
///    emits for a single `writeframes` call.
///
/// This mirrors `_save_audio_with_wav_headers` minus the tempfile/cleanup
/// machinery (Python only writes to disk because PyAV wants a path; the data
/// contract is just these bytes).
pub fn wrap_pcm_as_wav(pcm: &[u8]) -> Vec<u8> {
    if pcm.starts_with(b"RIFF") {
        return pcm.to_vec();
    }

    let block_align: u16 = CHANNELS * (BITS_PER_SAMPLE / 8);
    let byte_rate: u32 = SAMPLE_RATE * u32::from(block_align);
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

/// Byte-for-byte parity against the Python `wave`-module goldens.
///
/// `rust/fixtures/` is denylisted for the port, so the goldens are captured
/// separately by `rust/fixtures/capture/capture_bazarr_audio.py`. Until they
/// land, the golden-dependent test skips with an `eprintln` (same pattern as
/// `submate-jellyfin`'s `parity`) so it arms itself the moment the fixtures
/// appear. The header bytes are also pinned inline so a regression is caught
/// even without the fixtures.
#[cfg(test)]
mod parity {
    use super::*;
    use std::path::{Path, PathBuf};

    fn fixtures_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/bazarr/pcm")
    }

    /// Read a binary golden, or `None` (with an `eprintln` skip note) when the
    /// fixture has not been captured yet.
    fn golden(name: &str) -> Option<Vec<u8>> {
        let path = fixtures_dir().join(name);
        match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(_) => {
                eprintln!(
                    "skipping golden assertion: rust/fixtures/bazarr/pcm/{name} not captured \
                     yet (rust/fixtures/ is denylisted — capture first)"
                );
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

    /// Wrap branch: raw PCM golden in, Python `wave` WAV golden out, exact.
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
}
