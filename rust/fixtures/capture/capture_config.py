"""Capture config resolution -> fixtures/config/*.{env,resolved.json}.

Falsifier targets: submate-config parity::{defaults,env_nesting,validators}.

Determinism note: QueueSettings.db_path defaults under XDG_DATA_HOME AND the
validator creates that directory on resolution. So we resolve under a real
writable temp dir, then NORMALIZE the home-derived prefixes back to stable
tokens (`${XDG_DATA_HOME}`, `${HOME}`) in the golden. The Rust config tests must
apply the same normalization (substitute their XDG_DATA_HOME/HOME with the same
tokens) before comparing.
"""

from __future__ import annotations

import os
import tempfile

from _common import write_json, write_text

# Real writable dirs during capture (the validator mkdir's the queue path),
# normalized to these tokens in the golden output.
_TMP = tempfile.mkdtemp(prefix="submate-fixture-")
XDG = os.path.join(_TMP, "xdg")
HOME = os.path.join(_TMP, "home")
os.makedirs(XDG, exist_ok=True)
os.makedirs(HOME, exist_ok=True)

# Each case: label -> {SUBMATE__... : value}. Empty dict = defaults.
CASES: dict[str, dict[str, str]] = {
    "defaults": {},
    "nested": {
        "SUBMATE__WHISPER__MODEL": "large-v3",
        "SUBMATE__WHISPER__DEVICE": "cuda",
        "SUBMATE__SERVER__PORT": "9999",
        "SUBMATE__TRANSLATION__BACKEND": "openai",
        "SUBMATE__TRANSLATION__OPENAI_API_KEY": "sk-test",
    },
    "validators": {
        "SUBMATE__WHISPER__TRANSCRIBE_KWARGS": '{"beam_size": 5, "best_of": 5}',
        "SUBMATE__WHISPER__FOLDERS": "/data/a|/data/b|/data/c",
        "SUBMATE__SUBTITLE__SKIP_SUBTITLE_LANGUAGES": "en|es|fr",
        "SUBMATE__STABLE_TS__CUSTOM_REGROUP": "cm_sl=84_sl=42++++++1",
    },
}


def _reset_env() -> None:
    for key in [k for k in os.environ if k.startswith("SUBMATE__")]:
        del os.environ[key]
    os.environ["XDG_DATA_HOME"] = XDG
    os.environ["HOME"] = HOME


def _normalize(value):
    """Replace machine-specific temp prefixes with portable tokens."""
    if isinstance(value, str):
        return value.replace(XDG, "${XDG_DATA_HOME}").replace(HOME, "${HOME}")
    if isinstance(value, dict):
        return {k: _normalize(v) for k, v in value.items()}
    if isinstance(value, list):
        return [_normalize(v) for v in value]
    return value


def _resolve_to_json(env: dict[str, str]) -> dict:
    _reset_env()
    os.environ.update(env)
    # Import lazily so module-level config singletons don't freeze stale env.
    from submate.config import Config

    return _normalize(Config().model_dump(mode="json"))


def main() -> None:
    for label, env in CASES.items():
        resolved = _resolve_to_json(env)
        write_json(f"config/{label}.resolved.json", resolved)
        if env:
            # also emit the .env input the Rust env-resolution test consumes
            lines = [f"{k}={v}" for k, v in env.items()]
            write_text(f"config/{label}.env", "\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
