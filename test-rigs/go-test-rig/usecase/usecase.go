package usecase

import (
	"github.com/jkjamies/omnivore/go-test-rig/model"
	"github.com/jkjamies/omnivore/go-test-rig/repository"
	"github.com/jkjamies/omnivore/go-test-rig/validation"
)

// AddTask creates and stores a new task with validation.
func AddTask(repo repository.TaskRepository, id uint64, title string, priority model.Priority, description string, tags []string) (*model.Task, error) {
	if err := validation.ValidateTitle(title); err != nil {
		return nil, err
	}
	for _, tag := range tags {
		if err := validation.ValidateTag(tag); err != nil {
			return nil, err
		}
	}

	task := model.NewTask(id, title, priority)
	task.Description = validation.SanitizeDescription(description)
	task.Tags = tags

	if err := repo.Add(task); err != nil {
		return nil, err
	}
	return task, nil
}

// ToggleTask flips a task's completion status.
func ToggleTask(repo repository.TaskRepository, id uint64) (*model.Task, error) {
	task, err := repo.GetByID(id)
	if err != nil {
		return nil, err
	}
	task.ToggleCompleted()
	if err := repo.Update(task); err != nil {
		return nil, err
	}
	return task, nil
}

// RemoveTask deletes a task by ID.
func RemoveTask(repo repository.TaskRepository, id uint64) (*model.Task, error) {
	return repo.Remove(id)
}

// GetTasks returns all tasks, optionally filtered by search query.
func GetTasks(repo repository.TaskRepository, query string) []*model.Task {
	all := repo.GetAll()
	if query == "" {
		return all
	}
	var result []*model.Task
	for _, t := range all {
		if t.MatchesSearch(query) {
			result = append(result, t)
		}
	}
	return result
}

// GetTasksByPriority groups tasks by priority (Critical first, Low last).
func GetTasksByPriority(repo repository.TaskRepository) []PriorityGroup {
	all := repo.GetAll()
	priorities := []model.Priority{
		model.PriorityCritical,
		model.PriorityHigh,
		model.PriorityMedium,
		model.PriorityLow,
	}
	var groups []PriorityGroup
	for _, p := range priorities {
		var tasks []*model.Task
		for _, t := range all {
			if t.Priority == p {
				tasks = append(tasks, t)
			}
		}
		if len(tasks) > 0 {
			groups = append(groups, PriorityGroup{Priority: p, Tasks: tasks})
		}
	}
	return groups
}

// PriorityGroup holds tasks grouped by priority.
type PriorityGroup struct {
	Priority model.Priority
	Tasks    []*model.Task
}

// TaskStats holds summary statistics.
type TaskStats struct {
	Total          int
	Completed      int
	Pending        int
	Actionable     int
	CompletionRate float64
}

// GetStats computes summary statistics.
func GetStats(repo repository.TaskRepository) TaskStats {
	all := repo.GetAll()
	total := len(all)
	completed := 0
	actionable := 0
	for _, t := range all {
		if t.Completed {
			completed++
		}
		if t.IsActionable() {
			actionable++
		}
	}
	rate := 0.0
	if total > 0 {
		rate = float64(completed) / float64(total)
	}
	return TaskStats{
		Total:          total,
		Completed:      completed,
		Pending:        total - completed,
		Actionable:     actionable,
		CompletionRate: rate,
	}
}
