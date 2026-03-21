"""Tests for the task domain model."""

from taskmanager.model import Priority, Task


def test_create_task():
    task = Task(id=1, title="Test task")
    assert task.id == 1
    assert task.title == "Test task"
    assert task.priority == Priority.MEDIUM
    assert not task.completed


def test_create_task_with_priority():
    task = Task(id=1, title="Urgent", priority=Priority.CRITICAL)
    assert task.priority == Priority.CRITICAL


def test_create_task_with_tags():
    task = Task(id=1, title="Tagged", tags=["bug", "frontend"])
    assert len(task.tags) == 2
    assert "bug" in task.tags


def test_toggle_incomplete_to_complete():
    task = Task(id=1, title="Todo")
    toggled = task.toggle()
    assert toggled.completed
    assert toggled.completed_at is not None


def test_toggle_complete_to_incomplete():
    task = Task(id=1, title="Done", completed=True)
    toggled = task.toggle()
    assert not toggled.completed
    assert toggled.completed_at is None


def test_matches_search_title():
    task = Task(id=1, title="Fix login bug")
    assert task.matches_search("login")
    assert not task.matches_search("signup")


def test_matches_search_case_insensitive():
    task = Task(id=1, title="Fix LOGIN Bug")
    assert task.matches_search("login")


def test_priority_values():
    assert Priority.LOW.value == "low"
    assert Priority.CRITICAL.value == "critical"


# Intentionally NOT tested:
# - is_overdue()
# - matches_search() with description and tags
# - is_actionable()
