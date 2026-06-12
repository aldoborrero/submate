# stable-ts (B1): regroup DSL parser

**blocked-by:** port-stablets-model-A

## what
Port `parse_regroup_algo` for the string `cm_sl=84_sl=42++++++1`: split on `_`, then `=`, args on `+`, into an ordered op list bound to methods.

## where
`rust/crates/stable-ts/src/regroup.rs`.

## why
Drives the regroup pipeline; isolating the parser makes the apply step testable.

## falsifies
`cargo test -p stable-ts parity::regroup_parse` produces the op-list golden `rust/fixtures/stablets/regroup_parse.json`.
