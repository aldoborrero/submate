# meta: rotation-drift check false-positives on hyphenated specialist keys

## symptom

The META supervisor "rotation drift" check runs:

```sh
grep -Fq 'porter-scout: ({' .claude/grind.config.js || echo DRIFT
```

For round-2 specialist `porter-scout` this prints `DRIFT` even though
`porter-scout` is a valid, present key in **both** the rotation array
(`rotation: [..., 'porter-scout', ...]`) and the prompts map
(line ~174: `'porter-scout': ({ round, BASE_SETUP, MAIN_GUARD }) => ...`).

## root cause

JS object keys containing a hyphen **must be quoted**, so the on-disk form is
`'porter-scout': ({`, not `porter-scout: ({`. The check's fixed-string
(`-F`) pattern omits the quotes, so it never matches a hyphenated specialist
and always reports DRIFT for them. Every multi-word specialist
(`porter-scout`, and any future `x-y` key) trips this.

Acting on the false DRIFT would wrongly report `stop_requested:true`
("restart /grind") and kill a healthy round.

## fix

Make the drift check tolerant of optional quotes around the key, e.g.:

```sh
grep -Eq "'?${SPECIALIST}'?: \(\{" .claude/grind.config.js || echo DRIFT
```

(or normalize the key list and check membership in the `rotation:` array
plus the prompts map by stripping quotes first). Single-word keys like
`parity`/`simplifier` are unaffected because they are unquoted in the source,
but the regex above still matches them.

## verification this round (2026-06-12)

- `grep -Eq "'?porter-scout'?: \(\{" .claude/grind.config.js` → matches (key present).
- `grep -q "'porter-scout'" .claude/grind.config.js` → present in rotation array.
- Conclusion: no real drift; round proceeded with `stop_requested:false`.
