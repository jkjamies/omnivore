# Future: Multi-Platform Dependency Graphs

The current dependency graph feature works for Gradle/JVM projects via the Omnivore Gradle plugin. Extending this to other ecosystems would require platform-specific tooling to extract dependency information.

## Current State (Gradle/JVM)

The Gradle plugin uses `project.configurations` to walk resolved dependencies and produces a `DependencyGraph` (modules + edges) embedded in `omnivore-report.json`. The dashboard stores, visualizes (D3.js force-directed graph), and serves it via API.

## Platform-Specific Approaches

### Rust (Cargo)

**Data source:** `cargo metadata --format-version=1` outputs a complete dependency graph as JSON.

**Approach:** A CLI tool or build script that:
1. Runs `cargo metadata`
2. Parses the `resolve.nodes` array (each node has `id`, `deps`, `features`)
3. Converts to Omnivore `DependencyGraph` format
4. Embeds in the coverage report (or uploads separately)

**Complexity:** Low — `cargo metadata` is stable and well-documented. No plugin needed, just a CLI wrapper.

### Go

**Data source:** `go mod graph` outputs `module@version module@version` pairs (one edge per line).

**Approach:** A CLI tool that:
1. Runs `go mod graph`
2. Parses the space-separated pairs
3. Converts to Omnivore format

**Complexity:** Low — simple text format. Could be a shell script or small Go tool.

### Swift/iOS (Swift Package Manager)

**Data source:** `swift package show-dependencies --format=json` outputs dependency tree.

**Approach:** CLI parser, similar to Cargo approach.

**Complexity:** Low for SPM. For CocoaPods/Carthage, would need separate parsers.

### JavaScript/TypeScript (npm/yarn/pnpm)

**Data source:** `npm ls --json --all` or `yarn info --json`.

**Approach:** CLI parser.

**Complexity:** Medium — the JS ecosystem has massive dependency trees. Would need to cap depth or filter.

### Python (pip/poetry)

**Data source:** `pip show` + `pipdeptree --json`, or `poetry show --tree`.

**Approach:** CLI parser.

**Complexity:** Low.

## Architecture Options

### Option A: Platform-Specific CLI Tools
Each platform gets a small CLI tool (or script) that extracts dependencies and outputs Omnivore's `DependencyGraph` JSON. The coverage upload step merges this with the coverage report.

**Pros:** Simple, each tool is standalone, easy to maintain.
**Cons:** Users have to run two tools (coverage + deps), or wire them together in CI.

### Option B: Dashboard-Side Parsers
The dashboard accepts raw dependency formats (cargo metadata JSON, go mod graph text, etc.) via a separate endpoint and normalizes them.

**Pros:** Single upload step, format detection like we do for coverage.
**Cons:** Dashboard needs to understand every format, coupling.

### Option C: Universal CLI (Recommended)
A single `omnivore` CLI binary that detects the project type and runs the appropriate extraction. Could be a Rust binary distributed alongside the dashboard.

```sh
# In CI
omnivore deps --format=cargo > deps.json
omnivore upload --report=coverage.json --deps=deps.json
```

**Pros:** Single tool, consistent UX, can be distributed as a single binary.
**Cons:** More upfront work to build, needs to support multiple ecosystems.

## Recommendation

Start with **Option A** for the two most likely use cases (Rust via `cargo metadata` and Go via `go mod graph`) since those are the platforms using lcov/llvm-cov ingestion already. These can be simple shell scripts or thin CLI wrappers.

Later, if adoption grows, consolidate into a universal `omnivore` CLI (Option C).

## Data Model

The current `DependencyGraph` model is already platform-agnostic:

```json
{
  "modules": [
    { "id": ":app", "name": "app", "type": "INTERNAL" },
    { "id": "serde:1.0", "name": "serde", "type": "EXTERNAL", "group": "serde", "version": "1.0" }
  ],
  "edges": [
    { "from": ":app", "to": "serde:1.0", "configuration": "implementation" }
  ]
}
```

No changes needed to the model — just different producers per platform.

## Cycle Detection

A useful future feature: detect circular dependencies in the graph (especially for internal modules in monorepos). The D3.js visualization already makes cycles visually apparent, but an explicit API check + badge would be valuable.
