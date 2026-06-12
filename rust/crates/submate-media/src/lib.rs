//! ffmpeg/ffprobe wrappers (ports `submate/media.py`).
//!
//! Currently covers audio-track probing: [`get_audio_tracks`] and
//! [`get_audio_languages`] port the equivalently named Python helpers, which
//! run `ffprobe -show_streams -select_streams a -of json` and read each audio
//! stream's index, language tag and codec name.

use std::path::Path;

use serde::Deserialize;

/// Default language code used when a stream carries no `language` tag.
///
/// Matches the Python `stream.get("tags", {}).get("language", "und")` default.
const UNKNOWN_LANGUAGE: &str = "und";

/// Default codec name used when `codec_name` is absent.
///
/// Matches the Python `stream.get("codec_name", "unknown")` default.
const UNKNOWN_CODEC: &str = "unknown";

/// A single audio track in a media file.
///
/// Ports the `AudioTrack` dataclass in `submate/media.py`. `index` is the
/// 0-based position among the *audio* streams (i.e. the enumeration index over
/// the ffprobe-filtered stream list), not the global ffprobe `index` field —
/// this mirrors the Python `enumerate(...)` semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioTrack {
    /// Position among the audio streams, 0-based.
    pub index: usize,
    /// ISO 639 language code, or [`UNKNOWN_LANGUAGE`] when untagged.
    pub language: String,
    /// Codec name (e.g. `aac`, `ac3`), or [`UNKNOWN_CODEC`] when absent.
    pub codec: String,
}

/// Errors raised while probing a media file's audio tracks.
#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    /// Spawning or waiting on the `ffprobe` process failed.
    #[error("failed to run ffprobe: {0}")]
    Spawn(#[source] std::io::Error),

    /// `ffprobe` exited non-zero. Carries its captured stderr.
    #[error("ffprobe exited with status {status}: {stderr}")]
    Exit {
        /// The process exit status, rendered as ffprobe printed it.
        status: String,
        /// Captured stderr, for diagnostics.
        stderr: String,
    },

    /// The `ffprobe` JSON output could not be parsed.
    #[error("failed to parse ffprobe output: {0}")]
    Parse(#[source] serde_json::Error),
}

/// Top-level shape of `ffprobe -of json` output (only the `streams` array is
/// consumed; all other keys are ignored).
#[derive(Debug, Deserialize)]
struct ProbeOutput {
    #[serde(default)]
    streams: Vec<RawStream>,
}

/// A single stream entry from ffprobe. Only the fields needed for audio-track
/// reporting are deserialized; the rest are ignored.
#[derive(Debug, Deserialize)]
struct RawStream {
    codec_name: Option<String>,
    #[serde(default)]
    tags: StreamTags,
}

/// The `tags` object of a stream. Absent tag objects deserialize to the
/// default (no language), matching Python's `.get("tags", {})`.
#[derive(Debug, Default, Deserialize)]
struct StreamTags {
    language: Option<String>,
}

/// Parse the JSON payload produced by
/// `ffprobe -show_streams -select_streams a -of json` into [`AudioTrack`]s.
///
/// Split out from [`get_audio_tracks`] so the parsing logic is testable
/// without invoking the `ffprobe` binary. The `index` of each returned track
/// is its position in the input stream list, mirroring Python's `enumerate`.
pub fn parse_audio_tracks(json: &str) -> Result<Vec<AudioTrack>, ProbeError> {
    let probe: ProbeOutput = serde_json::from_str(json).map_err(ProbeError::Parse)?;

    let tracks = probe
        .streams
        .into_iter()
        .enumerate()
        .map(|(index, stream)| AudioTrack {
            index,
            language: stream
                .tags
                .language
                .unwrap_or_else(|| UNKNOWN_LANGUAGE.to_string()),
            codec: stream
                .codec_name
                .unwrap_or_else(|| UNKNOWN_CODEC.to_string()),
        })
        .collect();

    Ok(tracks)
}

/// Find an audio track by language code (case-insensitive).
///
/// Ports `get_audio_track_by_language` in `submate/media.py`: returns the first
/// track whose language matches, or `None`.
pub fn get_audio_track_by_language<'a>(
    tracks: &'a [AudioTrack],
    language: &str,
) -> Option<&'a AudioTrack> {
    let language = language.to_lowercase();
    tracks
        .iter()
        .find(|track| track.language.to_lowercase() == language)
}

