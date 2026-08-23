//! Parakeet (and other transcribe.cpp model families) transcription backend.
//!
//! Feature-gated on `parakeet`, which pulls in
//! [transcribe.cpp](https://github.com/handy-computer/transcribe.cpp) and
//! compiles its vendored C++/ggml. [`ParakeetBackend`] implements
//! [`submate_whisper::Transcriber`], so the `Dispatcher` drives it exactly like
//! the whisper.cpp backend — but with Parakeet's word/token-level timestamps.
//!
//! Without the `parakeet` feature this crate is an empty stub that builds with no
//! C++ toolchain (the same posture as submate-whisper's `model`).

#[cfg(feature = "parakeet")]
mod backend;

#[cfg(feature = "parakeet")]
pub use backend::ParakeetBackend;
