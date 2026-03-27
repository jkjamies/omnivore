use crate::model::api_key::{ApiKey, ApiKeyCreated};
use crate::model::coverage::CoverageSnapshot;
use crate::model::project::{CreateProject, Project};
use crate::model::session::Session;
use crate::model::settings::GlobalSettings;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        let db = Self { pool };
        db.run_migrations().await?;
        Ok(db)
    }

    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                github_repo TEXT,
                source_root TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS coverage_snapshots (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL REFERENCES projects(id),
                commit_sha TEXT,
                branch TEXT,
                target TEXT NOT NULL,
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
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_snapshots_project
             ON coverage_snapshots(project_id, created_at DESC)",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS source_cache (
                repo TEXT NOT NULL,
                path TEXT NOT NULL,
                commit_ref TEXT NOT NULL DEFAULT '',
                content TEXT NOT NULL,
                fetched_at TEXT NOT NULL,
                PRIMARY KEY (repo, path, commit_ref)
            )",
        )
        .execute(&self.pool)
        .await?;

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

        // Global settings table (single row, id=1)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS settings (
                id INTEGER PRIMARY KEY CHECK(id = 1),
                default_line_threshold REAL NOT NULL DEFAULT 0.8,
                default_branch_threshold REAL NOT NULL DEFAULT 0.8,
                default_line_warn_threshold REAL NOT NULL DEFAULT 0.5,
                default_branch_warn_threshold REAL NOT NULL DEFAULT 0.5
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("INSERT OR IGNORE INTO settings (id) VALUES (1)")
            .execute(&self.pool)
            .await?;

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

        // API keys table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS api_keys (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                key_hash TEXT NOT NULL,
                key_prefix TEXT NOT NULL,
                project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
                created_at TEXT NOT NULL,
                last_used_at TEXT
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash)",
        )
        .execute(&self.pool)
        .await?;

        // Sessions table (OAuth)
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                github_username TEXT NOT NULL,
                github_token TEXT NOT NULL,
                avatar_url TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        // Permission cache table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS permission_cache (
                user_id TEXT NOT NULL,
                repo TEXT NOT NULL,
                permission TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                PRIMARY KEY (user_id, repo)
            )",
        )
        .execute(&self.pool)
        .await?;

        // Enable foreign keys for CASCADE support
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&self.pool)
            .await?;

        Ok(())
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

    pub async fn delete_project(&self, id: &str) -> Result<(), sqlx::Error> {
        // Delete snapshots first (foreign key), then source cache, then project
        sqlx::query("DELETE FROM coverage_snapshots WHERE project_id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
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
             (id, project_id, commit_sha, branch, target, line_rate, branch_rate,
              lines_covered, lines_total, branches_covered, branches_total,
              file_count, created_at, files_json, dependencies_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&snapshot.id)
        .bind(&snapshot.project_id)
        .bind(&snapshot.commit_sha)
        .bind(&snapshot.branch)
        .bind(&snapshot.target)
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
                      target, line_rate, branch_rate,
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
                      target, line_rate, branch_rate,
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
                      target, line_rate, branch_rate,
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
                      target, line_rate, branch_rate,
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
                      target, line_rate, branch_rate,
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
                      target, line_rate, branch_rate,
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
                      target, line_rate, branch_rate,
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

    // -- Source cache --

    /// Look up cached source content.
    pub async fn get_cached_source(
        &self,
        repo: &str,
        path: &str,
        commit_ref: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let ref_key = if commit_ref.is_empty() { "" } else { commit_ref };
        sqlx::query_scalar::<_, String>(
            "SELECT content FROM source_cache WHERE repo = ? AND path = ? AND commit_ref = ?",
        )
        .bind(repo)
        .bind(path)
        .bind(ref_key)
        .fetch_optional(&self.pool)
        .await
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
    pub async fn ingest_snapshot(
        &self,
        snapshot: &CoverageSnapshot,
        project_name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        // Ensure project exists
        if self.get_project(&snapshot.project_id).await?.is_none() {
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

        // Prune old snapshots for this project+target
        let target = &snapshot.target;
        self.prune_snapshots(&snapshot.project_id, target).await?;

        Ok(())
    }

    /// Prune old snapshots based on retention limits.
    /// - Keep the newest `retention_full` snapshots with full file data.
    /// - Keep the next `retention_summary` snapshots as summary-only (files_json = NULL).
    /// - Delete everything older.
    pub async fn prune_snapshots(
        &self,
        project_id: &str,
        target: &str,
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
               WHERE project_id = ? AND target = ?
               ORDER BY created_at DESC
               LIMIT -1 OFFSET ?
             ) AND files_json IS NOT NULL",
        )
        .bind(project_id)
        .bind(target)
        .bind(retention_full)
        .execute(&self.pool)
        .await?;

        // Delete snapshots beyond the total retention limit
        sqlx::query(
            "DELETE FROM coverage_snapshots
             WHERE id IN (
               SELECT id FROM coverage_snapshots
               WHERE project_id = ? AND target = ?
               ORDER BY created_at DESC
               LIMIT -1 OFFSET ?
             )",
        )
        .bind(project_id)
        .bind(target)
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
        .bind(github_token)
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

        Ok(session)
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
