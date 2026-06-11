# Port config field validators

**blocked-by:** port-config-env

## what
Port the field validators: JSON-kwargs parsing (transcribe_kwargs), pipe-separated lists (folders, languages), and the regroup-string parse (`cm_sl=84_sl=42++++++1`, plus the "false"/"off" → disabled handling).

## where
`rust/crates/submate-config/src/lib.rs` via `#[serde(deserialize_with = ...)]`.

## why
These coercions are observable config behavior and easy to get subtly wrong.

## falsifies
`cargo test -p submate-config parity::validators` passes exact-match against `rust/fixtures/config/validators.resolved.json` (includes the regroup-string field).
