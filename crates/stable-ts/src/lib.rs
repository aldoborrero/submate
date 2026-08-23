//! Reimplementation of the stable-ts slice submate uses: model, regroup, suppress_silence, output.
//!
//! The data model ([`model`]) is the foundation the regroup (B), suppress (C),
//! and output (D) stages build on.

pub mod model;
pub mod output;
pub mod regroup;
pub mod suppress_silence;

pub use model::{Segment, WhisperResult, WordTiming, round_timestamp};
pub use output::{sec2ass, to_ass};
pub use regroup::{
    RegroupOp, UnknownMethod, UnsupportedMethod, apply_regroup, apply_regroup_op, ops_to_value,
    parse_regroup_algo, str_to_valid_type,
};
pub use suppress_silence::{
    DEFAULT_MIN_WORD_DUR, audio2loudness, audio2timings, mask2timing, set_current_as_orig,
    suppress_silence, update_nonspeech_sections, wav2mask,
};
