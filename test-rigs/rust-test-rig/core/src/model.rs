/// Priority level for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

impl Priority {
    pub fn is_urgent(&self) -> bool {
        matches!(self, Priority::High | Priority::Critical)
    }

    pub fn label(&self) -> &'static str {
        match self {
            Priority::Low => "Low",
            Priority::Medium => "Medium",
            Priority::High => "High",
            Priority::Critical => "Critical",
        }
    }
}

/// A task in the task management system.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub completed: bool,
    pub priority: Priority,
    pub tags: Vec<String>,
}

impl Task {
    pub fn new(id: u64, title: impl Into<String>, priority: Priority) -> Self {
        Self {
            id,
            title: title.into(),
            description: String::new(),
            completed: false,
            priority,
            tags: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn toggle_completed(&mut self) {
        self.completed = !self.completed;
    }

    pub fn matches_search(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.title.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
            || self.tags.iter().any(|t| t.to_lowercase().contains(&q))
    }

    pub fn is_actionable(&self) -> bool {
        !self.completed && self.priority.is_urgent()
    }
}

/// Result of an operation that may fail with a domain error.
pub type TaskResult<T> = Result<T, TaskError>;

#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    NotFound(u64),
    DuplicateTitle(String),
    ValidationError(String),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::NotFound(id) => write!(f, "Task not found: {id}"),
            TaskError::DuplicateTitle(t) => write!(f, "Duplicate title: {t}"),
            TaskError::ValidationError(msg) => write!(f, "Validation error: {msg}"),
        }
    }
}

impl std::error::Error for TaskError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_task_defaults() {
        let task = Task::new(1, "Write tests", Priority::Medium);
        assert_eq!(task.id, 1);
        assert_eq!(task.title, "Write tests");
        assert!(!task.completed);
        assert!(task.description.is_empty());
        assert!(task.tags.is_empty());
    }

    #[test]
    fn toggle_completed() {
        let mut task = Task::new(1, "Test", Priority::Low);
        assert!(!task.completed);
        task.toggle_completed();
        assert!(task.completed);
        task.toggle_completed();
        assert!(!task.completed);
    }

    #[test]
    fn matches_search_title() {
        let task = Task::new(1, "Fix login bug", Priority::High);
        assert!(task.matches_search("login"));
        assert!(task.matches_search("LOGIN"));
        assert!(!task.matches_search("signup"));
    }

    #[test]
    fn matches_search_description() {
        let task = Task::new(1, "Bug", Priority::High)
            .with_description("Users cannot login after password reset");
        assert!(task.matches_search("password"));
    }

    #[test]
    fn matches_search_tags() {
        let task = Task::new(1, "Bug", Priority::High)
            .with_tags(vec!["auth".into(), "urgent".into()]);
        assert!(task.matches_search("auth"));
        assert!(!task.matches_search("frontend"));
    }

    #[test]
    fn is_actionable() {
        let high = Task::new(1, "A", Priority::High);
        assert!(high.is_actionable());

        let critical = Task::new(2, "B", Priority::Critical);
        assert!(critical.is_actionable());

        let low = Task::new(3, "C", Priority::Low);
        assert!(!low.is_actionable());

        let mut done = Task::new(4, "D", Priority::High);
        done.toggle_completed();
        assert!(!done.is_actionable());
    }

    #[test]
    fn priority_labels() {
        assert_eq!(Priority::Low.label(), "Low");
        assert_eq!(Priority::Medium.label(), "Medium");
        assert_eq!(Priority::High.label(), "High");
        assert_eq!(Priority::Critical.label(), "Critical");
    }

    #[test]
    fn priority_is_urgent() {
        assert!(!Priority::Low.is_urgent());
        assert!(!Priority::Medium.is_urgent());
        assert!(Priority::High.is_urgent());
        assert!(Priority::Critical.is_urgent());
    }

    // Intentionally NOT testing: TaskError Display, with_description, with_tags builder
}
