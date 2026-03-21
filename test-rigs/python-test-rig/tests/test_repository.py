"""Tests for the task repository."""

from taskmanager.model import Priority, Task
from taskmanager.repository import InMemoryTaskRepository


def test_add_and_get():
    repo = InMemoryTaskRepository()
    task = Task(id=0, title="Test task")
    stored = repo.add(task)
    assert stored.id == 1
    assert repo.get(1) is not None


def test_add_assigns_sequential_ids():
    repo = InMemoryTaskRepository()
    t1 = repo.add(Task(id=0, title="First"))
    t2 = repo.add(Task(id=0, title="Second"))
    assert t1.id == 1
    assert t2.id == 2


def test_get_all():
    repo = InMemoryTaskRepository()
    repo.add(Task(id=0, title="A"))
    repo.add(Task(id=0, title="B"))
    assert len(repo.get_all()) == 2


def test_get_missing_returns_none():
    repo = InMemoryTaskRepository()
    assert repo.get(999) is None


def test_add_preserves_fields():
    repo = InMemoryTaskRepository()
    task = Task(id=0, title="Important", priority=Priority.HIGH, tags=["urgent"])
    stored = repo.add(task)
    assert stored.priority == Priority.HIGH
    assert stored.tags == ["urgent"]


# Intentionally NOT tested:
# - update()
# - remove()
# - find_by_tag()
# - get_by_priority()
