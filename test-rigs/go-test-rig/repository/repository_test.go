package repository

import (
	"testing"

	"github.com/jkjamies/omnivore/go-test-rig/model"
)

func TestAddAndGet(t *testing.T) {
	repo := NewInMemoryTaskRepository()
	task := model.NewTask(1, "Test task", model.PriorityMedium)
	if err := repo.Add(task); err != nil {
		t.Fatalf("add failed: %v", err)
	}
	got, err := repo.GetByID(1)
	if err != nil {
		t.Fatalf("get failed: %v", err)
	}
	if got.Title != "Test task" {
		t.Errorf("expected 'Test task', got '%s'", got.Title)
	}
}

func TestGetNonexistent(t *testing.T) {
	repo := NewInMemoryTaskRepository()
	_, err := repo.GetByID(999)
	if err == nil {
		t.Error("expected error for nonexistent task")
	}
}

func TestDuplicateTitle(t *testing.T) {
	repo := NewInMemoryTaskRepository()
	_ = repo.Add(model.NewTask(1, "Title", model.PriorityLow))
	err := repo.Add(model.NewTask(2, "title", model.PriorityHigh))
	if err == nil {
		t.Error("expected error for duplicate title")
	}
}

func TestGetAllSorted(t *testing.T) {
	repo := NewInMemoryTaskRepository()
	_ = repo.Add(model.NewTask(3, "Third", model.PriorityLow))
	_ = repo.Add(model.NewTask(1, "First", model.PriorityLow))
	_ = repo.Add(model.NewTask(2, "Second", model.PriorityLow))
	all := repo.GetAll()
	if len(all) != 3 {
		t.Fatalf("expected 3 tasks, got %d", len(all))
	}
	if all[0].ID != 1 || all[2].ID != 3 {
		t.Error("tasks not sorted by ID")
	}
}

func TestCount(t *testing.T) {
	repo := NewInMemoryTaskRepository()
	if repo.Count() != 0 {
		t.Error("expected empty repo")
	}
	_ = repo.Add(model.NewTask(1, "A", model.PriorityLow))
	if repo.Count() != 1 {
		t.Error("expected count 1")
	}
}

// Intentionally NOT testing: Update, Remove, FindByTag, NextID
