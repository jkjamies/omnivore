"""Tests for input validation."""

import pytest

from taskmanager.validation import sanitize_description, validate_tag, validate_title


def test_validate_title_strips_whitespace():
    assert validate_title("  Hello  ") == "Hello"


def test_validate_title_empty_raises():
    with pytest.raises(ValueError, match="empty"):
        validate_title("")


def test_validate_title_whitespace_only_raises():
    with pytest.raises(ValueError, match="empty"):
        validate_title("   ")


def test_validate_title_too_short_raises():
    with pytest.raises(ValueError, match="at least 3"):
        validate_title("ab")


def test_validate_tag_normalizes():
    assert validate_tag("  BUG  ") == "bug"


def test_validate_tag_empty_raises():
    with pytest.raises(ValueError, match="empty"):
        validate_tag("")


def test_validate_tag_invalid_chars_raises():
    with pytest.raises(ValueError, match="alphanumeric"):
        validate_tag("not valid!")


def test_sanitize_description_collapses_spaces():
    result = sanitize_description("  hello   world  ")
    assert result == "hello world"


# Intentionally NOT tested:
# - validate_title() with > 200 chars
# - validate_tag() with > 50 chars
# - sanitize_description() truncation at 2000 chars
