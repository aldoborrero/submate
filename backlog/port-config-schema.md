# Port config.py settings structs to submate-config

**blocked-by:** port-types-enums

## what
Define serde structs for all ~8 settings classes (Whisper, StableTs, Server, PathMapping, Jellyfin, Queue, Subtitle, Translation) + root Config, with defaults matching Pydantic.

## where
`rust/crates/submate-config/src/lib.rs`. Add `serde` + `figment` (workspace deps).

## why
The config surface; defaults must match Python.

## falsifies
`cargo test -p submate-config parity::defaults` passes exact-match against `rust/fixtures/config/defaults.resolved.json`.
