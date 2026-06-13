// grind.config.js — CONFIG for the submate Python→Rust port.
//
// Concatenated with .claude/workflows/grind-base.js by /grind and passed to
// Workflow({script}). The generic loop (triage → implementers in worktrees →
// serialized merge queue → rotating specialist → meta) lives in the base; this
// file defines what is project-specific: the gate, the rotation, the seams.
//
// The Python tree under submate/ is the SPEC and stays in place during the
// port. Golden fixtures captured from it live in rust/fixtures/ (see
// rust/fixtures/README.md). The port lives under rust/.

export const meta = {
  name: 'submate-rs',
  description: 'Multi-agent Python→Rust port of submate: implementers + rotating port specialist, parity-gated',
  whenToUse: 'When the user wants autonomous parallel progress porting submate to rust/',
  phases: [
    { title: 'Triage', detail: 'salvage orphans, pick port items by dependency order' },
    { title: 'Work', detail: 'implementers + 1 rotating specialist' },
    { title: 'Merge', detail: 'serialized queue, cargo test+clippy gate (in nix devshell)' },
    { title: 'Meta', detail: 'parity-drift + un-ported-surface audit, stop-signal' },
  ],
}

// clippy + the rust toolchain live in the nix devshell (nix/devshell.nix), not
// on the bare PATH, so the gate runs THROUGH the devshell. --manifest-path
// (never `cd rust && cargo`) keeps it a single non-compound command.
const FC = "nix develop --command bash -c 'cargo test --manifest-path rust/Cargo.toml --workspace && cargo clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings'"

const PORT_CTX = `submate is a ~6000-LOC Python AI-subtitle tool (Click CLI,
FastAPI server = Bazarr ASR provider + Jellyfin webhook, Huey SqliteHuey
queue, Whisper transcription + LLM translation). It is being ported to rust/
crate-by-crate; the Python code under submate/ is the SPEC and stays in place.
Golden fixtures live in rust/fixtures/ (see rust/fixtures/README.md). The
CONTRACT: Rust output must match the Python golden — EXACTLY for pure-data
layers (config resolution, language enum, paths, regroup, output formatting,
mocked-LLM translation) and STRUCTURALLY-within-tolerance for transcription
(whisper.cpp != faster-whisper: segment count +-1, timing +-200ms, text
token-set-ratio >= 0.9, via parity::assert_segments_close). Config keys
(SUBMATE__ env prefix, __ nesting), enum .value strings, and HTTP route
paths/signatures must match Python byte-for-byte. The crate map: submate-types
(types.py), submate-lang (language.py), submate-config (config.py), stable-ts
(the ~1240-LOC slice: model/regroup/suppress_silence/output), submate-subtitle
(subtitle.py), submate-paths (paths.py), submate-media (media.py),
submate-whisper (whisper.py via whisper-rs), submate-translate (translation.py),
submate-jellyfin + submate-bazarr (integrations), submate-queue (queue/),
submate-server (server/), submate-cli (cli/). parity/ is the test-helper crate.`

// Triage focus: a list of slug PREFIXES triage picks first each round (every
// ready item matching ANY prefix), then fills remaining slots normally. Set to
// [] to disable (foundational-first priority). Still gated by blocked-by, so a
// dependency chain advances one stage per round.
//
// EXCLUSIVE CLI-UX focus: work ONLY items whose slug starts with these prefixes;
// do NOT fill leftover implementer slots with other product items. When no
// matching item is ready, the round runs no implementers and the run dries out.
const FOCUS = ['port-stablets-regroup-', 'perf-']
const FOCUS_EXCLUSIVE = true

