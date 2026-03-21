use crate::model::{Priority, Task, TaskResult};
use crate::repository::TaskRepository;
use crate::validation;

/// Add a new task with validation.
pub fn add_task(
    repo: &mut dyn TaskRepository,
    id: u64,
    title: &str,
    priority: Priority,
    description: Option<&str>,
    tags: Vec<String>,
) -> TaskResult<Task> {
    validation::validate_title(title)?;
    for tag in &tags {
        validation::validate_tag(tag)?;
    }

    let desc = description
        .map(validation::sanitize_description)
        .unwrap_or_default();

    let task = Task::new(id, title, priority)
        .with_description(desc)
        .with_tags(tags);

    repo.add(task.clone())?;
    Ok(task)
}

/// Toggle a task's completion status.
pub fn toggle_task(repo: &mut dyn TaskRepository, id: u64) -> TaskResult<Task> {
    let mut task = repo.get_by_id(id)?;
    task.toggle_completed();
    repo.update(task.clone())?;
    Ok(task)
}

/// Remove a task by ID.
pub fn remove_task(repo: &mut dyn TaskRepository, id: u64) -> TaskResult<Task> {
    repo.remove(id)
}

/// Get all tasks, optionally filtered by a search query.
pub fn get_tasks(repo: &dyn TaskRepository, query: Option<&str>) -> Vec<Task> {
    let all = repo.get_all();
    match query {
        Some(q) if !q.is_empty() => all.into_iter().filter(|t| t.matches_search(q)).collect(),
        _ => all,
    }
}

/// Get tasks grouped by priority.
pub fn get_tasks_by_priority(repo: &dyn TaskRepository) -> Vec<(Priority, Vec<Task>)> {
    let all = repo.get_all();
    let priorities = [
        Priority::Critical,
        Priority::High,
        Priority::Medium,
        Priority::Low,
    ];

    priorities
        .into_iter()
        .map(|p| {
            let tasks: Vec<Task> = all.iter().filter(|t| t.priority == p).cloned().collect();
            (p, tasks)
        })
        .filter(|(_, tasks)| !tasks.is_empty())
        .collect()
}

/// Get summary statistics.
pub fn get_stats(repo: &dyn TaskRepository) -> TaskStats {
    let all = repo.get_all();
    let total = all.len();
    let completed = all.iter().filter(|t| t.completed).count();
    let actionable = all.iter().filter(|t| t.is_actionable()).count();
    TaskStats {
        total,
        completed,
        pending: total - completed,
        actionable,
        completion_rate: if total > 0 {
            completed as f64 / total as f64
        } else {
            0.0
        },
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskStats {
    pub total: usize,
    pub completed: usize,
    pub pending: usize,
    pub actionable: usize,
    pub completion_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::InMemoryTaskRepository;

    #[test]
    fn add_task_success() {
        let mut repo = InMemoryTaskRepository::new();
        let task = add_task(&mut repo, 1, "New task", Priority::Medium, None, vec![]).unwrap();
        assert_eq!(task.title, "New task");
        assert_eq!(repo.count(), 1);
    }

    #[test]
    fn add_task_with_description_and_tags() {
        let mut repo = InMemoryTaskRepository::new();
        let task = add_task(
            &mut repo,
            1,
            "Tagged task",
            Priority::High,
            Some("A  detailed   description"),
            vec!["backend".into(), "urgent".into()],
        )
        .unwrap();
        assert_eq!(task.description, "A detailed description");
        assert_eq!(task.tags.len(), 2);
    }

    #[test]
    fn add_task_invalid_title() {
        let mut repo = InMemoryTaskRepository::new();
        assert!(add_task(&mut repo, 1, "", Priority::Low, None, vec![]).is_err());
    }

    #[test]
    fn add_task_invalid_tag() {
        let mut repo = InMemoryTaskRepository::new();
        assert!(add_task(
            &mut repo,
            1,
            "Task",
            Priority::Low,
            None,
            vec!["bad tag".into()]
        )
        .is_err());
    }

    #[test]
    fn get_tasks_all() {
        let mut repo = InMemoryTaskRepository::new();
        add_task(&mut repo, 1, "Alpha", Priority::Low, None, vec![]).unwrap();
        add_task(&mut repo, 2, "Beta", Priority::High, None, vec![]).unwrap();
        assert_eq!(get_tasks(&repo, None).len(), 2);
    }

    #[test]
    fn get_tasks_filtered() {
        let mut repo = InMemoryTaskRepository::new();
        add_task(&mut repo, 1, "Fix login", Priority::High, None, vec![]).unwrap();
        add_task(&mut repo, 2, "Add signup", Priority::Medium, None, vec![]).unwrap();
        let results = get_tasks(&repo, Some("login"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Fix login");
    }

    #[test]
    fn get_stats_empty() {
        let repo = InMemoryTaskRepository::new();
        let stats = get_stats(&repo);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.completion_rate, 0.0);
    }

    #[test]
    fn get_stats_mixed() {
        let mut repo = InMemoryTaskRepository::new();
        add_task(&mut repo, 1, "Done", Priority::Low, None, vec![]).unwrap();
        toggle_task(&mut repo, 1).unwrap();
        add_task(&mut repo, 2, "Pending", Priority::High, None, vec![]).unwrap();
        let stats = get_stats(&repo);
        assert_eq!(stats.total, 2);
        assert_eq!(stats.completed, 1);
        assert_eq!(stats.pending, 1);
        assert_eq!(stats.actionable, 1);
        assert_eq!(stats.completion_rate, 0.5);
    }

    // Intentionally NOT testing: toggle_task, remove_task, get_tasks_by_priority
}
