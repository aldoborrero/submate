"""Tests for language code data integrity."""

from submate.language import LanguageCode


def test_armenian_native_name_is_correct():
    """The native name must be the Armenian endonym, not a corrupted literal."""
    assert LanguageCode.ARMENIAN.value[4] == "Հայերեն"
