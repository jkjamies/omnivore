"""Business logic use cases for task management."""

from dataclasses import dataclass

from .model import NotFoundError, Priority, Task, ValidationError
from .repository import TaskRepository
from .validation import sanitize_description, validate_tag, validate_title


def add_task(
    repo: TaskRepository,
    title: str,
    description: str = "",
    priority: Priority = Priority.MEDIUM,
    tags: list[str] | None = None,
) -> Task:
    """Create and store a new task."""
    clean_title = validate_title(title)
    clean_desc = sanitize_description(description)
    clean_tags = [validate_tag(t) for t in (tags or [])]

    task = Task(
        id=0,
        title=clean_title,
        description=clean_desc,
        priority=priority,
        tags=clean_tags,
    )
    return repo.add(task)


def toggle_task(repo: TaskRepository, task_id: int) -> Task:
    """Toggle a task's completion status."""
    task = repo.get(task_id)
    if task is None:
        raise NotFoundError(f"Task {task_id} not found")
    toggled = task.toggle()
    return repo.update(toggled)


def remove_task(repo: TaskRepository, task_id: int) -> bool:
    """Remove a task by ID."""
    if repo.get(task_id) is None:
        raise NotFoundError(f"Task {task_id} not found")
    return repo.remove(task_id)


def get_tasks(
    repo: TaskRepository,
    completed: bool | None = None,
) -> list[Task]:
    """Get tasks, optionally filtered by completion status."""
    tasks = repo.get_all()
    if completed is not None:
        tasks = [t for t in tasks if t.completed == completed]
    return tasks


def get_tasks_by_priority(
    repo: TaskRepository, priority: Priority
) -> list[Task]:
    """Get tasks filtered by priority."""
    return repo.get_by_priority(priority)


@dataclass
class TaskStats:
    total: int
    completed: int
    pending: int
    by_priority: dict[Priority, int]


def get_stats(repo: TaskRepository) -> TaskStats:
    """Calculate aggregate task statistics."""
    tasks = repo.get_all()
    total = len(tasks)
    completed = sum(1 for t in tasks if t.completed)
    pending = total - completed

    by_priority: dict[Priority, int] = {}
    for t in tasks:
        by_priority[t.priority] = by_priority.get(t.priority, 0) + 1

    return TaskStats(
        total=total,
        completed=completed,
        pending=pending,
        by_priority=by_priority,
    )
