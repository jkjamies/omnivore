-- Omnivore dashboard schema.
--
-- This file is the single source of truth for table creation. It is used twice:
--
--   1. At runtime, by `Database::run_migrations`, which executes it via
--      `include_str!` on every startup (every statement is idempotent).
--   2. At build time, to create the throwaway database that sqlx's
--      compile-time query macros verify against. `cargo build` cannot check a
--      query against a table that does not exist yet, and the tables used to
--      exist only after the server had been run once — a circular dependency
--      that made `docker build` fail outright on a clean tree.
--
-- Statements are separated by a semicolon at end of line. Column additions to
-- an existing deployment live in `run_migrations` as guarded ALTER TABLEs;
-- new columns must be added in BOTH places.

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    github_repo TEXT,
    source_root TEXT,
    tags TEXT,
    line_threshold REAL,
    branch_threshold REAL,
    line_warn_threshold REAL,
    branch_warn_threshold REAL,
    ratchet_enabled INTEGER NOT NULL DEFAULT 0,
    ratchet_line_floor REAL,
    ratchet_branch_floor REAL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS coverage_snapshots (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id),
    commit_sha TEXT,
    branch TEXT,
    target TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'omnivore-agent',
    line_rate REAL NOT NULL,
    branch_rate REAL NOT NULL,
    lines_covered INTEGER NOT NULL,
    lines_total INTEGER NOT NULL,
    branches_covered INTEGER NOT NULL,
    branches_total INTEGER NOT NULL,
    file_count INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    files_json TEXT,
    dependencies_json TEXT
);

CREATE INDEX IF NOT EXISTS idx_snapshots_project
    ON coverage_snapshots(project_id, created_at DESC);

-- Retention, trends and the project detail page all slice by
-- (project, target, source), so index that tuple rather than relying on the
-- project-only index and a filter.
CREATE INDEX IF NOT EXISTS idx_snapshots_series
    ON coverage_snapshots(project_id, target, source, created_at DESC);

CREATE TABLE IF NOT EXISTS source_cache (
    repo TEXT NOT NULL,
    path TEXT NOT NULL,
    commit_ref TEXT NOT NULL DEFAULT '',
    content TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    PRIMARY KEY (repo, path, commit_ref)
);

CREATE TABLE IF NOT EXISTS settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    default_line_threshold REAL NOT NULL DEFAULT 0.8,
    default_branch_threshold REAL NOT NULL DEFAULT 0.8,
    default_line_warn_threshold REAL NOT NULL DEFAULT 0.5,
    default_branch_warn_threshold REAL NOT NULL DEFAULT 0.5,
    retention_full INTEGER NOT NULL DEFAULT 30,
    retention_summary INTEGER NOT NULL DEFAULT 60
);

INSERT OR IGNORE INTO settings (id) VALUES (1);

CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    key_prefix TEXT NOT NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL,
    last_used_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    github_username TEXT NOT NULL,
    github_token TEXT NOT NULL,
    avatar_url TEXT,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

-- Session lookup happens on every authenticated request.
CREATE INDEX IF NOT EXISTS idx_sessions_expiry ON sessions(expires_at);

CREATE TABLE IF NOT EXISTS permission_cache (
    user_id TEXT NOT NULL,
    repo TEXT NOT NULL,
    permission TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    PRIMARY KEY (user_id, repo)
);
