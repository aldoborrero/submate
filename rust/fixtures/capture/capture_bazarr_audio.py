"""Capture goldens for the Bazarr raw-PCM -> WAV-header wrapping port.

Mirrors `WhisperModelWrapper._save_audio_with_wav_headers` from
`submate/whisper.py`: Bazarr posts raw s16le (signed 16-bit little-endian),
mono, 16 kHz PCM with no container. The Python `wave` module prepends the
canonical 44-byte WAV/RIFF header before handing the file to PyAV/ffmpeg.

This script is NOT part of the grind. A human runs it once (and again if the
Python spec changes), then commits the resulting fixtures:

    rust/fixtures/bazarr/pcm/sine440.pcm  raw s16le mono 16 kHz, 440 Hz tone
    rust/fixtures/bazarr/pcm/sine440.wav  Python `wave`-module output for it

The PCM is generated from a deterministic numpy sine so the golden is
reproducible byte-for-byte.
"""

from __future__ import annotations

import io
import wave
from pathlib import Path

import numpy as np

# capture/ lives at rust/fixtures/capture, so the fixtures root is its parent.
FIXTURES = Path(__file__).resolve().parent.parent

# Bazarr's wire format: s16le, mono, 16 kHz.
SAMPLE_RATE = 16000
CHANNELS = 1
SAMPLE_WIDTH = 2  # 16-bit

# Short, fixed tone: 440 Hz for 0.1 s -> 1600 samples -> 3200 PCM bytes.
FREQ_HZ = 440.0
DURATION_S = 0.1
AMPLITUDE = 0.5  # fraction of full-scale, well clear of clipping


def make_pcm() -> bytes:
    """Deterministic s16le mono 16 kHz 440 Hz sine."""
    n = int(round(SAMPLE_RATE * DURATION_S))
    t = np.arange(n, dtype=np.float64) / SAMPLE_RATE
    sine = np.sin(2.0 * np.pi * FREQ_HZ * t) * AMPLITUDE
    samples = np.round(sine * 32767.0).astype("<i2")
    return samples.tobytes()


def wrap_pcm_as_wav(pcm_data: bytes) -> bytes:
    """Byte-for-byte mirror of `_save_audio_with_wav_headers`' wrap branch.

    Uses the same `wave` module the Python code uses, so the captured header
    is exactly what production emits.
    """
    if pcm_data[:4] == b"RIFF":
        return pcm_data
    buf = io.BytesIO()
    with wave.open(buf, "wb") as wav_file:
        wav_file.setnchannels(CHANNELS)
        wav_file.setsampwidth(SAMPLE_WIDTH)
        wav_file.setframerate(SAMPLE_RATE)
        wav_file.writeframes(pcm_data)
    return buf.getvalue()


def write_bytes(rel: str, data: bytes) -> None:
    path = FIXTURES / rel
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    print(f"wrote {rel} ({len(data)} bytes)")


def main() -> None:
    pcm = make_pcm()
    wav = wrap_pcm_as_wav(pcm)
    write_bytes("bazarr/pcm/sine440.pcm", pcm)
    write_bytes("bazarr/pcm/sine440.wav", wav)


if __name__ == "__main__":
    main()
