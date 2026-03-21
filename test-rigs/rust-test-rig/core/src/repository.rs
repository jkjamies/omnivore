use crate::model::{Task, TaskError, TaskResult};
use std::collections::HashMap;

/// Trait for task persistence.
pub trait TaskRepository {
    fn get_all(&self) -> Vec<Task>;
    fn get_by_id(&self, id: u64) -> TaskResult<Task>;
    fn add(&mut self, task: Task) -> TaskResult<()>;
    fn update(&mut self, task: Task) -> TaskResult<()>;
    fn remove(&mut self, id: u64) -> TaskResult<Task>;
    fn find_by_tag(&self, tag: &str) -> Vec<Task>;
    fn count(&self) -> usize;
}

/// In-memory implementation for testing.
pub struct InMemoryTaskRepository {
    tasks: HashMap<u64, Task>,
    next_id: u64,
}

impl InMemoryTaskRepository {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

impl Default for InMemoryTaskRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskRepository for InMemoryTaskRepository {
    fn get_all(&self) -> Vec<Task> {
        let mut tasks: Vec<Task> = self.tasks.values().cloned().collect();
        tasks.sort_by_key(|t| t.id);
        tasks
    }

    fn get_by_id(&self, id: u64) -> TaskResult<Task> {
        self.tasks
            .get(&id)
            .cloned()
            .ok_or(TaskError::NotFound(id))
    }

    fn add(&mut self, task: Task) -> TaskResult<()> {
        if self
            .tasks
            .values()
            .any(|t| t.title.to_lowercase() == task.title.to_lowercase())
        {
            return Err(TaskError::DuplicateTitle(task.title));
        }
        self.tasks.insert(task.id, task);
        Ok(())
    }

    fn update(&mut self, task: Task) -> TaskResult<()> {
        if !self.tasks.contains_key(&task.id) {
            return Err(TaskError::NotFound(task.id));
        }
        self.tasks.insert(task.id, task);
        Ok(())
    }

    fn remove(&mut self, id: u64) -> TaskResult<Task> {
        self.tasks.remove(&id).ok_or(TaskError::NotFound(id))
    }

    fn find_by_tag(&self, tag: &str) -> Vec<Task> {
        let tag_lower = tag.to_lowercase();
        self.tasks
            .values()
            .filter(|t| t.tags.iter().any(|t| t.to_lowercase() == tag_lower))
            .cloned()
            .collect()
    }

    fn count(&self) -> usize {
        self.tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Priority;

    #[test]
    fn add_and_get() {
        let mut repo = InMemoryTaskRepository::new();
        let task = Task::new(1, "Test task", Priority::Medium);
        repo.add(task.clone()).unwrap();
        assert_eq!(repo.get_by_id(1).unwrap(), task);
    }

    #[test]
    fn get_nonexistent_returns_error() {
        let repo = InMemoryTaskRepository::new();
        assert!(repo.get_by_id(999).is_err());
    }

    #[test]
    fn duplicate_title_rejected() {
        let mut repo = InMemoryTaskRepository::new();
        repo.add(Task::new(1, "Title", Priority::Low)).unwrap();
        assert!(repo.add(Task::new(2, "title", Priority::High)).is_err());
    }

    #[test]
    fn get_all_sorted() {
        let mut repo = InMemoryTaskRepository::new();
        repo.add(Task::new(3, "Third", Priority::Low)).unwrap();
        repo.add(Task::new(1, "First", Priority::Low)).unwrap();
        repo.add(Task::new(2, "Second", Priority::Low)).unwrap();
        let all = repo.get_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, 1);
        assert_eq!(all[2].id, 3);
    }

    #[test]
    fn count_tracks_size() {
        let mut repo = InMemoryTaskRepository::new();
        assert_eq!(repo.count(), 0);
        repo.add(Task::new(1, "A", Priority::Low)).unwrap();
        assert_eq!(repo.count(), 1);
    }

    // Intentionally NOT testing: update, remove, find_by_tag, next_id
}
