package com.example.android.testrig.presentation

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.example.android.testrig.domain.model.Task
import com.example.android.testrig.domain.usecase.GetTasksUseCase

/**
 * Pure Compose UI screen for the task list.
 *
 * This file contains ONLY @Composable functions, so Omnivore's auto-exclusion
 * will remove it from JVM unit test coverage (Compose UI can't be unit tested
 * on the JVM — it needs instrumented tests or screenshot tests).
 */
@Composable
fun TaskListScreen(viewModel: TaskListViewModel) {
    val state by viewModel.state.collectAsState()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(16.dp),
    ) {
        Text(
            text = "Tasks",
            style = MaterialTheme.typography.headlineMedium,
        )

        Spacer(modifier = Modifier.height(8.dp))

        SearchBar(
            query = state.searchQuery,
            onQueryChange = { viewModel.processIntent(TaskListContract.Intent.Search(it)) },
        )

        Spacer(modifier = Modifier.height(8.dp))

        FilterRow(
            currentFilter = state.filter,
            activeCount = state.activeCount,
            completedCount = state.completedCount,
            onFilterSelected = { viewModel.processIntent(TaskListContract.Intent.SetFilter(it)) },
        )

        Spacer(modifier = Modifier.height(16.dp))

        if (state.isLoading) {
            CircularProgressIndicator(modifier = Modifier.align(Alignment.CenterHorizontally))
        }

        state.error?.let { error ->
            Text(
                text = error,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
            )
        }

        LazyColumn(
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            items(state.filteredTasks, key = { it.id }) { task ->
                TaskCard(
                    task = task,
                    onToggle = { viewModel.processIntent(TaskListContract.Intent.ToggleTask(task.id)) },
                    onDelete = { viewModel.processIntent(TaskListContract.Intent.DeleteTask(task.id)) },
                )
            }
        }
    }
}

@Composable
fun SearchBar(query: String, onQueryChange: (String) -> Unit) {
    TextField(
        value = query,
        onValueChange = onQueryChange,
        modifier = Modifier.fillMaxWidth(),
        placeholder = { Text("Search tasks...") },
        singleLine = true,
    )
}

@Composable
fun FilterRow(
    currentFilter: GetTasksUseCase.Filter,
    activeCount: Int,
    completedCount: Int,
    onFilterSelected: (GetTasksUseCase.Filter) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        FilterChip(
            selected = currentFilter == GetTasksUseCase.Filter.ALL,
            onClick = { onFilterSelected(GetTasksUseCase.Filter.ALL) },
            label = { Text("All") },
        )
        FilterChip(
            selected = currentFilter == GetTasksUseCase.Filter.ACTIVE,
            onClick = { onFilterSelected(GetTasksUseCase.Filter.ACTIVE) },
            label = { Text("Active ($activeCount)") },
        )
        FilterChip(
            selected = currentFilter == GetTasksUseCase.Filter.COMPLETED,
            onClick = { onFilterSelected(GetTasksUseCase.Filter.COMPLETED) },
            label = { Text("Done ($completedCount)") },
        )
    }
}

@Composable
fun TaskCard(task: Task, onToggle: () -> Unit, onDelete: () -> Unit) {
    Card(
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Checkbox(
                checked = task.isCompleted,
                onCheckedChange = { onToggle() },
            )

            Spacer(modifier = Modifier.width(8.dp))

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = task.title,
                    style = MaterialTheme.typography.bodyLarge,
                    textDecoration = if (task.isCompleted) TextDecoration.LineThrough else null,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                if (task.description.isNotBlank()) {
                    Text(
                        text = task.description,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                PriorityBadge(priority = task.priority)
            }

            IconButton(onClick = onDelete) {
                Text("X", color = MaterialTheme.colorScheme.error)
            }
        }
    }
}

@Composable
fun PriorityBadge(priority: Task.Priority) {
    val (text, color) = when (priority) {
        Task.Priority.HIGH -> "HIGH" to MaterialTheme.colorScheme.error
        Task.Priority.MEDIUM -> "MED" to MaterialTheme.colorScheme.tertiary
        Task.Priority.LOW -> "LOW" to MaterialTheme.colorScheme.outline
    }
    Text(
        text = text,
        style = MaterialTheme.typography.labelSmall,
        color = color,
    )
}
