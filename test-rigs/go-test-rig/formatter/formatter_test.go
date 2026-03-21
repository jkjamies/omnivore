package formatter

import (
	"strings"
	"testing"

	"github.com/jkjamies/omnivore/go-test-rig/model"
	"github.com/jkjamies/omnivore/go-test-rig/usecase"
)

func TestFormatIncompleteTask(t *testing.T) {
	task := model.NewTask(1, "Write docs", model.PriorityMedium)
	line := FormatTaskLine(task)
	if !strings.Contains(line, "○") {
		t.Error("expected incomplete marker")
	}
	if !strings.Contains(line, "[!]") {
		t.Error("expected medium priority badge")
	}
	if !strings.Contains(line, "Write docs") {
		t.Error("expected title in output")
	}
}

func TestFormatCompletedTask(t *testing.T) {
	task := model.NewTask(1, "Done", model.PriorityLow)
	task.ToggleCompleted()
	line := FormatTaskLine(task)
	if !strings.Contains(line, "✓") {
		t.Error("expected completed marker")
	}
}

func TestFormatTaskWithTags(t *testing.T) {
	task := model.NewTask(1, "Task", model.PriorityHigh)
	task.Tags = []string{"bug", "ui"}
	line := FormatTaskLine(task)
	if !strings.Contains(line, "(bug, ui)") {
		t.Error("expected tags in output")
	}
}

func TestFormatEmptyList(t *testing.T) {
	result := FormatTaskList(nil)
	if result != "No tasks found." {
		t.Errorf("expected 'No tasks found.', got '%s'", result)
	}
}

func TestFormatStats(t *testing.T) {
	stats := usecase.TaskStats{
		Total:          10,
		Completed:      7,
		Pending:        3,
		Actionable:     2,
		CompletionRate: 0.7,
	}
	output := FormatStats(stats)
	if !strings.Contains(output, "10 total") {
		t.Error("expected total in output")
	}
	if !strings.Contains(output, "70% done") {
		t.Error("expected completion rate in output")
	}
}

// Intentionally NOT testing: FormatTaskList with items, FormatPriorityDistribution
