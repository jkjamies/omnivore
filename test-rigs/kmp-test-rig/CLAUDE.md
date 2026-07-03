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

### Kover (opt-in second coverage source)

This rig also has the [Kover](https://github.com/Kotlin/kotlinx-kover) Gradle
plugin wired up to produce a JaCoCo-compatible XML report — a convenient way to
exercise the dashboard's JaCoCo/Kover ingestion path. It is **opt-in** via the
`omnivore.kover` property, kept off by default because applying Kover instruments
the same `test` tasks that the Omnivore agent already instruments during
`omnivoreReport`; running only one at a time keeps them from interfering.

```sh
# Generate a JaCoCo-compatible XML report at build/reports/kover/report.xml
./gradlew koverXmlReport -Pomnivore.kover

# Ingest it — recorded as target=JVM_UNIT, source=kover (a separate series from
# the omnivore-agent JVM_UNIT report, so both trend independently on the dashboard)
curl -X POST "http://localhost:3000/api/v1/ingest/coverage?format=kover&project_id=kmp-test-rig&project_name=KMP+Test+Rig" \
  --data-binary @build/reports/kover/report.xml
```

Without `-Pomnivore.kover` the Kover plugin is not applied, so the standard
`./gradlew test omnivoreReport` flow (and CI) is completely unaffected.

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
