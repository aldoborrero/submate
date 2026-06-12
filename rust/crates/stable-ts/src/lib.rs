//! Reimplementation of the stable-ts slice submate uses: model, regroup, suppress_silence, output.
//!
//! The data model ([`model`]) is the foundation the regroup (B), suppress (C),
//! and output (D) stages build on.
//!
//! All four stage modules are declared here up front (with stub files for the
//! not-yet-ported stages) so each backlog item fills ONLY its own module file
//! and never edits this crate root — avoiding the merge contention that
//! repeatedly stranded the stable-ts cluster.

pub mod model;
pub mod output;
pub mod regroup;
pub mod suppress_silence;

pub use model::{round_timestamp, Segment, WhisperResult, WordTiming};
pub use regroup::{ops_to_value, parse_regroup_algo, str_to_valid_type, RegroupOp, UnknownMethod};
pub use suppress_silence::{audio2loudness, wav2mask};
