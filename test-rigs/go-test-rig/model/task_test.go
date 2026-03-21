package model

import "testing"

func TestNewTask(t *testing.T) {
	task := NewTask(1, "Write tests", PriorityMedium)
	if task.ID != 1 {
		t.Errorf("expected ID 1, got %d", task.ID)
	}
	if task.Title != "Write tests" {
		t.Errorf("expected title 'Write tests', got '%s'", task.Title)
	}
	if task.Completed {
		t.Error("new task should not be completed")
	}
	if task.Description != "" {
		t.Error("description should be empty by default")
	}
}

func TestToggleCompleted(t *testing.T) {
	task := NewTask(1, "Test", PriorityLow)
	if task.Completed {
		t.Fatal("should start incomplete")
	}
	task.ToggleCompleted()
	if !task.Completed {
		t.Fatal("should be completed after toggle")
	}
	task.ToggleCompleted()
	if task.Completed {
		t.Fatal("should be incomplete after second toggle")
	}
}

func TestMatchesSearchTitle(t *testing.T) {
	task := NewTask(1, "Fix login bug", PriorityHigh)
	if !task.MatchesSearch("login") {
		t.Error("should match title")
	}
	if !task.MatchesSearch("LOGIN") {
		t.Error("should be case-insensitive")
	}
	if task.MatchesSearch("signup") {
		t.Error("should not match unrelated query")
	}
}

func TestMatchesSearchDescription(t *testing.T) {
	task := NewTask(1, "Bug", PriorityHigh)
	task.Description = "Users cannot login after password reset"
	if !task.MatchesSearch("password") {
		t.Error("should match description")
	}
}

func TestMatchesSearchTags(t *testing.T) {
	task := NewTask(1, "Bug", PriorityHigh)
	task.Tags = []string{"auth", "urgent"}
	if !task.MatchesSearch("auth") {
		t.Error("should match tag")
	}
	if task.MatchesSearch("frontend") {
		t.Error("should not match unrelated tag")
	}
}

func TestIsActionable(t *testing.T) {
	high := NewTask(1, "A", PriorityHigh)
	if !high.IsActionable() {
		t.Error("high priority incomplete task should be actionable")
	}

	low := NewTask(2, "B", PriorityLow)
	if low.IsActionable() {
		t.Error("low priority task should not be actionable")
	}

	done := NewTask(3, "C", PriorityCritical)
	done.ToggleCompleted()
	if done.IsActionable() {
		t.Error("completed task should not be actionable")
	}
}

func TestPriorityString(t *testing.T) {
	tests := []struct {
		p    Priority
		want string
	}{
		{PriorityLow, "Low"},
		{PriorityMedium, "Medium"},
		{PriorityHigh, "High"},
		{PriorityCritical, "Critical"},
	}
	for _, tc := range tests {
		if got := tc.p.String(); got != tc.want {
			t.Errorf("Priority(%d).String() = %q, want %q", tc.p, got, tc.want)
		}
	}
}

func TestPriorityIsUrgent(t *testing.T) {
	if PriorityLow.IsUrgent() {
		t.Error("Low should not be urgent")
	}
	if PriorityMedium.IsUrgent() {
		t.Error("Medium should not be urgent")
	}
	if !PriorityHigh.IsUrgent() {
		t.Error("High should be urgent")
	}
	if !PriorityCritical.IsUrgent() {
		t.Error("Critical should be urgent")
	}
}

// Intentionally NOT testing: TaskError.Error(), ErrNotFound, ErrDuplicateTitle, ErrValidation
