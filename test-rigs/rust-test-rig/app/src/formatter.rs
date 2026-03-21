use task_core::model::{Priority, Task};
use task_core::usecase::TaskStats;

/// Format a task as a single-line summary.
pub fn format_task_line(task: &Task) -> String {
    let status = if task.completed { "✓" } else { "○" };
    let priority_badge = match task.priority {
        Priority::Critical => "[!!!]",
        Priority::High => "[!!]",
        Priority::Medium => "[!]",
        Priority::Low => "[ ]",
    };
    let tags = if task.tags.is_empty() {
        String::new()
    } else {
        format!(" ({})", task.tags.join(", "))
    };
    format!("{status} {priority_badge} {}{tags}", task.title)
}

/// Format a list of tasks as a report.
pub fn format_task_list(tasks: &[Task]) -> String {
    if tasks.is_empty() {
        return "No tasks found.".to_string();
    }
    tasks
        .iter()
        .map(format_task_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Format stats as a summary string.
pub fn format_stats(stats: &TaskStats) -> String {
    format!(
        "Tasks: {} total, {} completed, {} pending ({:.0}% done)",
        stats.total,
        stats.completed,
        stats.pending,
        stats.completion_rate * 100.0
    )
}

/// Format a priority distribution table.
pub fn format_priority_distribution(groups: &[(Priority, Vec<Task>)]) -> String {
    if groups.is_empty() {
        return "No tasks.".to_string();
    }
    groups
        .iter()
        .map(|(priority, tasks)| format!("{}: {} task(s)", priority.label(), tasks.len()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use task_core::model::Priority;

    #[test]
    fn format_incomplete_task() {
        let task = Task::new(1, "Write docs", Priority::Medium);
        let line = format_task_line(&task);
        assert!(line.contains("○"));
        assert!(line.contains("[!]"));
        assert!(line.contains("Write docs"));
    }

    #[test]
    fn format_completed_task() {
        let mut task = Task::new(1, "Done", Priority::Low);
        task.toggle_completed();
        let line = format_task_line(&task);
        assert!(line.contains("✓"));
    }

    #[test]
    fn format_task_with_tags() {
        let task = Task::new(1, "Task", Priority::High).with_tags(vec!["bug".into(), "ui".into()]);
        let line = format_task_line(&task);
        assert!(line.contains("(bug, ui)"));
    }

    #[test]
    fn format_empty_list() {
        assert_eq!(format_task_list(&[]), "No tasks found.");
    }

    #[test]
    fn format_stats_display() {
        let stats = TaskStats {
            total: 10,
            completed: 7,
            pending: 3,
            actionable: 2,
            completion_rate: 0.7,
        };
        let output = format_stats(&stats);
        assert!(output.contains("10 total"));
        assert!(output.contains("70% done"));
    }

    // Intentionally NOT testing: format_task_list with items, format_priority_distribution
}
