"""Tests for business logic use cases."""

import pytest

from taskmanager.model import NotFoundError, Priority
from taskmanager.repository import InMemoryTaskRepository
from taskmanager.usecase import add_task, get_stats, get_tasks


def test_add_task_basic():
    repo = InMemoryTaskRepository()
    task = add_task(repo, "Buy groceries")
    assert task.title == "Buy groceries"
    assert task.id == 1


def test_add_task_with_priority_and_tags():
    repo = InMemoryTaskRepository()
    task = add_task(repo, "Fix bug", priority=Priority.HIGH, tags=["bug"])
    assert task.priority == Priority.HIGH
    assert task.tags == ["bug"]


def test_add_task_validates_title():
    repo = InMemoryTaskRepository()
    with pytest.raises(ValueError):
        add_task(repo, "")


def test_get_tasks_all():
    repo = InMemoryTaskRepository()
    add_task(repo, "Task one")
    add_task(repo, "Task two")
    assert len(get_tasks(repo)) == 2


def test_get_tasks_filter_incomplete():
    repo = InMemoryTaskRepository()
    add_task(repo, "Task one")
    tasks = get_tasks(repo, completed=False)
    assert len(tasks) == 1
    assert not tasks[0].completed


def test_get_stats_empty():
    repo = InMemoryTaskRepository()
    stats = get_stats(repo)
    assert stats.total == 0
    assert stats.completed == 0


def test_get_stats_with_tasks():
    repo = InMemoryTaskRepository()
    add_task(repo, "Task one", priority=Priority.HIGH)
    add_task(repo, "Task two", priority=Priority.LOW)
    stats = get_stats(repo)
    assert stats.total == 2
    assert stats.pending == 2
    assert stats.by_priority[Priority.HIGH] == 1


# Intentionally NOT tested:
# - toggle_task()
# - remove_task()
# - get_tasks(completed=True)
# - get_tasks_by_priority()