/// Extract audio-track information from a media file via `ffprobe`.
///
/// Ports `get_audio_tracks` in `submate/media.py`. Runs
/// `ffprobe -show_streams -select_streams a -of json <path>` and parses the
/// result. Returns a [`ProbeError`] if `ffprobe` cannot be run, exits non-zero,
/// or emits unparseable output.
pub async fn get_audio_tracks(video_path: &Path) -> Result<Vec<AudioTrack>, ProbeError> {
    let output = tokio::process::Command::new("ffprobe")
        .args(["-show_streams", "-select_streams", "a", "-of", "json"])
        .arg(video_path)
        .output()
        .await
        .map_err(ProbeError::Spawn)?;

    if !output.status.success() {
        return Err(ProbeError::Exit {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_audio_tracks(&stdout)
}

/// Get the language code of every audio track in a media file.
///
/// Ports `get_audio_languages` in `submate/media.py`. On any probe failure it
/// logs at debug level and returns an empty list rather than propagating the
/// error, matching the Python helper's swallow-and-return-`[]` behaviour.
pub async fn get_audio_languages(video_path: &Path) -> Vec<String> {
    match get_audio_tracks(video_path).await {
        Ok(tracks) => tracks.into_iter().map(|track| track.language).collect(),
        Err(err) => {
            tracing::debug!(
                path = %video_path.display(),
                error = %err,
                "failed to get audio languages",
            );
            Vec::new()
        }
    }
}

#[cfg(test)]
mod parity {
    use super::*;

    /// Representative `ffprobe -show_streams -select_streams a -of json`
    /// output for a file with two audio tracks (English AAC, French AC-3),
    /// plus a third untagged track to exercise the `und`/`unknown` defaults.
    ///
    /// Embedded inline (not a fixture file) so the parser is exercised without
    /// invoking `ffprobe`. The non-audio keys ffprobe emits per stream are
    /// trimmed to the fields the parser reads.
    const SAMPLE_PROBE_JSON: &str = r#"{
        "streams": [
            {
                "index": 1,
                "codec_name": "aac",
                "codec_type": "audio",
                "tags": { "language": "eng", "title": "English" }
            },
            {
                "index": 2,
                "codec_name": "ac3",
                "codec_type": "audio",
                "tags": { "language": "fre" }
            },
            {
                "index": 3,
                "codec_name": "dts",
                "codec_type": "audio"
            }
        ]
    }"#;

    #[test]
    fn probe_parses_index_language_codec() {
        let tracks = parse_audio_tracks(SAMPLE_PROBE_JSON).expect("sample JSON parses");

        assert_eq!(
            tracks,
            vec![
                AudioTrack {
                    index: 0,
                    language: "eng".to_string(),
                    codec: "aac".to_string(),
                },
                AudioTrack {
                    index: 1,
                    language: "fre".to_string(),
                    codec: "ac3".to_string(),
                },
                AudioTrack {
                    index: 2,
                    language: "und".to_string(),
                    codec: "dts".to_string(),
                },
            ],
        );
    }

    #[test]
    fn probe_defaults_for_missing_codec_and_tags() {
        let json = r#"{ "streams": [ { "index": 0 } ] }"#;
        let tracks = parse_audio_tracks(json).expect("minimal JSON parses");

        assert_eq!(
            tracks,
            vec![AudioTrack {
                index: 0,
                language: "und".to_string(),
                codec: "unknown".to_string(),
            }],
        );
    }

    #[test]
    fn probe_handles_no_audio_streams() {
        assert!(parse_audio_tracks(r#"{ "streams": [] }"#)
            .expect("empty streams parses")
            .is_empty());
        assert!(parse_audio_tracks("{}")
            .expect("missing streams key parses")
            .is_empty());
    }

    #[test]
    fn probe_rejects_invalid_json() {
        assert!(matches!(
            parse_audio_tracks("not json"),
            Err(ProbeError::Parse(_)),
        ));
    }

    #[test]
    fn track_lookup_is_case_insensitive() {
        let tracks = parse_audio_tracks(SAMPLE_PROBE_JSON).expect("sample JSON parses");

        let found = get_audio_track_by_language(&tracks, "ENG").expect("english track found");
        assert_eq!(found.codec, "aac");

        assert!(get_audio_track_by_language(&tracks, "spa").is_none());
    }
}

/// Opt-in test against the real `ffprobe` binary. Skipped (passes as a no-op)
/// when `ffprobe` is not on `PATH`; never writes fixtures. Synthesizes a tiny
/// silent audio file with `ffmpeg` (also skipped if `ffmpeg` is missing).
#[cfg(test)]
mod real_ffprobe {
    use super::*;

    fn binary_on_path(name: &str) -> bool {
        std::process::Command::new(name)
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn probes_a_generated_audio_file() {
        if !binary_on_path("ffprobe") || !binary_on_path("ffmpeg") {
            eprintln!("skipping: ffprobe/ffmpeg not available on PATH");
            return;
        }

        let dir = std::env::temp_dir();
        let path = dir.join(format!("submate-media-probe-{}.mka", std::process::id()));

        // 1s of silence, AAC, tagged eng. Written to a temp file, not a fixture.
        let gen = std::process::Command::new("ffmpeg")
            .args(["-y", "-f", "lavfi", "-i", "anullsrc=r=16000:cl=mono", "-t", "1", "-c:a", "aac"])
            .args(["-metadata:s:a:0", "language=eng"])
            .arg(&path)
            .output()
            .expect("ffmpeg runs");
        assert!(
            gen.status.success(),
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&gen.stderr)
        );

        let tracks = get_audio_tracks(&path).await.expect("probe succeeds");
        let _ = std::fs::remove_file(&path);

        assert_eq!(tracks.len(), 1, "expected one audio track");
        assert_eq!(tracks[0].index, 0);
        assert_eq!(tracks[0].language, "eng");
    }
}
