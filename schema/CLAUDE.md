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
