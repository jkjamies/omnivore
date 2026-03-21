package formatter

import (
	"fmt"
	"strings"

	"github.com/jkjamies/omnivore/go-test-rig/model"
	"github.com/jkjamies/omnivore/go-test-rig/usecase"
)

// FormatTaskLine returns a single-line summary of a task.
func FormatTaskLine(task *model.Task) string {
	status := "○"
	if task.Completed {
		status = "✓"
	}

	var badge string
	switch task.Priority {
	case model.PriorityCritical:
		badge = "[!!!]"
	case model.PriorityHigh:
		badge = "[!!]"
	case model.PriorityMedium:
		badge = "[!]"
	default:
		badge = "[ ]"
	}

	tags := ""
	if len(task.Tags) > 0 {
		tags = fmt.Sprintf(" (%s)", strings.Join(task.Tags, ", "))
	}

	return fmt.Sprintf("%s %s %s%s", status, badge, task.Title, tags)
}

// FormatTaskList formats a slice of tasks as a multi-line report.
func FormatTaskList(tasks []*model.Task) string {
	if len(tasks) == 0 {
		return "No tasks found."
	}
	lines := make([]string, len(tasks))
	for i, t := range tasks {
		lines[i] = FormatTaskLine(t)
	}
	return strings.Join(lines, "\n")
}

// FormatStats formats task statistics as a summary string.
func FormatStats(stats usecase.TaskStats) string {
	return fmt.Sprintf("Tasks: %d total, %d completed, %d pending (%.0f%% done)",
		stats.Total, stats.Completed, stats.Pending, stats.CompletionRate*100)
}

// FormatPriorityDistribution formats priority groups as a table.
func FormatPriorityDistribution(groups []usecase.PriorityGroup) string {
	if len(groups) == 0 {
		return "No tasks."
	}
	lines := make([]string, len(groups))
	for i, g := range groups {
		lines[i] = fmt.Sprintf("%s: %d task(s)", g.Priority.String(), len(g.Tasks))
	}
	return strings.Join(lines, "\n")
}
