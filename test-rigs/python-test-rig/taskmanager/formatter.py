"""Text formatting utilities for task display."""

from .model import Priority, Task
from .usecase import TaskStats


def format_task_line(task: Task) -> str:
    """Format a single task as a one-line string."""
    status = "[x]" if task.completed else "[ ]"
    priority_label = task.priority.value.upper()
    tags_str = ""
    if task.tags:
        tags_str = " " + " ".join(f"#{t}" for t in task.tags)
    return f"{status} [{priority_label}] {task.title}{tags_str}"


def format_task_list(tasks: list[Task]) -> str:
    """Format a list of tasks as a multi-line string."""
    if not tasks:
        return "No tasks."
    lines = [format_task_line(t) for t in tasks]
    return "\n".join(lines)


def format_stats(stats: TaskStats) -> str:
    """Format task statistics as a summary string."""
    if stats.total == 0:
        return "No tasks tracked."
    pct = (stats.completed / stats.total) * 100
    return (
        f"Tasks: {stats.total} total, {stats.completed} completed, "
        f"{stats.pending} pending ({pct:.1f}% done)"
    )


def format_priority_distribution(stats: TaskStats) -> str:
    """Format priority distribution as a multi-line string."""
    if stats.total == 0:
        return "No tasks to analyze."
    lines = []
    for priority in Priority:
        count = stats.by_priority.get(priority, 0)
        pct = (count / stats.total) * 100
        bar = "#" * int(pct / 5)
        lines.append(f"  {priority.value:>8}: {bar} {count} ({pct:.0f}%)")
    return "\n".join(lines)
