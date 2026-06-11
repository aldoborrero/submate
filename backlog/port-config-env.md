# Port SUBMATE__ env + --config-file resolution

**blocked-by:** port-config-schema

## what
Wire figment to resolve config from the `SUBMATE__` env prefix with `__` nesting, plus a `--config-file` JSON merge, matching Pydantic-Settings precedence.

## where
`rust/crates/submate-config/src/lib.rs` — `figment::providers::Env::prefixed("SUBMATE__").split("__")` + Json provider.

## why
Env-driven config is the primary configuration path; nesting + precedence must match.

## falsifies
`cargo test -p submate-config parity::env_nesting` passes against `rust/fixtures/config/nested.env` → `rust/fixtures/config/nested.resolved.json`.
