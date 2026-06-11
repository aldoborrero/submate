# stable-ts (B2): clamp_max + split_by_length

**blocked-by:** port-stablets-regroup-parse-B1

## what
Port the regroup algorithms used by the config string: `clamp_max` (median-based duration clamp) and `split_by_length` (max_chars), plus the segment-split helpers.

## where
`rust/crates/stable-ts/src/regroup.rs`.

## why
These transform the WhisperResult into clean subtitle segments — the project's quality differentiator.

## falsifies
`cargo test -p stable-ts parity::regroup_apply` transforms `00_raw.json` → each `rust/fixtures/stablets/*/01_regroup_*.json` exactly (one golden per staged op).
