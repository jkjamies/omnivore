# kmp-test-rig

Multi-module KMP project for testing the Omnivore coverage plugin. Uses Clean Architecture + MVI. Contains intentionally partial test coverage to exercise reporting.

## How to Run

```sh
# Requires coverage-plugin to be built first
./gradlew test omnivoreReport

# Upload report to dashboard (requires running dashboard)
./gradlew omnivoreUpload -Pomnivore.dashboard.url=http://localhost:3000
```

Reports generated in `build/reports/omnivore/`:
- `omnivore-report.json` — machine-readable coverage data
- `index.html` — visual HTML report
- `coverage.md` — Markdown summary

## Plugin Configuration

Uses composite build (`includeBuild("../../coverage-plugin")`) to resolve the plugin from local source. Compose filter is disabled. JSON, HTML, and Markdown reports are enabled.

Dependency graph collection is enabled (internal modules only by default).

## Architecture (Clean Architecture + MVI)

```
core/ (domain layer)
  model/         User, OpResult sealed class
  repository/    UserRepository interface
  usecase/       AddUser, GetUsers, DeactivateUser, RemoveUser use cases
  Validation.kt  Shared validation utilities

app/ (data + presentation + utilities)
  data/repository/   InMemoryUserRepository implementation
  presentation/      MVI: UserListContract (State/Intent/Effect) + UserListStore
  util/              Calculator, StringUtils (coverage targets with intentional gaps)
```

## Intentional Coverage Gaps

| Layer | File | Untested |
|---|---|---|
| Domain | `OpResult.kt` | `map()` |
| Domain | `Validation.kt` | `isValidName()` edge cases, email dot-after-@ |
| Domain | `GetUsersUseCase.kt` | `INACTIVE` filter |
| Domain | `DeactivateUserUseCase.kt` | Fully untested (only exercised via store) |
| Domain | `RemoveUserUseCase.kt` | Fully untested (only exercised via store) |
| Data | `InMemoryUserRepository.kt` | `update()`, `findByEmail()`, `getAll()` |
| Presentation | `UserListStore.kt` | Deactivate, remove, setFilter, error effects |
| Presentation | `UserListContract.kt` | `activeCount`, `inactiveCount` |
| Util | `Calculator.kt` | `multiply()`, medium/large branches |
| Util | `StringUtils.kt` | `toCamelCase()`, `countWords()`, truncate edge cases |
