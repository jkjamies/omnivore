use crate::model::api_key::{ApiKey, ApiKeyCreated};
use crate::model::coverage::CoverageSnapshot;
use crate::model::project::{CreateProject, Project};
use crate::model::session::Session;
use crate::model::settings::GlobalSettings;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use std::str::FromStr;
use std::time::Duration;

/// Result of the ratchet check performed during ingest.
#[derive(Debug, Default)]
pub struct RatchetResult {
    pub line_floor_violated: bool,
    pub branch_floor_violated: bool,
    pub line_floor: Option<f64>,
    pub branch_floor: Option<f64>,
}

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
    /// Resolved once at construction rather than read from the environment on
    /// every ingest.
    ///
    /// Reading it per call made the setting process-global *state*, which meant
    /// a test could not exercise the closed mode without changing behaviour for
    /// every other test running at the same moment — and that is exactly what
    /// happened: `project_autocreate_can_be_disabled` set the variable while
    /// ~40 concurrent `#[tokio::test]`s were ingesting, so an unrelated test
    /// occasionally got a refusal it never asked for. A suite that fails
    /// roughly one run in ten teaches people to press re-run, which is worse
    /// than no suite.
    allow_project_autocreate: bool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        // Connection-level PRAGMAs must be set per connection, not once against
        // a pooled connection — `PRAGMA foreign_keys = ON` executed as a plain
        // query only applies to whichever of the pool's connections ran it, so
        // ON DELETE CASCADE silently did nothing on the other four.
        //
        // WAL + a busy timeout matter for the self-hosted case: with the
        // default rollback journal, a page render reading while an ingest
        // writes fails outright with SQLITE_BUSY instead of waiting.
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;

        let db = Self {
            pool,
            allow_project_autocreate: auto_create_projects(),
        };
        db.run_migrations().await?;
        Ok(db)
    }

    /// Override whether ingest may create unseen projects.
    ///
    /// The environment decides this for a real server; this exists so a test
    /// can pin the setting on one `Database` without mutating process state
    /// that every other concurrently-running test also reads.
    pub fn with_project_autocreate(mut self, allowed: bool) -> Self {
        self.allow_project_autocreate = allowed;
        self
    }

    /// Table creation DDL, shared with the build-time sqlx database.
    ///
    /// See `crates/omnivore-core/schema.sql` for why this lives in a file
    /// rather than inline here.
    pub const SCHEMA_SQL: &'static str = include_str!("../../schema.sql");

    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        // Order matters, in three phases.
        //
        // On an *existing* database `CREATE TABLE IF NOT EXISTS` is a no-op, so
        // a table keeps whatever columns it already had. Any index over a
        // column that only the ALTER phase adds must therefore be created
        // *after* that phase — otherwise upgrading an older deployment fails
        // with "no such column" and the server refuses to start. Indexes are
        // split out by inspection rather than by convention so this cannot
        // regress when a new index is added to schema.sql.
        let (indexes, tables): (Vec<&str>, Vec<&str>) = Self::schema_statements()
            .partition(|s| s.to_ascii_uppercase().starts_with("CREATE INDEX"));

        // Phase 1: tables and seed rows. Creates everything on a fresh install;
        // no-ops on an existing one.
        for statement in tables {
            sqlx::query(statement).execute(&self.pool).await?;
        }

        // Phase 2: the guarded ALTER TABLEs below upgrade databases created by
        // earlier versions, which predate columns the schema file now declares.
        // They are no-ops on a fresh database.

        // Migration: add dependencies_json column if it doesn't exist
        // SQLite doesn't support IF NOT EXISTS for ALTER TABLE, so we check the schema.
        let has_deps_col: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('coverage_snapshots') WHERE name = 'dependencies_json'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_deps_col {
            sqlx::query("ALTER TABLE coverage_snapshots ADD COLUMN dependencies_json TEXT")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add source (provenance) column to coverage_snapshots if missing.
        // Existing rows predate multi-source ingestion and came from the native
        // agent, so they backfill to 'omnivore-agent' via the column default.
        let has_source_col: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('coverage_snapshots') WHERE name = 'source'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_source_col {
            sqlx::query(
                "ALTER TABLE coverage_snapshots ADD COLUMN source TEXT NOT NULL DEFAULT 'omnivore-agent'"
            )
            .execute(&self.pool)
            .await?;
        }

        // Migration: add github_repo column to projects if missing
        let has_repo_col: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'github_repo'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_repo_col {
            sqlx::query("ALTER TABLE projects ADD COLUMN github_repo TEXT")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add source_root column to projects if missing
        let has_source_root_col: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'source_root'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_source_root_col {
            sqlx::query("ALTER TABLE projects ADD COLUMN source_root TEXT")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add threshold columns to projects if missing
        let has_line_threshold: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'line_threshold'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_line_threshold {
            sqlx::query("ALTER TABLE projects ADD COLUMN line_threshold REAL")
                .execute(&self.pool)
                .await?;
            sqlx::query("ALTER TABLE projects ADD COLUMN branch_threshold REAL")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add warning threshold columns to projects if missing
        let has_line_warn_threshold: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'line_warn_threshold'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_line_warn_threshold {
            sqlx::query("ALTER TABLE projects ADD COLUMN line_warn_threshold REAL")
                .execute(&self.pool)
                .await?;
            sqlx::query("ALTER TABLE projects ADD COLUMN branch_warn_threshold REAL")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add warning threshold columns to settings if missing
        let has_settings_warn: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('settings') WHERE name = 'default_line_warn_threshold'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_settings_warn {
            sqlx::query("ALTER TABLE settings ADD COLUMN default_line_warn_threshold REAL NOT NULL DEFAULT 0.5")
                .execute(&self.pool)
                .await?;
            sqlx::query("ALTER TABLE settings ADD COLUMN default_branch_warn_threshold REAL NOT NULL DEFAULT 0.5")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add retention columns to settings if missing
        let has_retention: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('settings') WHERE name = 'retention_full'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_retention {
            sqlx::query("ALTER TABLE settings ADD COLUMN retention_full INTEGER NOT NULL DEFAULT 30")
                .execute(&self.pool)
                .await?;
            sqlx::query("ALTER TABLE settings ADD COLUMN retention_summary INTEGER NOT NULL DEFAULT 60")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add tags column to projects if missing
        let has_tags: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'tags'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_tags {
            sqlx::query("ALTER TABLE projects ADD COLUMN tags TEXT")
                .execute(&self.pool)
                .await?;
        }

        // Migration: add ratchet columns to projects if missing
        let has_ratchet: bool = sqlx::query_scalar::<_, i32>(
            "SELECT COUNT(*) FROM pragma_table_info('projects') WHERE name = 'ratchet_enabled'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(0) > 0;

        if !has_ratchet {
            sqlx::query("ALTER TABLE projects ADD COLUMN ratchet_enabled INTEGER NOT NULL DEFAULT 0")
                .execute(&self.pool)
                .await?;
            sqlx::query("ALTER TABLE projects ADD COLUMN ratchet_line_floor REAL")
                .execute(&self.pool)
                .await?;
            sqlx::query("ALTER TABLE projects ADD COLUMN ratchet_branch_floor REAL")
                .execute(&self.pool)
                .await?;
        }

        // Phase 3: indexes, now that every column they reference exists.
        for statement in indexes {
            sqlx::query(statement).execute(&self.pool).await?;
        }

        Ok(())
    }

    /// Split [`Self::SCHEMA_SQL`] into executable statements.
    ///
    /// Statements are separated by a semicolon at end of line; comment lines are
    /// stripped first, so a statement preceded by a comment block is not
    /// mistaken for a comment and skipped whole.
    fn schema_statements() -> impl Iterator<Item = &'static str> {
        Self::SCHEMA_SQL.split(";\n").filter_map(|chunk| {
            let mut rest = chunk.trim_start();
            while let Some(stripped) = rest.strip_prefix("--") {
                rest = stripped.find('\n').map(|i| &stripped[i + 1..]).unwrap_or("").trim_start();
            }
            let rest = rest.trim();
            if rest.is_empty() { None } else { Some(rest) }
        })
    }

    // -- Projects --

    pub async fn create_project(&self, input: &CreateProject) -> Result<Project, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO projects (id, name, description, github_repo, source_root, line_threshold, branch_threshold, line_warn_threshold, branch_warn_threshold, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.github_repo)
        .bind(&input.source_root)
        .bind(input.line_threshold)
        .bind(input.branch_threshold)
        .bind(input.line_warn_threshold)
        .bind(input.branch_warn_threshold)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        self.get_project(&input.id)
            .await?
            .ok_or(sqlx::Error::RowNotFound)
    }

    pub async fn get_project(&self, id: &str) -> Result<Option<Project>, sqlx::Error> {
        sqlx::query_as!(
            Project,
            r#"SELECT id as "id!: String", name,
                      description as "description: String",
                      github_repo as "github_repo: String",
                      source_root as "source_root: String",
                      line_threshold as "line_threshold: f64",
                      branch_threshold as "branch_threshold: f64",
                      line_warn_threshold as "line_warn_threshold: f64",
                      branch_warn_threshold as "branch_warn_threshold: f64",
                      tags as "tags: String",
                      ratchet_enabled as "ratchet_enabled!: bool",
                      ratchet_line_floor as "ratchet_line_floor: f64",
                      ratchet_branch_floor as "ratchet_branch_floor: f64",
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM projects WHERE id = ?"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>, sqlx::Error> {
        sqlx::query_as!(
            Project,
            r#"SELECT id as "id!: String", name,
                      description as "description: String",
                      github_repo as "github_repo: String",
                      source_root as "source_root: String",
                      line_threshold as "line_threshold: f64",
                      branch_threshold as "branch_threshold: f64",
                      line_warn_threshold as "line_warn_threshold: f64",
                      branch_warn_threshold as "branch_warn_threshold: f64",
                      tags as "tags: String",
                      ratchet_enabled as "ratchet_enabled!: bool",
                      ratchet_line_floor as "ratchet_line_floor: f64",
                      ratchet_branch_floor as "ratchet_branch_floor: f64",
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM projects ORDER BY name"#
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_project_settings(
        &self,
        id: &str,
        github_repo: Option<&str>,
        source_root: Option<&str>,
    ) -> Result<Option<Project>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE projects SET github_repo = COALESCE(?, github_repo),
                                 source_root = COALESCE(?, source_root),
                                 updated_at = ? WHERE id = ?",
        )
        .bind(github_repo)
        .bind(source_root)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_project(id).await
    }

    /// Delete a project and everything that references it.
    ///
    /// Runs in one transaction so a failure part-way through can't leave
    /// orphaned snapshots or, worse, live API keys pointing at a project that
    /// no longer exists.
    pub async fn delete_project(&self, id: &str) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM coverage_snapshots WHERE project_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        // Project-scoped keys authorise uploads to this project; they must not
        // outlive it. The schema declares ON DELETE CASCADE, but deleting
        // explicitly keeps this correct regardless of the foreign_keys pragma.
        sqlx::query("DELETE FROM api_keys WHERE project_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await
    }

    // -- Global settings --

    pub async fn get_global_settings(&self) -> Result<GlobalSettings, sqlx::Error> {
        let row = sqlx::query_as::<_, (f64, f64, f64, f64, i64, i64)>(
            "SELECT default_line_threshold, default_branch_threshold, default_line_warn_threshold, default_branch_warn_threshold, retention_full, retention_summary FROM settings WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .map(|(lt, bt, lwt, bwt, rf, rs)| GlobalSettings {
                default_line_threshold: lt,
                default_branch_threshold: bt,
                default_line_warn_threshold: lwt,
                default_branch_warn_threshold: bwt,
                retention_full: rf,
                retention_summary: rs,
            })
            .unwrap_or_default())
    }

    pub async fn update_global_settings(
        &self,
        settings: &GlobalSettings,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE settings SET default_line_threshold = ?, default_branch_threshold = ?, default_line_warn_threshold = ?, default_branch_warn_threshold = ?, retention_full = ?, retention_summary = ? WHERE id = 1",
        )
        .bind(settings.default_line_threshold)
        .bind(settings.default_branch_threshold)
        .bind(settings.default_line_warn_threshold)
        .bind(settings.default_branch_warn_threshold)
        .bind(settings.retention_full)
        .bind(settings.retention_summary)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // -- Project thresholds --

    pub async fn update_project_thresholds(
        &self,
        id: &str,
        line_threshold: Option<f64>,
        branch_threshold: Option<f64>,
        line_warn_threshold: Option<f64>,
        branch_warn_threshold: Option<f64>,
    ) -> Result<Option<Project>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE projects SET line_threshold = ?, branch_threshold = ?, line_warn_threshold = ?, branch_warn_threshold = ?, updated_at = ? WHERE id = ?",
        )
        .bind(line_threshold)
        .bind(branch_threshold)
        .bind(line_warn_threshold)
        .bind(branch_warn_threshold)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_project(id).await
    }

    pub async fn update_project_tags(
        &self,
        id: &str,
        tags: Option<&str>,
    ) -> Result<Option<Project>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE projects SET tags = ?, updated_at = ? WHERE id = ?")
            .bind(tags)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;

        self.get_project(id).await
    }

    // -- Ratchet --

    pub async fn update_project_ratchet(
        &self,
        id: &str,
        enabled: bool,
        line_floor: Option<f64>,
        branch_floor: Option<f64>,
    ) -> Result<Option<Project>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE projects SET ratchet_enabled = ?, ratchet_line_floor = ?, ratchet_branch_floor = ?, updated_at = ? WHERE id = ?",
        )
        .bind(enabled)
        .bind(line_floor)
        .bind(branch_floor)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get_project(id).await
    }

    /// Advance ratchet floors if new rates are higher. Uses MAX in SQL for atomicity.
    async fn advance_ratchet_floor(
        &self,
        project_id: &str,
        new_line_rate: f64,
        new_branch_rate: f64,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE projects SET
               ratchet_line_floor = MAX(COALESCE(ratchet_line_floor, 0), ?),
               ratchet_branch_floor = MAX(COALESCE(ratchet_branch_floor, 0), ?),
               updated_at = ?
             WHERE id = ? AND ratchet_enabled = 1",
        )
        .bind(new_line_rate)
        .bind(new_branch_rate)
        .bind(&now)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get recent ingest activity across all projects.
    pub async fn get_recent_activity(&self, limit: i64) -> Result<Vec<ActivityEntry>, sqlx::Error> {
        sqlx::query_as!(
            ActivityEntry,
            r#"SELECT s.created_at as "created_at!: DateTime<Utc>",
                      s.project_id as "project_id!: String",
                      p.name as "project_name!: String",
                      s.target as "target!: String",
                      s.commit_sha as "commit_sha: String",
                      s.line_rate as "line_rate!: f64",
                      s.branch_rate as "branch_rate!: f64",
                      s.lines_covered as "lines_covered!: i64",
                      s.lines_total as "lines_total!: i64"
               FROM coverage_snapshots s
               JOIN projects p ON s.project_id = p.id
               ORDER BY s.created_at DESC
               LIMIT ?"#,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get recent ingest activity for a specific project.
    pub async fn get_project_activity(&self, project_id: &str, limit: i64) -> Result<Vec<ActivityEntry>, sqlx::Error> {
        sqlx::query_as!(
            ActivityEntry,
            r#"SELECT s.created_at as "created_at!: DateTime<Utc>",
                      s.project_id as "project_id!: String",
                      p.name as "project_name!: String",
                      s.target as "target!: String",
                      s.commit_sha as "commit_sha: String",
                      s.line_rate as "line_rate!: f64",
                      s.branch_rate as "branch_rate!: f64",
                      s.lines_covered as "lines_covered!: i64",
                      s.lines_total as "lines_total!: i64"
               FROM coverage_snapshots s
               JOIN projects p ON s.project_id = p.id
               WHERE s.project_id = ?
               ORDER BY s.created_at DESC
               LIMIT ?"#,
            project_id,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    // -- API Keys --

    fn hash_key(raw_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(raw_key.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub async fn create_api_key(
        &self,
        name: &str,
        project_id: Option<&str>,
    ) -> Result<ApiKeyCreated, sqlx::Error> {
        let id = uuid::Uuid::new_v4().to_string();
        let raw_key = format!(
            "omni_{}{}",
            uuid::Uuid::new_v4().simple(),
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        let key_prefix = raw_key[..8].to_string();
        let key_hash = Self::hash_key(&raw_key);
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO api_keys (id, name, key_hash, key_prefix, project_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(&key_hash)
        .bind(&key_prefix)
        .bind(project_id)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        Ok(ApiKeyCreated {
            id,
            name: name.to_string(),
            key: raw_key,
            key_prefix,
            project_id: project_id.map(String::from),
        })
    }

    pub async fn list_api_keys(
        &self,
        project_id: Option<&str>,
    ) -> Result<Vec<ApiKey>, sqlx::Error> {
        match project_id {
            None => {
                sqlx::query_as!(
                    ApiKey,
                    r#"SELECT id as "id!: String",
                              name as "name!: String",
                              key_prefix as "key_prefix!: String",
                              key_hash as "key_hash!: String",
                              project_id as "project_id: String",
                              created_at as "created_at!: DateTime<Utc>",
                              last_used_at as "last_used_at: DateTime<Utc>"
                       FROM api_keys
                       WHERE project_id IS NULL
                       ORDER BY created_at DESC"#
                )
                .fetch_all(&self.pool)
                .await
            }
            Some(pid) => {
                sqlx::query_as!(
                    ApiKey,
                    r#"SELECT id as "id!: String",
                              name as "name!: String",
                              key_prefix as "key_prefix!: String",
                              key_hash as "key_hash!: String",
                              project_id as "project_id: String",
                              created_at as "created_at!: DateTime<Utc>",
                              last_used_at as "last_used_at: DateTime<Utc>"
                       FROM api_keys
                       WHERE project_id = ?
                       ORDER BY created_at DESC"#,
                    pid
                )
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    pub async fn delete_api_key(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM api_keys WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn validate_api_key(&self, raw_key: &str) -> Result<Option<ApiKey>, sqlx::Error> {
        let key_hash = Self::hash_key(raw_key);
        let result = sqlx::query_as!(
            ApiKey,
            r#"SELECT id as "id!: String",
                      name as "name!: String",
                      key_prefix as "key_prefix!: String",
                      key_hash as "key_hash!: String",
                      project_id as "project_id: String",
                      created_at as "created_at!: DateTime<Utc>",
                      last_used_at as "last_used_at: DateTime<Utc>"
               FROM api_keys
               WHERE key_hash = ?"#,
            key_hash
        )
        .fetch_optional(&self.pool)
        .await?;

        if let Some(ref key) = result {
            let now = Utc::now().to_rfc3339();
            sqlx::query("UPDATE api_keys SET last_used_at = ? WHERE id = ?")
                .bind(&now)
                .bind(&key.id)
                .execute(&self.pool)
                .await?;
        }

        Ok(result)
    }

    pub async fn any_api_keys_exist(&self) -> Result<bool, sqlx::Error> {
        let count: i32 =
            sqlx::query_scalar("SELECT COUNT(*) FROM api_keys")
                .fetch_one(&self.pool)
                .await?;
        Ok(count > 0)
    }

    // -- Coverage snapshots --

    pub async fn insert_snapshot(
        &self,
        snapshot: &CoverageSnapshot,
    ) -> Result<(), sqlx::Error> {
        let created_at = snapshot.created_at.to_rfc3339();
        sqlx::query(
            "INSERT INTO coverage_snapshots
             (id, project_id, commit_sha, branch, target, source, line_rate, branch_rate,
              lines_covered, lines_total, branches_covered, branches_total,
              file_count, created_at, files_json, dependencies_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&snapshot.id)
        .bind(&snapshot.project_id)
        .bind(&snapshot.commit_sha)
        .bind(&snapshot.branch)
        .bind(&snapshot.target)
        .bind(&snapshot.source)
        .bind(snapshot.line_rate)
        .bind(snapshot.branch_rate)
        .bind(snapshot.lines_covered)
        .bind(snapshot.lines_total)
        .bind(snapshot.branches_covered)
        .bind(snapshot.branches_total)
        .bind(snapshot.file_count)
        .bind(&created_at)
        .bind(&snapshot.files_json)
        .bind(&snapshot.dependencies_json)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_latest_snapshot(
        &self,
        project_id: &str,
    ) -> Result<Option<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ?
               ORDER BY created_at DESC
               LIMIT 1"#,
            project_id
        )
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_snapshots_for_project(
        &self,
        project_id: &str,
        limit: i64,
    ) -> Result<Vec<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ?
               ORDER BY created_at DESC
               LIMIT ?"#,
            project_id,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get the latest snapshot for a project on a specific branch.
    pub async fn get_latest_snapshot_for_branch(
        &self,
        project_id: &str,
        branch: &str,
    ) -> Result<Option<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ? AND branch = ?
               ORDER BY created_at DESC
               LIMIT 1"#,
            project_id,
            branch
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get the latest snapshot for a specific target.
    pub async fn get_latest_snapshot_by_target(
        &self,
        project_id: &str,
        target: &str,
    ) -> Result<Option<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ? AND target = ?
               ORDER BY created_at DESC
               LIMIT 1"#,
            project_id,
            target
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get trend data for a specific target.
    pub async fn get_snapshots_for_project_by_target(
        &self,
        project_id: &str,
        target: &str,
        limit: i64,
    ) -> Result<Vec<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ? AND target = ?
               ORDER BY created_at DESC
               LIMIT ?"#,
            project_id,
            target,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get a single snapshot by its ID.
    pub async fn get_snapshot_by_id(
        &self,
        id: &str,
    ) -> Result<Option<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE id = ?"#,
            id
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get the snapshot for a target closest to a given date.
    pub async fn get_snapshot_closest_to_date(
        &self,
        project_id: &str,
        target: &str,
        date: &str,
    ) -> Result<Option<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ? AND target = ?
               ORDER BY ABS(JULIANDAY(created_at) - JULIANDAY(?))
               LIMIT 1"#,
            project_id,
            target,
            date
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get distinct targets that have snapshots for a project.
    ///
    /// Target-only view (aggregated across sources) — used by the badge, embed,
    /// and export endpoints whose contract is keyed on target alone.
    pub async fn get_targets_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT target FROM coverage_snapshots WHERE project_id = ? ORDER BY target",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get the distinct `(target, source)` series that have snapshots for a
    /// project. A "series" is the unit the dashboard trends and prunes on, so a
    /// project measured by two tools for the same target shows as two series.
    pub async fn get_series_for_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<(String, String)>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT DISTINCT target, source FROM coverage_snapshots
             WHERE project_id = ? ORDER BY target, source",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get the latest snapshot for a specific `(target, source)` series.
    pub async fn get_latest_snapshot_by_series(
        &self,
        project_id: &str,
        target: &str,
        source: &str,
    ) -> Result<Option<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ? AND target = ? AND source = ?
               ORDER BY created_at DESC
               LIMIT 1"#,
            project_id,
            target,
            source
        )
        .fetch_optional(&self.pool)
        .await
    }

    /// Get trend data (most-recent first) for a specific `(target, source)` series.
    pub async fn get_snapshots_for_project_by_series(
        &self,
        project_id: &str,
        target: &str,
        source: &str,
        limit: i64,
    ) -> Result<Vec<CoverageSnapshot>, sqlx::Error> {
        sqlx::query_as!(
            CoverageSnapshot,
            r#"SELECT id as "id!: String", project_id,
                      commit_sha as "commit_sha: String",
                      branch as "branch: String",
                      target, source, line_rate, branch_rate,
                      lines_covered, lines_total,
                      branches_covered, branches_total, file_count,
                      created_at as "created_at!: DateTime<Utc>",
                      files_json as "files_json: String",
                      dependencies_json as "dependencies_json: String"
               FROM coverage_snapshots
               WHERE project_id = ? AND target = ? AND source = ?
               ORDER BY created_at DESC
               LIMIT ?"#,
            project_id,
            target,
            source,
            limit
        )
        .fetch_all(&self.pool)
        .await
    }

    // -- Source cache --

    /// How long cached source stays servable when it was fetched without a
    /// pinned commit.
    ///
    /// An entry keyed by a real commit SHA is immutable and never needs to
    /// expire. An entry keyed by the empty ref was fetched from a moving
    /// branch head, so it goes stale the moment anyone pushes — and, with no
    /// expiry at all, the file view showed whatever was fetched first for the
    /// life of the database.
    const FLOATING_SOURCE_TTL_SECONDS: i64 = 60 * 60;

    /// Look up cached source content.
    ///
    /// Entries fetched against a floating ref are ignored once older than
    /// [`Self::FLOATING_SOURCE_TTL_SECONDS`], so the caller re-fetches instead
    /// of rendering stale source against current coverage line numbers — a
    /// mismatch that shows covered/uncovered marks against the wrong lines.
    pub async fn get_cached_source(
        &self,
        repo: &str,
        path: &str,
        commit_ref: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let ref_key = if commit_ref.is_empty() { "" } else { commit_ref };

        let row = sqlx::query_as::<_, (String, String)>(
            "SELECT content, fetched_at FROM source_cache
             WHERE repo = ? AND path = ? AND commit_ref = ?",
        )
        .bind(repo)
        .bind(path)
        .bind(ref_key)
        .fetch_optional(&self.pool)
        .await?;

        let Some((content, fetched_at)) = row else {
            return Ok(None);
        };

        if !ref_key.is_empty() {
            // Pinned to an immutable commit — always fresh.
            return Ok(Some(content));
        }

        let is_fresh = DateTime::parse_from_rfc3339(&fetched_at)
            .map(|t| {
                (Utc::now() - t.with_timezone(&Utc)).num_seconds()
                    < Self::FLOATING_SOURCE_TTL_SECONDS
            })
            // An unparseable timestamp predates this check; treat it as stale
            // and re-fetch rather than serving something of unknown age.
            .unwrap_or(false);

        Ok(if is_fresh { Some(content) } else { None })
    }

    /// Delete cached source that can no longer be served.
    ///
    /// Source blobs are by far the largest thing in the database, and nothing
    /// used to remove them — a dashboard accumulated every file anyone had ever
    /// viewed, forever.
    pub async fn prune_source_cache(&self) -> Result<u64, sqlx::Error> {
        let cutoff = (Utc::now()
            - chrono::Duration::seconds(Self::FLOATING_SOURCE_TTL_SECONDS))
        .to_rfc3339();

        let result = sqlx::query(
            "DELETE FROM source_cache WHERE commit_ref = '' AND fetched_at < ?",
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Store source content in cache.
    pub async fn cache_source(
        &self,
        repo: &str,
        path: &str,
        commit_ref: &str,
        content: &str,
    ) -> Result<(), sqlx::Error> {
        let ref_key = if commit_ref.is_empty() { "" } else { commit_ref };
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO source_cache (repo, path, commit_ref, content, fetched_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(repo)
        .bind(path)
        .bind(ref_key)
        .bind(content)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Auto-create project if it doesn't exist, then insert the snapshot.
    /// Returns ratchet check results (empty if ratchet not enabled).
    pub async fn ingest_snapshot(
        &self,
        snapshot: &CoverageSnapshot,
        project_name: Option<&str>,
    ) -> Result<RatchetResult, sqlx::Error> {
        // Ensure project exists
        if self.get_project(&snapshot.project_id).await?.is_none() {
            if !self.allow_project_autocreate {
                // Closed mode: a project must be created deliberately before
                // it can receive coverage. Otherwise any caller who reaches
                // ingest can mint rows for arbitrary project IDs — a typo in a
                // CI config silently creates a second project rather than
                // failing, and an open instance accumulates junk indefinitely.
                return Err(sqlx::Error::Protocol(format!(
                    "Project '{}' does not exist and OMNIVORE_ALLOW_PROJECT_AUTOCREATE is disabled",
                    snapshot.project_id
                )));
            }
            let input = CreateProject {
                id: snapshot.project_id.clone(),
                name: project_name
                    .unwrap_or(&snapshot.project_id)
                    .to_string(),
                description: None,
                github_repo: None,
                source_root: None,
                line_threshold: None,
                branch_threshold: None,
                line_warn_threshold: None,
                branch_warn_threshold: None,
            };
            self.create_project(&input).await?;
        }

        self.insert_snapshot(snapshot).await?;

        // Ratchet: check floors and advance if improved
        let project = self.get_project(&snapshot.project_id).await?;
        let ratchet = if let Some(ref proj) = project {
            if proj.ratchet_enabled {
                let line_violated = proj.ratchet_line_floor
                    .map(|floor| snapshot.line_rate < floor)
                    .unwrap_or(false);
                let branch_violated = proj.ratchet_branch_floor
                    .map(|floor| snapshot.branch_rate < floor)
                    .unwrap_or(false);

                // Only a release-line build may raise the floor. Advancing on
                // every ingest let a one-off feature branch permanently raise
                // the bar for the whole project — and, because the branch was
                // never merged, raise it to a number no build on the main line
                // could reach again.
                if is_ratchet_branch(snapshot.branch.as_deref()) {
                    self.advance_ratchet_floor(
                        &snapshot.project_id,
                        snapshot.line_rate,
                        snapshot.branch_rate,
                    ).await?;
                }

                RatchetResult {
                    line_floor_violated: line_violated,
                    branch_floor_violated: branch_violated,
                    line_floor: proj.ratchet_line_floor,
                    branch_floor: proj.ratchet_branch_floor,
                }
            } else {
                RatchetResult::default()
            }
        } else {
            RatchetResult::default()
        };

        // Prune old snapshots for this project+target+source series
        self.prune_snapshots(&snapshot.project_id, &snapshot.target, &snapshot.source)
            .await?;

        Ok(ratchet)
    }

    /// Prune old snapshots based on retention limits.
    ///
    /// Retention applies per `(project, target, source)` series so coverage from
    /// different tools (e.g. the native agent vs. Kover) keeps independent
    /// history and one never evicts the other.
    /// - Keep the newest `retention_full` snapshots with full file data.
    /// - Keep the next `retention_summary` snapshots as summary-only (files_json = NULL).
    /// - Delete everything older.
    pub async fn prune_snapshots(
        &self,
        project_id: &str,
        target: &str,
        source: &str,
    ) -> Result<(), sqlx::Error> {
        let settings = self.get_global_settings().await.unwrap_or_default();
        let retention_full = settings.retention_full;
        let retention_summary = settings.retention_summary;
        let retention_total = retention_full + retention_summary;

        // Strip files_json from snapshots beyond the full retention limit
        sqlx::query(
            "UPDATE coverage_snapshots SET files_json = NULL
             WHERE id IN (
               SELECT id FROM coverage_snapshots
               WHERE project_id = ? AND target = ? AND source = ?
               ORDER BY created_at DESC
               LIMIT -1 OFFSET ?
             ) AND files_json IS NOT NULL",
        )
        .bind(project_id)
        .bind(target)
        .bind(source)
        .bind(retention_full)
        .execute(&self.pool)
        .await?;

        // Delete snapshots beyond the total retention limit
        sqlx::query(
            "DELETE FROM coverage_snapshots
             WHERE id IN (
               SELECT id FROM coverage_snapshots
               WHERE project_id = ? AND target = ? AND source = ?
               ORDER BY created_at DESC
               LIMIT -1 OFFSET ?
             )",
        )
        .bind(project_id)
        .bind(target)
        .bind(source)
        .bind(retention_total)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get system health stats: project count, snapshot count, DB size, last ingest time.
    pub async fn get_health_stats(&self) -> Result<HealthStats, sqlx::Error> {
        let project_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM projects")
            .fetch_one(&self.pool)
            .await?;

        let snapshot_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM coverage_snapshots")
            .fetch_one(&self.pool)
            .await?;

        let last_ingest: Option<String> = sqlx::query_scalar(
            "SELECT created_at FROM coverage_snapshots ORDER BY created_at DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        // Get DB file size via PRAGMA
        let db_page_count: i64 = sqlx::query_scalar("PRAGMA page_count")
            .fetch_one(&self.pool)
            .await?;
        let db_page_size: i64 = sqlx::query_scalar("PRAGMA page_size")
            .fetch_one(&self.pool)
            .await?;
        let db_size_bytes = db_page_count * db_page_size;

        Ok(HealthStats {
            project_count,
            snapshot_count,
            last_ingest,
            db_size_bytes,
        })
    }

    // -- Sessions --

    pub async fn create_session(
        &self,
        github_username: &str,
        github_token: &str,
        avatar_url: Option<&str>,
    ) -> Result<Session, sqlx::Error> {
        let id = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let now = Utc::now();
        let expires_at = now + chrono::Duration::days(7);

        sqlx::query(
            "INSERT INTO sessions (id, github_username, github_token, avatar_url, created_at, expires_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(github_username)
        // Encrypted when OMNIVORE_SECRET_KEY is configured; stored as-is
        // otherwise, so upgrading does not require the operator to act first.
        .bind(crate::crypto::encrypt(github_token))
        .bind(avatar_url)
        .bind(now.to_rfc3339())
        .bind(expires_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(Session {
            id,
            github_username: github_username.to_string(),
            github_token: github_token.to_string(),
            avatar_url: avatar_url.map(String::from),
            created_at: now,
            expires_at,
        })
    }

    /// Look up a live session, decrypting its stored GitHub token.
    ///
    /// A session whose token cannot be decrypted is treated as absent rather
    /// than surfaced with a broken token: that happens when
    /// `OMNIVORE_SECRET_KEY` is missing or has changed, and the right outcome is
    /// to send the user back through login rather than to make GitHub calls
    /// with garbage.
    pub async fn get_session(&self, session_id: &str) -> Result<Option<Session>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let session = sqlx::query_as!(
            Session,
            r#"SELECT id as "id!: String",
                      github_username as "github_username!: String",
                      github_token as "github_token!: String",
                      avatar_url as "avatar_url: String",
                      created_at as "created_at!: DateTime<Utc>",
                      expires_at as "expires_at!: DateTime<Utc>"
               FROM sessions
               WHERE id = ? AND expires_at > ?"#,
            session_id,
            now
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(mut session) = session else {
            return Ok(None);
        };

        match crate::crypto::decrypt(&session.github_token) {
            Ok(token) => {
                session.github_token = token;
                Ok(Some(session))
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    username = %session.github_username,
                    "Discarding session with unreadable token"
                );
                Ok(None)
            }
        }
    }

    pub async fn delete_session(&self, session_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn prune_expired_sessions(&self) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // -- Permission cache --

    pub async fn get_cached_permission(
        &self,
        user_id: &str,
        repo: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        let perm: Option<String> = sqlx::query_scalar(
            "SELECT permission FROM permission_cache
             WHERE user_id = ? AND repo = ? AND expires_at > ?",
        )
        .bind(user_id)
        .bind(repo)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;

        Ok(perm)
    }

    pub async fn cache_permission(
        &self,
        user_id: &str,
        repo: &str,
        permission: &str,
    ) -> Result<(), sqlx::Error> {
        let expires_at = (Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
        sqlx::query(
            "INSERT OR REPLACE INTO permission_cache (user_id, repo, permission, expires_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(user_id)
        .bind(repo)
        .bind(permission)
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn prune_permission_cache(&self) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("DELETE FROM permission_cache WHERE expires_at <= ?")
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Whether ingest may create a project it has not seen before.
///
/// Defaults to on, which is what makes first-run setup a single `curl`. Set
/// `OMNIVORE_ALLOW_PROJECT_AUTOCREATE=false` on a shared or exposed instance so
/// coverage can only be uploaded to projects someone created deliberately.
fn auto_create_projects() -> bool {
    match std::env::var("OMNIVORE_ALLOW_PROJECT_AUTOCREATE") {
        Ok(v) if !v.trim().is_empty() => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
        _ => true,
    }
}

/// Branches whose snapshots are allowed to raise a project's ratchet floor.
///
/// Defaults to the usual release lines; override with a comma-separated
/// `OMNIVORE_RATCHET_BRANCHES`. A snapshot with no branch recorded still
/// counts, since single-branch setups often omit it.
fn is_ratchet_branch(branch: Option<&str>) -> bool {
    let Some(branch) = branch.map(str::trim).filter(|b| !b.is_empty()) else {
        return true;
    };
    match std::env::var("OMNIVORE_RATCHET_BRANCHES") {
        Ok(list) if !list.trim().is_empty() => list
            .split(',')
            .map(str::trim)
            .any(|allowed| !allowed.is_empty() && allowed.eq_ignore_ascii_case(branch)),
        _ => branch.eq_ignore_ascii_case("main") || branch.eq_ignore_ascii_case("master"),
    }
}

pub struct ActivityEntry {
    pub created_at: DateTime<Utc>,
    pub project_id: String,
    pub project_name: String,
    pub target: String,
    pub commit_sha: Option<String>,
    pub line_rate: f64,
    pub branch_rate: f64,
    pub lines_covered: i64,
    pub lines_total: i64,
}

pub struct HealthStats {
    pub project_count: i64,
    pub snapshot_count: i64,
    pub last_ingest: Option<String>,
    pub db_size_bytes: i64,
}