const CONFIG = {
  name: 'submate-rs',
  implementers: 3,
  archCadence: 6,
  // Round cap. Honored even when launch args don't propagate (the earlier run
  // ignored {rounds:2} and ran unbounded). Raise this — or pass {rounds:N} at
  // launch — for a longer run; set Infinity to drain the whole backlog.
  maxRounds: 4,
  dryLimit: 2,
  rotation: ['parity', 'porter-scout', 'aligner', 'simplifier', 'curator', 'documenter'],

  fastCheck: FC,

  // Honor backlog `blocked-by:` edges — the generic triage doesn't. An item is
  // READY only when every dependency it lists is already done (its backlog file
  // removed on merge). Without this, triage could pick a deep item (e.g.
  // port-whisper-pipeline) before its dependency crates exist and waste the round.
  triageExtra: () => `
2. **Dependency readiness (REQUIRED gate before picking).** Many items have a
   \`**blocked-by:** slug-a, slug-b\` line near the top. An item is READY only if
   EVERY listed slug's file no longer exists in backlog/ (a finished dependency
   \`git rm\`s its own file on merge). Check per candidate:
   \`\`\`sh
   # ready iff this prints nothing:
   for dep in $(sed -n 's/^\\*\\*blocked-by:\\*\\* //p' backlog/<slug>.md | tr ',' ' '); do
     [ -f "backlog/$(echo "$dep" | xargs).md" ] && echo "BLOCKED by $dep"
   done
   \`\`\`
   Skip any item with an unsatisfied blocker THIS round.${FOCUS && FOCUS.length ? `

3. **FOCUS — ${FOCUS_EXCLUSIVE ? 'EXCLUSIVE filter' : 'priority override'} (this run).**
   Consider ONLY READY items whose slug STARTS WITH any of these prefixes:
   ${FOCUS.map((p) => '`' + p + '`').join(', ')}.
   ${FOCUS_EXCLUSIVE
       ? 'Pick ONLY those — do NOT fill leftover implementer slots with any other item, even if ready. If fewer than the implementer count match, run fewer this round; if none match, pick nothing (the run will dry out and stop, which is correct).'
       : 'Pick every matching ready item FIRST, then fill leftover slots with other ready items by normal priority.'}` : `
   Prefer ready foundational items (types/lang/config/proto/paths, then the
   stable-ts A stage) — they unblock the most downstream work.`}`,

  // Skip parity when no rust src changed since its last run (nothing new to
  // diverge). One gate-agent evaluates this; rotation advances past it.
  skipIf: {
    parity: `LAST=$(git log --format=%H -1 --grep='^parity r'); : "\${LAST:=$(git rev-list --max-parents=0 HEAD)}"; [ -z "$(git diff --name-only "$LAST"..HEAD -- rust/crates/)" ]`,
  },

  // Implementers never hand-touch the grind, the Python spec, or golden truth.
  // Golden fixtures change only via a deliberate capture run (a human/parity
  // item), never as a side effect of porting code.
  mergeDenylist: [/^\.claude\//, /^\.git\//, /^\.github\//, /grind-base\.js$/,
                  /^rust\/fixtures\//, /^submate\//, /\.py$/],

  // Binary gate: the merged result must pass test + clippy. A port — correctness
  // only, no perf budget.
  mergeGate: (impl, actualFiles) => {
    const files = actualFiles ?? []
    return {
      needsGate: files.some(f => /^rust\/.*\.rs$/.test(f) || /Cargo\.(toml|lock)$/.test(f)),
      cmd: FC + ' 2>&1 | tail -20',
      instructions: '   Binary gate: 0 if green, 100 if red.',
    }
  },

  architect: ({ round, BASE_SETUP, MAIN_GUARD }) => `
You are the ARCHITECT for submate-rs. Round ${round}. Runs every 6 rounds —
macro-structure of the PORT, not bugs.
${BASE_SETUP}
${PORT_CTX}
Review CRATE BOUNDARIES and the ASYNC MODEL against the Python crate map:
- Did a leaf utility land in the wrong crate (a regroup helper outside
  stable-ts, an HTTP backend outside submate-translate)?
- Is the pure-data / I/O seam intact? Pure crates (submate-types, -lang,
  -config, stable-ts, -subtitle, -paths, parity) MUST stay free of
  tokio/reqwest/rusqlite so they remain exact-diff testable. Flag any I/O dep
  that crept into a pure crate's Cargo.toml.
- Async model: is whisper-rs (blocking) wrapped in spawn_blocking? Is the queue
  poller a single worker matching SqliteHuey, not a re-architected job system?
  Does axum shared state mirror FastAPI DI?
- Does a crate re-implement what a sibling already ported?
Diff since the last architect commit:
\`\`\`sh
LAST=$(git log --format=%H -1 --grep='^architect r')
: "\${LAST:=$(git rev-list --max-parents=0 HEAD)}"
git diff --stat "$LAST"..HEAD -- rust/crates/
\`\`\`
File one backlog/arch-*.md per structural divergence with a falsifier
(e.g. "submate-config/Cargo.toml has no tokio/reqwest/rusqlite dependency").
Empty diff → marker commit \`architect r${round}: 0 rust delta, skip\`.
${MAIN_GUARD}`,

  specialists: {
    parity: ({ round, BASE_SETUP, MAIN_GUARD }) => `
You are the PARITY agent for submate-rs. Round ${round}.
${BASE_SETUP}
${PORT_CTX}
You find Rust modules whose output DIVERGES from the Python golden and file
backlog items. You mostly FALSIFY and FILE, you do not fix. Pick ONE ported
crate that changed since your last run:
\`\`\`sh
LAST=$(git log --format=%H -1 --grep='^parity r')
: "\${LAST:=$(git rev-list --max-parents=0 HEAD)}"
git diff --name-only "$LAST"..HEAD -- rust/crates/
\`\`\`
Run its parity tests:
\`nix develop --command cargo test --manifest-path rust/Cargo.toml -p <crate> parity:: 2>&1 | tail -40\`.
- A parity test MISSING for a code path that has a golden fixture → file
  backlog/parity-<crate>-<path>.md: falsifier "cargo test -p <crate>
  parity::<x> exists and passes against rust/fixtures/<y>".
- A parity test that FAILS → file backlog/bug-parity-<crate>-<path>.md with the
  EXACT diff (Python golden value vs Rust value, first differing keys/lines) so
  an implementer can fix blind.
For transcription judge against the TOLERANCE contract, never byte-equality.
Trivial off-by-one formatting you may fix directly (net-small). Empty diff →
marker commit \`parity r${round}: 0 rust delta since <LAST>, skip\`.
${MAIN_GUARD}`,

    'porter-scout': ({ round, BASE_SETUP, MAIN_GUARD }) => `
You are the PORTER-SCOUT for submate-rs. Round ${round}.
${BASE_SETUP}
${PORT_CTX}
You keep the backlog FED: find Python surface not yet ported and file
decomposed, dependency-ordered items. Respect the order: types/lang/config/
paths first, then leaf utils (subtitle), then the stable-ts slice A→B→C→D, then
whisper, translate, queue, server, cli, integrations. Per round:
1. Pick ONE un-ported or partially-ported Python module not already covered by
   an open backlog item.
2. Read it + its tests under tests/. Decompose into 1-3 independent,
   one-worktree-sized items, EACH with a parity falsifier naming a concrete
   golden fixture under rust/fixtures/. If the golden does not exist yet, name
   it anyway and add "requires fixture: rust/fixtures/<y> (capture first)" — you
   cannot touch rust/fixtures/ (denylisted), so flag it for a human/capture.
3. File backlog/port-<crate>-<unit>.md (## what / ## where / ## why /
   ## falsifies). Add \`blocked-by:\` lines for unported dependencies.
Nothing un-ported left → marker commit \`scout r${round}: surface exhausted\`.
${MAIN_GUARD}`,

    aligner: ({ round, BASE_SETUP, MAIN_GUARD }) => `
You are the ALIGNER for submate-rs. Round ${round}.
${BASE_SETUP}
${PORT_CTX}
Three contracts must match Python EXACTLY: (1) CONFIG KEYS — the SUBMATE__ env
prefix, __ nesting, every settings field name + default; (2) ENUM .value
STRINGS — WhisperModel/Device/TranscriptionTask/TranslationBackend/LanguageCode
must serialize to the same strings Python emits; (3) ROUTE SIGNATURES —
/bazarr/asr, /bazarr/detect-language, /jellyfin/webhook, /, /version,
/queue/stats: same paths, methods, request/response JSON shapes. Pick ONE
contract, follow it Python → Rust crate → golden fixture. File
backlog/align-<slug>.md for any key/value/route that differs (a missing serde
rename, a drifted default, a route returning a different shape than FastAPI),
with a falsifier. One contract per round.
${MAIN_GUARD}`,

    simplifier: ({ round, BASE_SETUP, MAIN_GUARD }) => `
You are the SIMPLIFIER for submate-rs. Round ${round}.
${BASE_SETUP}
Gate on rust delta:
\`\`\`sh
LAST=$(git log --format=%H -1 --grep='^simplify r')
: "\${LAST:=$(git rev-list --max-parents=0 HEAD)}"
git log --oneline "$LAST"..HEAD -- rust/crates/
\`\`\`
Empty → marker commit \`simplify r${round}: 0 rust delta since <LAST>, skip\`.
A port accretes cruft: a .clone() where a borrow works, a hand loop where an
iterator reads, an unwrap() that should be a typed error (thiserror), a util
duplicated across crates that belongs in a shared one, a dep pulled for one
call site replaceable by std. Do NOT change observable behavior — the parity
golden tests are your safety net; run \`${FC}\` and keep it green. Net-negative-
line commits only.
${MAIN_GUARD}`,

    curator: ({ round, BASE_SETUP, MAIN_GUARD }) => `
You are the CURATOR for submate-rs. Round ${round}.
${BASE_SETUP}
You own the Cargo dependency set + CVE scanning (no Dependabot here). Diff:
\`\`\`sh
LAST=$(git log --format=%H -1 --grep='^deps r')
: "\${LAST:=$(git rev-list --max-parents=0 HEAD)}"
git diff "$LAST"..HEAD -- 'rust/**/Cargo.toml' rust/Cargo.lock
\`\`\`
Per round pick ONE: (a) CVE scan EVERY round —
\`nix develop --command cargo audit --file rust/Cargo.lock\`; file
backlog/sec-deps-<slug>.md for any advisory, high+ in the subject. (b) A new
dep since LAST — load-bearing, or replaceable by std / an existing dep? Reject
micro-deps. Watch port traps: two srt parsers, two http clients (standardized
on reqwest+rustls), an SDK pulled where raw reqwest suffices. (c) Confirm pure
crates carry no I/O deps (manifest-side mirror of the architect's seam check).
Land trivial bumps directly (Cargo.lock is yours; the gate catches breakage).
Clean scan + empty diff → marker commit \`deps r${round}: cargo-audit clean, 0
manifest delta since <LAST>\`.
${MAIN_GUARD}`,

    documenter: ({ round, BASE_SETUP, MAIN_GUARD }) => `
You are the DOCUMENTER for submate-rs. Round ${round}.
${BASE_SETUP}
rust/README.md is the port map (Python module ↔ Rust crate ↔ parity status);
rust/fixtures/README.md is the golden-fixture contract. Diff:
\`\`\`sh
LAST=$(git log --format=%H -1 --grep='^docs r')
: "\${LAST:=$(git rev-list --max-parents=0 HEAD)}"
git diff --name-only "$LAST"..HEAD -- rust/crates/ rust/fixtures/
\`\`\`
For each changed file: a newly-ported crate not marked done in the port map?
a new golden fixture without an entry in the fixtures README? a config key /
route documented for Python but missing from the Rust crate docs? Fix in place,
prefix \`docs r${round}:\`. Empty diff → marker commit, stop.
${MAIN_GUARD}`,
  },
}
