"""Domain model for task management."""

from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
from typing import Optional


class Priority(Enum):
    LOW = "low"
    MEDIUM = "medium"
    HIGH = "high"
    CRITICAL = "critical"


class TaskError(Exception):
    """Base exception for task operations."""

    pass


class ValidationError(TaskError):
    """Raised when task data fails validation."""

    pass


class NotFoundError(TaskError):
    """Raised when a task is not found."""

    pass


@dataclass
class Task:
    id: int
    title: str
    description: str = ""
    priority: Priority = Priority.MEDIUM
    completed: bool = False
    tags: list[str] = field(default_factory=list)
    created_at: datetime = field(default_factory=datetime.now)
    completed_at: Optional[datetime] = None

    def toggle(self) -> "Task":
        """Toggle completion status."""
        if self.completed:
            return Task(
                id=self.id,
                title=self.title,
                description=self.description,
                priority=self.priority,
                completed=False,
                tags=self.tags,
                created_at=self.created_at,
                completed_at=None,
            )
        else:
            return Task(
                id=self.id,
                title=self.title,
                description=self.description,
                priority=self.priority,
                completed=True,
                tags=self.tags,
                created_at=self.created_at,
                completed_at=datetime.now(),
            )

    def is_overdue(self, deadline: datetime) -> bool:
        """Check if task is past deadline and not completed."""
        if self.completed:
            return False
        return datetime.now() > deadline

    def matches_search(self, query: str) -> bool:
        """Check if task matches a search query."""
        q = query.lower()
        if q in self.title.lower():
            return True
        if q in self.description.lower():
            return True
        return any(q in tag.lower() for tag in self.tags)

    def is_actionable(self) -> bool:
        """Check if task can be acted upon."""
        return not self.completed and self.priority != Priority.LOW
