//! Reimplementation of the stable-ts slice submate uses: model, regroup, suppress_silence, output.
//!
//! The data model ([`model`]) is the foundation the regroup (B), suppress (C),
//! and output (D) stages build on; the rest is implemented by the grind backlog
//! (see `backlog/`).

pub mod model;
pub mod suppress_silence;

pub use model::{round_timestamp, Segment, WhisperResult, WordTiming};
pub use suppress_silence::{audio2loudness, wav2mask};
