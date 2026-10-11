# schema

Shared data format definitions for the Omnivore platform.

## Status

Currently empty. Will contain:
- JSON Schema for `OmnivoreReport` format (the contract between coverage-plugin and dashboard)
- Protocol definitions for multi-platform ingestion (lcov, llvm-cov, Xcode result bundles)

## Current Format Reference

The `OmnivoreReport` JSON format is currently defined implicitly by:
- **Kotlin side:** `coverage-plugin/omnivore-agent/src/main/kotlin/com/jkjamies/omnivore/agent/model/CoverageData.kt` (kotlinx-serialization)
- **Rust side:** `dashboard/crates/omnivore-core/src/model/coverage.rs` (serde)

These must stay in sync. A formal JSON Schema here will serve as the single source of truth.

## Fields that carry non-obvious meaning

Two are easy to get wrong when adding a producer or consumer, so they are
spelled out here until a real schema exists.

### `files[].branchesCovered` / `files[].branchesTotal`

Branch **edges** covered and total, per file. Both default to `0` so reports
from older producers still deserialize.

These exist because branch coverage cannot be aggregated from `branchRate`
alone. Averaging per-file rates gives an unweighted mean, in which a 3-branch
file counts as much as a 300-branch one. Any consumer computing a directory,
project, or composite branch rate must sum these and divide.

A file with genuinely no branches reports `branchesTotal: 0` and
`branchRate: 0.0` — **not** `1.0`. "100% of no branches" reads as a perfect
score and inflates every aggregate it is rolled into. Distinguish "no branches"
from "poor coverage" by checking `branchesTotal`, never by the rate.

### `files[].lines[].hitCount`

`0` means uncovered, `≥1` means covered. It is **not** reliably an execution
count: the Omnivore agent uses boolean probes, so it only ever emits `0` or `1`.
Formats that do track real counts (JaCoCo XML's `ci`) pass them through.

Consumers must treat this as a boolean unless the value exceeds 1, and must not
present it as an execution count when it is exactly 1.

### `project.source`

Which *tool* produced the report (`omnivore-agent`, `kover`, `jacoco`,
`llvm-cov`, `lcov`, `go`, `python-coverage`), as distinct from `project.target`,
which is *where the code ran*. Optional on the wire; defaults to the Omnivore
agent.

The dashboard keys trends and retention on the `(target, source)` pair, so two
tools measuring the same target stay independent series rather than
interleaving into one incoherent trend line.

## When changing the format

1. Update both `CoverageData.kt` and `coverage.rs`.
2. Give new fields a default on the Rust side (`#[serde(default)]`) and a
   default value on the Kotlin side, so a newer dashboard still accepts older
   reports — plugin and dashboard are versioned and deployed separately, and in
   practice are rarely upgraded together.
3. Update this file, since it is the closest thing to a spec.

Note that the binary `.omnivore` / `.probes` formats are a *separate* contract,
internal to the plugin, and are versioned independently — see
`coverage-plugin/CLAUDE.md`.
