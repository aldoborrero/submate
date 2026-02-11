"""Tests for subtitle file scanner service."""

from pathlib import Path


class TestSubtitleScanner:
    """Tests for SubtitleScanner class."""

    def test_scan_subtitles_finds_srt(self, temp_dir: Path) -> None:
        """Test scanning finds .srt files next to media."""
        from submate.services.scanner import SubtitleScanner

        # Create a media file and subtitle file
        media_file = temp_dir / "movie.mp4"
        media_file.touch()
        subtitle_file = temp_dir / "movie.srt"
        subtitle_file.write_text("1\n00:00:00,000 --> 00:00:01,000\nHello\n")

        scanner = SubtitleScanner()
        subtitles = scanner.scan_for_media(media_file)

        assert len(subtitles) == 1
        assert subtitles[0]["path"] == subtitle_file
        assert subtitles[0]["source"] == "external"

    def test_scan_subtitles_finds_multiple_extensions(self, temp_dir: Path) -> None:
        """Test scanning finds various subtitle extensions."""
        from submate.services.scanner import SUBTITLE_EXTENSIONS, SubtitleScanner

        media_file = temp_dir / "movie.mkv"
        media_file.touch()

        # Create subtitle files with different extensions
        created_files = []
        for ext in SUBTITLE_EXTENSIONS:
            sub_file = temp_dir / f"movie{ext}"
            sub_file.write_text("subtitle content")
            created_files.append(sub_file)

        scanner = SubtitleScanner()
        subtitles = scanner.scan_for_media(media_file)

        assert len(subtitles) == len(SUBTITLE_EXTENSIONS)
        found_paths = {s["path"] for s in subtitles}
        for created in created_files:
            assert created in found_paths

    def test_scan_subtitles_detects_language_from_filename(self, temp_dir: Path) -> None:
        """Test language detection from filename patterns."""
        from submate.services.scanner import SubtitleScanner

        media_file = temp_dir / "movie.mp4"
        media_file.touch()

        # Create subtitles with various language patterns
        patterns = [
            ("movie.en.srt", "en"),
            ("movie.eng.srt", "en"),
            ("movie.english.srt", "en"),
            ("movie.es.srt", "es"),
            ("movie.spa.srt", "es"),
            ("movie.spanish.srt", "es"),
            ("movie.de.srt", "de"),
            ("movie.ger.srt", "de"),
            ("movie.german.srt", "de"),
        ]

        for filename, expected_lang in patterns:
            sub_file = temp_dir / filename
            sub_file.write_text("content")

        scanner = SubtitleScanner()
        subtitles = scanner.scan_for_media(media_file)

        # Build a mapping of filename to detected language
        lang_map = {s["path"].name: s["language"] for s in subtitles}

        for filename, expected_lang in patterns:
            assert lang_map.get(filename) == expected_lang, f"Expected {expected_lang} for {filename}"

    def test_scan_subtitles_handles_no_subtitles(self, temp_dir: Path) -> None:
        """Test empty result when no subtitles exist."""
        from submate.services.scanner import SubtitleScanner

        media_file = temp_dir / "movie.mp4"
        media_file.touch()

        # Create only non-subtitle files
        (temp_dir / "movie.nfo").write_text("info")
        (temp_dir / "movie.jpg").write_bytes(b"\x00")

        scanner = SubtitleScanner()
        subtitles = scanner.scan_for_media(media_file)

        assert subtitles == []

    def test_scan_subtitles_undetermined_language(self, temp_dir: Path) -> None:
        """Test subtitles without language in filename get 'und' language."""
        from submate.services.scanner import SubtitleScanner

        media_file = temp_dir / "movie.mp4"
        media_file.touch()
        subtitle_file = temp_dir / "movie.srt"
        subtitle_file.write_text("content")

        scanner = SubtitleScanner()
        subtitles = scanner.scan_for_media(media_file)

        assert len(subtitles) == 1
        assert subtitles[0]["language"] == "und"


