"""Capture build_subtitle_path cases -> fixtures/paths/path_cases.json.

Falsifier target: submate-paths parity::path_cases. Each case records the full
keyword arguments and the resulting path, so the Rust port reproduces them.
"""

from __future__ import annotations

from _common import write_json

from submate.paths import build_subtitle_path, map_path
from submate.types import LanguageNamingType

# (label, kwargs) for build_subtitle_path
BUILD_CASES = [
    ("basic", dict(video_path="movie.mp4", language="eng")),
    ("subgen", dict(video_path="movie.mp4", language="eng", include_subgen_marker=True)),
    ("with_model", dict(video_path="movie.mp4", language="eng", include_model=True, model_name="medium")),
    ("iso1", dict(video_path="movie.mp4", language="eng", naming_type=LanguageNamingType.ISO_639_1)),
    ("name", dict(video_path="movie.mp4", language="eng", naming_type=LanguageNamingType.NAME)),
    ("vtt", dict(video_path="show.s01e01.mkv", language="spa", extension=".vtt")),
    ("nested_dir", dict(video_path="/media/movies/movie.mkv", language="fra")),
]

# (label, kwargs) for map_path (Docker path translation)
MAP_CASES = [
    ("disabled", dict(path="/host/movie.mkv", use_mapping=False, path_from="/host", path_to="/data")),
    ("enabled", dict(path="/host/movie.mkv", use_mapping=True, path_from="/host", path_to="/data")),
    ("no_prefix", dict(path="/other/movie.mkv", use_mapping=True, path_from="/host", path_to="/data")),
]


def main() -> None:
    out = {"build_subtitle_path": {}, "map_path": {}}
    for label, kwargs in BUILD_CASES:
        # serialize the enum for the fixture
        serial = {k: (v.value if hasattr(v, "value") else v) for k, v in kwargs.items()}
        out["build_subtitle_path"][label] = {"args": serial, "result": build_subtitle_path(**kwargs)}
    for label, kwargs in MAP_CASES:
        out["map_path"][label] = {"args": kwargs, "result": map_path(**kwargs)}
    write_json("paths/path_cases.json", out)


if __name__ == "__main__":
    main()
