"""Tests for text formatting utilities."""

from taskmanager.formatter import format_stats, format_task_line, format_task_list
from taskmanager.model import Priority, Task
from taskmanager.usecase import TaskStats


def test_format_task_line_incomplete():
    task = Task(id=1, title="Buy milk", priority=Priority.MEDIUM)
    result = format_task_line(task)
    assert result == "[ ] [MEDIUM] Buy milk"


def test_format_task_line_complete():
    task = Task(id=1, title="Done", priority=Priority.LOW, completed=True)
    result = format_task_line(task)
    assert "[x]" in result


def test_format_task_line_with_tags():
    task = Task(id=1, title="Fix bug", tags=["bug", "urgent"])
    result = format_task_line(task)
    assert "#bug" in result
    assert "#urgent" in result


def test_format_task_list_empty():
    assert format_task_list([]) == "No tasks."


def test_format_stats_with_tasks():
    stats = TaskStats(
        total=10, completed=7, pending=3, by_priority={Priority.HIGH: 5, Priority.LOW: 5}
    )
    result = format_stats(stats)
    assert "10 total" in result
    assert "70.0%" in result


# Intentionally NOT tested:
# - format_stats() with empty stats
# - format_priority_distribution()
# - format_task_list() with multiple tasks
