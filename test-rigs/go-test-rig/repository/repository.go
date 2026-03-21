package repository

import (
	"sort"
	"strings"
	"sync"

	"github.com/jkjamies/omnivore/go-test-rig/model"
)

// TaskRepository defines the interface for task persistence.
type TaskRepository interface {
	GetAll() []*model.Task
	GetByID(id uint64) (*model.Task, error)
	Add(task *model.Task) error
	Update(task *model.Task) error
	Remove(id uint64) (*model.Task, error)
	FindByTag(tag string) []*model.Task
	Count() int
}

// InMemoryTaskRepository is an in-memory implementation for testing.
type InMemoryTaskRepository struct {
	mu     sync.RWMutex
	tasks  map[uint64]*model.Task
	nextID uint64
}

// NewInMemoryTaskRepository creates a new in-memory repository.
func NewInMemoryTaskRepository() *InMemoryTaskRepository {
	return &InMemoryTaskRepository{
		tasks:  make(map[uint64]*model.Task),
		nextID: 1,
	}
}

// NextID returns the next available ID.
func (r *InMemoryTaskRepository) NextID() uint64 {
	r.mu.Lock()
	defer r.mu.Unlock()
	id := r.nextID
	r.nextID++
	return id
}

// GetAll returns all tasks sorted by ID.
func (r *InMemoryTaskRepository) GetAll() []*model.Task {
	r.mu.RLock()
	defer r.mu.RUnlock()
	tasks := make([]*model.Task, 0, len(r.tasks))
	for _, t := range r.tasks {
		clone := *t
		tasks = append(tasks, &clone)
	}
	sort.Slice(tasks, func(i, j int) bool {
		return tasks[i].ID < tasks[j].ID
	})
	return tasks
}

// GetByID returns a task by its ID.
func (r *InMemoryTaskRepository) GetByID(id uint64) (*model.Task, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()
	task, ok := r.tasks[id]
	if !ok {
		return nil, model.ErrNotFound(id)
	}
	clone := *task
	return &clone, nil
}

// Add stores a new task, rejecting duplicate titles.
func (r *InMemoryTaskRepository) Add(task *model.Task) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	for _, existing := range r.tasks {
		if strings.EqualFold(existing.Title, task.Title) {
			return model.ErrDuplicateTitle(task.Title)
		}
	}
	clone := *task
	r.tasks[task.ID] = &clone
	return nil
}

// Update replaces an existing task.
func (r *InMemoryTaskRepository) Update(task *model.Task) error {
	r.mu.Lock()
	defer r.mu.Unlock()
	if _, ok := r.tasks[task.ID]; !ok {
		return model.ErrNotFound(task.ID)
	}
	clone := *task
	r.tasks[task.ID] = &clone
	return nil
}

// Remove deletes a task and returns it.
func (r *InMemoryTaskRepository) Remove(id uint64) (*model.Task, error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	task, ok := r.tasks[id]
	if !ok {
		return nil, model.ErrNotFound(id)
	}
	delete(r.tasks, id)
	return task, nil
}

// FindByTag returns all tasks that have the given tag (case-insensitive).
func (r *InMemoryTaskRepository) FindByTag(tag string) []*model.Task {
	r.mu.RLock()
	defer r.mu.RUnlock()
	tagLower := strings.ToLower(tag)
	var result []*model.Task
	for _, t := range r.tasks {
		for _, tt := range t.Tags {
			if strings.ToLower(tt) == tagLower {
				clone := *t
				result = append(result, &clone)
				break
			}
		}
	}
	return result
}

// Count returns the number of tasks.
func (r *InMemoryTaskRepository) Count() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return len(r.tasks)
}
