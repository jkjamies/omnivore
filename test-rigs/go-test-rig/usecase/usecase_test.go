package usecase

import (
	"testing"

	"github.com/jkjamies/omnivore/go-test-rig/model"
	"github.com/jkjamies/omnivore/go-test-rig/repository"
)

func TestAddTask(t *testing.T) {
	repo := repository.NewInMemoryTaskRepository()
	task, err := AddTask(repo, 1, "New task", model.PriorityMedium, "", nil)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if task.Title != "New task" {
		t.Errorf("expected 'New task', got '%s'", task.Title)
	}
	if repo.Count() != 1 {
		t.Errorf("expected count 1, got %d", repo.Count())
	}
}

func TestAddTaskWithDescriptionAndTags(t *testing.T) {
	repo := repository.NewInMemoryTaskRepository()
	task, err := AddTask(repo, 1, "Tagged task", model.PriorityHigh, "A  detailed   description", []string{"backend", "urgent"})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if task.Description != "A detailed description" {
		t.Errorf("expected sanitized description, got '%s'", task.Description)
	}
	if len(task.Tags) != 2 {
		t.Errorf("expected 2 tags, got %d", len(task.Tags))
	}
}

func TestAddTaskInvalidTitle(t *testing.T) {
	repo := repository.NewInMemoryTaskRepository()
	_, err := AddTask(repo, 1, "", model.PriorityLow, "", nil)
	if err == nil {
		t.Error("expected error for empty title")
	}
}

func TestAddTaskInvalidTag(t *testing.T) {
	repo := repository.NewInMemoryTaskRepository()
	_, err := AddTask(repo, 1, "Task", model.PriorityLow, "", []string{"bad tag"})
	if err == nil {
		t.Error("expected error for invalid tag")
	}
}

func TestGetTasks(t *testing.T) {
	repo := repository.NewInMemoryTaskRepository()
	_, _ = AddTask(repo, 1, "Fix login", model.PriorityHigh, "", nil)
	_, _ = AddTask(repo, 2, "Add signup", model.PriorityMedium, "", nil)

	all := GetTasks(repo, "")
	if len(all) != 2 {
		t.Errorf("expected 2 tasks, got %d", len(all))
	}

	filtered := GetTasks(repo, "login")
	if len(filtered) != 1 {
		t.Errorf("expected 1 filtered task, got %d", len(filtered))
	}
}

func TestGetStatsEmpty(t *testing.T) {
	repo := repository.NewInMemoryTaskRepository()
	stats := GetStats(repo)
	if stats.Total != 0 {
		t.Errorf("expected 0 total, got %d", stats.Total)
	}
	if stats.CompletionRate != 0.0 {
		t.Errorf("expected 0.0 rate, got %f", stats.CompletionRate)
	}
}

func TestGetStatsMixed(t *testing.T) {
	repo := repository.NewInMemoryTaskRepository()
	_, _ = AddTask(repo, 1, "Done", model.PriorityLow, "", nil)
	_, _ = ToggleTask(repo, 1)
	_, _ = AddTask(repo, 2, "Pending", model.PriorityHigh, "", nil)

	stats := GetStats(repo)
	if stats.Total != 2 {
		t.Errorf("expected 2 total, got %d", stats.Total)
	}
	if stats.Completed != 1 {
		t.Errorf("expected 1 completed, got %d", stats.Completed)
	}
	if stats.Actionable != 1 {
		t.Errorf("expected 1 actionable, got %d", stats.Actionable)
	}
}

// Intentionally NOT testing: ToggleTask, RemoveTask, GetTasksByPriority
