use crate::model::coverage::CoverageSnapshot;
use crate::model::project::{CreateProject, Project};
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

        Ok(())
    }

    // -- Projects --

    pub async fn create_project(&self, input: &CreateProject) -> Result<Project, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO projects (id, name, description, github_repo, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&input.id)
        .bind(&input.name)
        .bind(&input.description)
        .bind(&input.github_repo)
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
                      created_at as "created_at!: DateTime<Utc>",
                      updated_at as "updated_at!: DateTime<Utc>"
               FROM projects ORDER BY name"#
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn update_project_github_repo(
        &self,
        id: &str,
        github_repo: Option<&str>,
    ) -> Result<Option<Project>, sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE projects SET github_repo = ?, updated_at = ? WHERE id = ?",
        )
        .bind(github_repo)
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
            };
            self.create_project(&input).await?;
        }

        self.insert_snapshot(snapshot).await
    }
}
