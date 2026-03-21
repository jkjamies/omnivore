"""Task repository interface and in-memory implementation."""

from abc import ABC, abstractmethod
from threading import Lock
from typing import Optional

from .model import NotFoundError, Priority, Task


class TaskRepository(ABC):
    """Abstract repository for task persistence."""

    @abstractmethod
    def add(self, task: Task) -> Task:
        ...

    @abstractmethod
    def get(self, task_id: int) -> Optional[Task]:
        ...

    @abstractmethod
    def get_all(self) -> list[Task]:
        ...

    @abstractmethod
    def update(self, task: Task) -> Task:
        ...

    @abstractmethod
    def remove(self, task_id: int) -> bool:
        ...

    @abstractmethod
    def find_by_tag(self, tag: str) -> list[Task]:
        ...

    @abstractmethod
    def get_by_priority(self, priority: Priority) -> list[Task]:
        ...


class InMemoryTaskRepository(TaskRepository):
    """Thread-safe in-memory task repository."""

    def __init__(self) -> None:
        self._tasks: dict[int, Task] = {}
        self._next_id = 1
        self._lock = Lock()

    def add(self, task: Task) -> Task:
        with self._lock:
            stored = Task(
                id=self._next_id,
                title=task.title,
                description=task.description,
                priority=task.priority,
                completed=task.completed,
                tags=list(task.tags),
                created_at=task.created_at,
                completed_at=task.completed_at,
            )
            self._tasks[self._next_id] = stored
            self._next_id += 1
            return stored

    def get(self, task_id: int) -> Optional[Task]:
        with self._lock:
            return self._tasks.get(task_id)

    def get_all(self) -> list[Task]:
        with self._lock:
            return list(self._tasks.values())

    def update(self, task: Task) -> Task:
        with self._lock:
            if task.id not in self._tasks:
                raise NotFoundError(f"Task {task.id} not found")
            self._tasks[task.id] = task
            return task

    def remove(self, task_id: int) -> bool:
        with self._lock:
            if task_id in self._tasks:
                del self._tasks[task_id]
                return True
            return False

    def find_by_tag(self, tag: str) -> list[Task]:
        with self._lock:
            return [t for t in self._tasks.values() if tag in t.tags]

    def get_by_priority(self, priority: Priority) -> list[Task]:
        with self._lock:
            return [t for t in self._tasks.values() if t.priority == priority]