class TestDetectLanguageFromFilename:
    """Tests for language detection from filename."""

    def test_detect_iso_639_1_codes(self) -> None:
        """Test detection of ISO 639-1 codes (2-letter)."""
        from submate.services.scanner import SubtitleScanner

        scanner = SubtitleScanner()

        # Test common 2-letter codes
        assert scanner.detect_language_from_filename("movie.en.srt") == "en"
        assert scanner.detect_language_from_filename("movie.es.srt") == "es"
        assert scanner.detect_language_from_filename("movie.fr.srt") == "fr"
        assert scanner.detect_language_from_filename("movie.de.srt") == "de"
        assert scanner.detect_language_from_filename("movie.it.srt") == "it"
        assert scanner.detect_language_from_filename("movie.pt.srt") == "pt"
        assert scanner.detect_language_from_filename("movie.ja.srt") == "ja"
        assert scanner.detect_language_from_filename("movie.zh.srt") == "zh"
        assert scanner.detect_language_from_filename("movie.ko.srt") == "ko"
        assert scanner.detect_language_from_filename("movie.ru.srt") == "ru"

    def test_detect_iso_639_2_codes(self) -> None:
        """Test detection of ISO 639-2 codes (3-letter)."""
        from submate.services.scanner import SubtitleScanner

        scanner = SubtitleScanner()

        # Test common 3-letter codes
        assert scanner.detect_language_from_filename("movie.eng.srt") == "en"
        assert scanner.detect_language_from_filename("movie.spa.srt") == "es"
        assert scanner.detect_language_from_filename("movie.fra.srt") == "fr"
        assert scanner.detect_language_from_filename("movie.fre.srt") == "fr"  # ISO 639-2/B
        assert scanner.detect_language_from_filename("movie.deu.srt") == "de"
        assert scanner.detect_language_from_filename("movie.ger.srt") == "de"  # ISO 639-2/B
        assert scanner.detect_language_from_filename("movie.ita.srt") == "it"
        assert scanner.detect_language_from_filename("movie.por.srt") == "pt"
        assert scanner.detect_language_from_filename("movie.jpn.srt") == "ja"
        assert scanner.detect_language_from_filename("movie.zho.srt") == "zh"
        assert scanner.detect_language_from_filename("movie.chi.srt") == "zh"  # ISO 639-2/B
        assert scanner.detect_language_from_filename("movie.kor.srt") == "ko"
        assert scanner.detect_language_from_filename("movie.rus.srt") == "ru"

    def test_detect_full_language_names(self) -> None:
        """Test detection of full language names."""
        from submate.services.scanner import SubtitleScanner

        scanner = SubtitleScanner()

        assert scanner.detect_language_from_filename("movie.english.srt") == "en"
        assert scanner.detect_language_from_filename("movie.spanish.srt") == "es"
        assert scanner.detect_language_from_filename("movie.french.srt") == "fr"
        assert scanner.detect_language_from_filename("movie.german.srt") == "de"
        assert scanner.detect_language_from_filename("movie.italian.srt") == "it"
        assert scanner.detect_language_from_filename("movie.portuguese.srt") == "pt"
        assert scanner.detect_language_from_filename("movie.japanese.srt") == "ja"
        assert scanner.detect_language_from_filename("movie.chinese.srt") == "zh"
        assert scanner.detect_language_from_filename("movie.korean.srt") == "ko"
        assert scanner.detect_language_from_filename("movie.russian.srt") == "ru"

    def test_detect_case_insensitive(self) -> None:
        """Test language detection is case-insensitive."""
        from submate.services.scanner import SubtitleScanner

        scanner = SubtitleScanner()

        assert scanner.detect_language_from_filename("movie.EN.srt") == "en"
        assert scanner.detect_language_from_filename("movie.ENG.srt") == "en"
        assert scanner.detect_language_from_filename("movie.English.srt") == "en"
        assert scanner.detect_language_from_filename("movie.ENGLISH.srt") == "en"

    def test_detect_no_language_returns_none(self) -> None:
        """Test None is returned when no language is detected."""
        from submate.services.scanner import SubtitleScanner

        scanner = SubtitleScanner()

        assert scanner.detect_language_from_filename("movie.srt") is None
        assert scanner.detect_language_from_filename("movie.forced.srt") is None
        assert scanner.detect_language_from_filename("movie.sdh.srt") is None
        assert scanner.detect_language_from_filename("subtitles.srt") is None


