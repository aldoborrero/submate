# sec-deps: cargo-audit r5 — 0 vulnerabilities; cleared one unmaintained transitive dep

**relates-to:** submate-cli dependency set

## Scan result

`cargo audit --file rust/Cargo.lock` against the RustSec advisory DB
(1130 advisories, 292 crate dependencies): **0 vulnerabilities**.

One informational `unmaintained` warning was surfaced and has been
resolved in this same round:

- **RUSTSEC-2025-0119** — `number_prefix 0.4.0` unmaintained.
  Path: `number_prefix 0.4.0 -> indicatif 0.17.11 -> submate-cli`.
  Severity: informational (unmaintained, no CVE, no patched advisory).

## Action taken

Bumped the workspace `indicatif` pin `0.17 -> 0.18` (`rust/Cargo.toml`).
indicatif 0.18 dropped its `number_prefix` dependency in favour of the
maintained `unit-prefix` crate, so the bump removes the unmaintained
transitive dep entirely:

```
indicatif 0.17.11 -> 0.18.4
console   0.15.11 -> 0.16.3   (transitive bump pulled in by indicatif 0.18)
number_prefix 0.4.0  REMOVED
unit-prefix   0.5.2  ADDED
```

The only indicatif API used by `submate-cli` is the stable progress-bar
surface (`ProgressBar::new`, `set_draw_target`,
`ProgressDrawTarget::stderr`, `ProgressStyle::with_template` /
`default_spinner` — see `submate-cli/src/main.rs`), all unchanged across
the 0.17 -> 0.18 boundary, so the bump is a drop-in.

## Tooling note for future rounds

The pinned `cargo-audit 0.21.2` cannot parse RustSec advisories that use
`cvss = "CVSS:4.0/..."` (50+ such advisories now exist in the DB, e.g.
RUSTSEC-2026-0076 for `libcrux-ml-dsa`, none of which are submate
dependencies). A raw `cargo audit` aborts with
`unsupported CVSS version: 4.0` *before scanning any of our crates*,
exiting non-zero — this is a tooling-version failure, **not** a clean
scan, and must not be reported as clean.

Workaround used this round: copy the advisory DB to a temp dir, neutralise
the unparseable `cvss = "CVSS:4.0..."` lines (metadata only — package /
version ranges drive matching, not the CVSS string), then run
`cargo audit --no-fetch --db <tmp> --file rust/Cargo.lock`.

Durable fix: bump `cargo-audit` in the dev shell to a release with
CVSS 4.0 support (rustsec/cargo-audit added it after 0.21.2). Until then,
every curator round must apply the temp-DB workaround or the scan silently
fails closed.
