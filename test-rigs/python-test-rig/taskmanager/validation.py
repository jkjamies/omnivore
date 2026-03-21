"""Input validation for task data."""

import re


def validate_title(title: str) -> str:
    """Validate and normalize a task title.

    Raises ValueError for empty, whitespace-only, or oversized titles.
    """
    stripped = title.strip()
    if not stripped:
        raise ValueError("Title cannot be empty")
    if len(stripped) > 200:
        raise ValueError("Title cannot exceed 200 characters")
    if len(stripped) < 3:
        raise ValueError("Title must be at least 3 characters")
    return stripped


def validate_tag(tag: str) -> str:
    """Validate and normalize a tag.

    Tags must be alphanumeric with hyphens, 1-50 characters.
    """
    stripped = tag.strip().lower()
    if not stripped:
        raise ValueError("Tag cannot be empty")
    if len(stripped) > 50:
        raise ValueError("Tag cannot exceed 50 characters")
    if not re.match(r"^[a-z0-9][a-z0-9-]*$", stripped):
        raise ValueError("Tag must be alphanumeric with hyphens")
    return stripped


def sanitize_description(description: str) -> str:
    """Sanitize a task description.

    Strips leading/trailing whitespace, collapses multiple spaces,
    and limits to 2000 characters.
    """
    if not description:
        return ""
    cleaned = re.sub(r"\s+", " ", description.strip())
    if len(cleaned) > 2000:
        cleaned = cleaned[:2000]
    return cleaned
