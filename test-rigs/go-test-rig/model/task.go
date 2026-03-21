package model

import "strings"

// Priority represents the urgency level of a task.
type Priority int

const (
	PriorityLow Priority = iota
	PriorityMedium
	PriorityHigh
	PriorityCritical
)

// String returns the display name of the priority.
func (p Priority) String() string {
	switch p {
	case PriorityLow:
		return "Low"
	case PriorityMedium:
		return "Medium"
	case PriorityHigh:
		return "High"
	case PriorityCritical:
		return "Critical"
	default:
		return "Unknown"
	}
}

// IsUrgent returns true for High and Critical priorities.
func (p Priority) IsUrgent() bool {
	return p == PriorityHigh || p == PriorityCritical
}

// Task represents a task in the system.
type Task struct {
	ID          uint64
	Title       string
	Description string
	Completed   bool
	Priority    Priority
	Tags        []string
}

// NewTask creates a new task with the given title and priority.
func NewTask(id uint64, title string, priority Priority) *Task {
	return &Task{
		ID:       id,
		Title:    title,
		Priority: priority,
		Tags:     []string{},
	}
}

// ToggleCompleted flips the completion status.
func (t *Task) ToggleCompleted() {
	t.Completed = !t.Completed
}

// MatchesSearch returns true if the task matches the search query
// against title, description, or tags (case-insensitive).
func (t *Task) MatchesSearch(query string) bool {
	q := strings.ToLower(query)
	if strings.Contains(strings.ToLower(t.Title), q) {
		return true
	}
	if strings.Contains(strings.ToLower(t.Description), q) {
		return true
	}
	for _, tag := range t.Tags {
		if strings.Contains(strings.ToLower(tag), q) {
			return true
		}
	}
	return false
}

// IsActionable returns true if the task is incomplete and urgent.
func (t *Task) IsActionable() bool {
	return !t.Completed && t.Priority.IsUrgent()
}

// TaskError represents a domain error.
type TaskError struct {
	Code    string
	Message string
}

func (e *TaskError) Error() string {
	return e.Message
}

// ErrNotFound creates a not-found error.
func ErrNotFound(id uint64) *TaskError {
	return &TaskError{Code: "not_found", Message: "task not found"}
}

// ErrDuplicateTitle creates a duplicate title error.
func ErrDuplicateTitle(title string) *TaskError {
	return &TaskError{Code: "duplicate_title", Message: "duplicate title: " + title}
}

// ErrValidation creates a validation error.
func ErrValidation(msg string) *TaskError {
	return &TaskError{Code: "validation", Message: msg}
}
