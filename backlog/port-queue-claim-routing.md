# Claim routing: capability match + priority

**blocked-by:** none (submate-queue store is implemented and merged)

## what
Extend the claim query so a node only receives jobs it can run (GPU jobs → GPU nodes; translation jobs → nodes with LLM creds) and higher-priority jobs (Bazarr ASR) are claimed before library scans.

## where
`rust/crates/submate-queue/src/lib.rs`.

## why
Heterogeneous nodes + the synchronous Bazarr path needing to jump the queue.

## falsifies
`cargo test -p submate-queue claim_routing` — a GPU-only job is not handed to a CPU node; given two eligible jobs, the higher-priority one is claimed first.