class TestScanDirectory:
    """Tests for directory scanning."""

    def test_scan_directory_finds_media_and_subtitles(self, temp_dir: Path) -> None:
        """Test scanning a directory finds media files and their subtitles."""
        from submate.services.scanner import SubtitleScanner

        # Create media files with subtitles
        (temp_dir / "movie1.mp4").touch()
        (temp_dir / "movie1.en.srt").write_text("content")
        (temp_dir / "movie1.es.srt").write_text("content")

        (temp_dir / "movie2.mkv").touch()
        (temp_dir / "movie2.fr.srt").write_text("content")

        scanner = SubtitleScanner()
        result = scanner.scan_directory(temp_dir)

        assert len(result) == 2
        # Check first movie has 2 subtitles
        movie1_subs = result[str(temp_dir / "movie1.mp4")]
        assert len(movie1_subs) == 2

        # Check second movie has 1 subtitle
        movie2_subs = result[str(temp_dir / "movie2.mkv")]
        assert len(movie2_subs) == 1

    def test_scan_directory_recursive(self, temp_dir: Path) -> None:
        """Test scanning recursively finds media in subdirectories."""
        from submate.services.scanner import SubtitleScanner

        # Create nested structure
        subdir = temp_dir / "movies" / "action"
        subdir.mkdir(parents=True)

        (subdir / "action_movie.mp4").touch()
        (subdir / "action_movie.en.srt").write_text("content")

        scanner = SubtitleScanner()
        result = scanner.scan_directory(temp_dir)

        assert len(result) == 1
        assert str(subdir / "action_movie.mp4") in result

    def test_scan_directory_ignores_non_media(self, temp_dir: Path) -> None:
        """Test scanning ignores non-media files."""
        from submate.services.scanner import SubtitleScanner

        # Create various non-media files
        (temp_dir / "readme.txt").write_text("info")
        (temp_dir / "cover.jpg").write_bytes(b"\x00")
        (temp_dir / "data.json").write_text("{}")
        (temp_dir / "subtitles.srt").write_text("orphan subtitle")

        scanner = SubtitleScanner()
        result = scanner.scan_directory(temp_dir)

        assert result == {}


class TestLanguageCodes:
    """Tests for LANGUAGE_CODES constant."""

    def test_language_codes_contains_common_languages(self) -> None:
        """Test LANGUAGE_CODES has common language mappings."""
        from submate.services.scanner import LANGUAGE_CODES

        # ISO 639-1 codes
        assert LANGUAGE_CODES["en"] == "en"
        assert LANGUAGE_CODES["es"] == "es"
        assert LANGUAGE_CODES["fr"] == "fr"
        assert LANGUAGE_CODES["de"] == "de"

        # ISO 639-2 codes
        assert LANGUAGE_CODES["eng"] == "en"
        assert LANGUAGE_CODES["spa"] == "es"
        assert LANGUAGE_CODES["fra"] == "fr"
        assert LANGUAGE_CODES["deu"] == "de"

        # Full names
        assert LANGUAGE_CODES["english"] == "en"
        assert LANGUAGE_CODES["spanish"] == "es"
        assert LANGUAGE_CODES["french"] == "fr"
        assert LANGUAGE_CODES["german"] == "de"


class TestSubtitleExtensions:
    """Tests for SUBTITLE_EXTENSIONS constant."""

    def test_subtitle_extensions_contains_common_formats(self) -> None:
        """Test SUBTITLE_EXTENSIONS has common formats."""
        from submate.services.scanner import SUBTITLE_EXTENSIONS

        assert ".srt" in SUBTITLE_EXTENSIONS
        assert ".ass" in SUBTITLE_EXTENSIONS
        assert ".ssa" in SUBTITLE_EXTENSIONS
        assert ".sub" in SUBTITLE_EXTENSIONS
        assert ".vtt" in SUBTITLE_EXTENSIONS
