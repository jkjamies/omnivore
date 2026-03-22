use crate::model::coverage::CoverageSnapshot;
use crate::model::project::{CreateProject, Project};
use crate::model::settings::GlobalSettings;
use chrono::{DateTime, Utc};
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

    // -- Global settings --

    pub async fn get_global_settings(&self) -> Result<GlobalSettings, sqlx::Error> {
        let row = sqlx::query_as::<_, (f64, f64, f64, f64)>(
            "SELECT default_line_threshold, default_branch_threshold, default_line_warn_threshold, default_branch_warn_threshold FROM settings WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .map(|(lt, bt, lwt, bwt)| GlobalSettings {
                default_line_threshold: lt,
                default_branch_threshold: bt,
                default_line_warn_threshold: lwt,
                default_branch_warn_threshold: bwt,
            })
            .unwrap_or_default())
    }

    pub async fn update_global_settings(
        &self,
        settings: &GlobalSettings,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE settings SET default_line_threshold = ?, default_branch_threshold = ?, default_line_warn_threshold = ?, default_branch_warn_threshold = ? WHERE id = 1",
        )
        .bind(settings.default_line_threshold)
        .bind(settings.default_branch_threshold)
        .bind(settings.default_line_warn_threshold)
        .bind(settings.default_branch_warn_threshold)
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
        let retention_full: i64 = std::env::var("OMNIVORE_RETENTION_FULL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        let retention_summary: i64 = std::env::var("OMNIVORE_RETENTION_SUMMARY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);
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
}
