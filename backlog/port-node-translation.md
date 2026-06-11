# Translation jobs on the node

**blocked-by:** port-node-agent, port-translate-srt-apply

## what
Let a node handle translation jobs (subtitle in → translated subtitle out) via submate-translate, advertised as a capability at register time.

## where
`rust/crates/submate-node/src/lib.rs`.

## why
Translation is node work too (LLM-bound, not GPU-bound); CPU nodes can serve it.

## falsifies
`cargo test -p submate-node node_translation` — a translation job pulled from a mock server is processed through submate-translate and the translated subtitle is returned.
