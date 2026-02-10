"""Tests for translation service."""

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from pytest_mock import MockerFixture


class TestValidateAssTags:
    """Tests for validate_ass_tags function."""

    def test_validate_ass_tags_matching(self) -> None:
        """Test tag validation with matching tags."""
        from submate.translation import validate_ass_tags

        original = "{\\i1}Hello{\\i0} {\\b1}world{\\b0}"
        translated = "{\\i1}Hola{\\i0} {\\b1}mundo{\\b0}"

        assert validate_ass_tags(original, translated) is True

    def test_validate_ass_tags_mismatched(self) -> None:
        """Test tag validation with mismatched tags."""
        from submate.translation import validate_ass_tags

        original = "{\\i1}Hello{\\i0} world"
        translated = "{\\i1}Hola{\\i0} {\\b1}mundo{\\b0}"  # Extra tag

        assert validate_ass_tags(original, translated) is False

    def test_validate_ass_tags_no_tags(self) -> None:
        """Test tag validation with no tags in either string."""
        from submate.translation import validate_ass_tags

        original = "Hello world"
        translated = "Hola mundo"

        assert validate_ass_tags(original, translated) is True

    def test_validate_ass_tags_complex_tags(self) -> None:
        """Test tag validation with complex ASS tags."""
        from submate.translation import validate_ass_tags

        original = "{\\pos(320,240)}{\\an8}Hello{\\fad(100,200)}"
        translated = "{\\pos(320,240)}{\\an8}Hola{\\fad(100,200)}"

        assert validate_ass_tags(original, translated) is True

    def test_validate_ass_tags_missing_tag(self) -> None:
        """Test tag validation when translated is missing a tag."""
        from submate.translation import validate_ass_tags

        original = "{\\i1}Hello{\\i0}"
        translated = "Hola"  # Missing tags

        assert validate_ass_tags(original, translated) is False


class TestTranslationServiceASS:
    """Tests for ASS translation in TranslationService."""

    def test_translate_ass_content_preserves_structure(self, mocker: "MockerFixture") -> None:
        """Verify ASS translation preserves file structure."""
        from submate.translation import TranslationService

        # Mock config with nested attributes
        mock_translation_settings = mocker.MagicMock()
        mock_translation_settings.chunk_size = 50

        config = mocker.MagicMock()
        config.translation = mock_translation_settings

        # Create service with mocked backend
        service = TranslationService.__new__(TranslationService)
        service.config = config

        # Mock backend
        mock_backend = mocker.MagicMock()
        mock_backend.translate.return_value = "{\\i1}Hola{\\i0} mundo"
        service.backend = mock_backend

        # Test input
        ass_content = """[Script Info]
Title: Test
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize
Style: Default,Arial,48

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,{\\i1}Hello{\\i0} world
"""

        result = service.translate_ass_content(ass_content, "en", "es")

        # Should preserve ASS structure
        assert "[Script Info]" in result
        assert "[V4+ Styles]" in result
        assert "[Events]" in result
        assert "Dialogue:" in result

    def test_translate_ass_content_same_language(self, mocker: "MockerFixture") -> None:
        """Verify same language returns original content."""
        from submate.translation import TranslationService

        # Mock config with nested attributes
        mock_translation_settings = mocker.MagicMock()
        mock_translation_settings.chunk_size = 50

        config = mocker.MagicMock()
        config.translation = mock_translation_settings

        # Create service
        service = TranslationService.__new__(TranslationService)
        service.config = config
        service.backend = mocker.MagicMock()

        ass_content = "[Script Info]\nTitle: Test"
        result = service.translate_ass_content(ass_content, "en", "en")

        assert result == ass_content
        service.backend.translate.assert_not_called()

    def test_translate_ass_content_validates_tags(self, mocker: "MockerFixture") -> None:
        """Verify tag validation prevents corrupted translations."""
        from submate.translation import TranslationService

        # Mock config with nested attributes
        mock_translation_settings = mocker.MagicMock()
        mock_translation_settings.chunk_size = 50

        config = mocker.MagicMock()
        config.translation = mock_translation_settings

        # Create service
        service = TranslationService.__new__(TranslationService)
        service.config = config

        # Mock backend returns translation with wrong tags
        mock_backend = mocker.MagicMock()
        mock_backend.translate.return_value = "{\\b1}Hola{\\b0} mundo"  # Wrong tags
        service.backend = mock_backend

        ass_content = """[Script Info]
Title: Test
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize
Style: Default,Arial,48

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,{\\i1}Hello{\\i0} world
"""

        result = service.translate_ass_content(ass_content, "en", "es")

        # Original text should be kept due to tag mismatch
        assert "{\\i1}Hello{\\i0} world" in result


class TestASSTranslationPrompt:
    """Tests for ASS translation prompt."""

    def test_ass_prompt_preserves_tags_instruction(self) -> None:
        """Verify ASS prompt includes tag preservation rules."""
        from submate.translation import ASS_TRANSLATION_PROMPT

        assert "{\\i1}" in ASS_TRANSLATION_PROMPT or "{{\\i1}}" in ASS_TRANSLATION_PROMPT
        assert "PRESERVE" in ASS_TRANSLATION_PROMPT.upper()
        assert "tag" in ASS_TRANSLATION_PROMPT.lower()
