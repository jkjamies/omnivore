use super::comment::COMMENT_MARKER;
use serde::{Deserialize, Serialize};

/// GitHub API client for posting PR comments.
///
/// Uses the GitHub REST API v3. Requires a personal access token or
/// GitHub App token with `pull_requests:write` permission.
#[derive(Clone)]
pub struct GitHubClient {
    token: String,
    api_base: String,
    http: reqwest::Client,
}

#[derive(Serialize)]
struct CommentBody {
    body: String,
}

impl GitHubClient {
    /// Create a new client.
    /// - `token`: GitHub token (PAT or `GITHUB_TOKEN` from Actions)
    /// - `api_base`: defaults to `https://api.github.com` if None
    pub fn new(token: String, api_base: Option<String>) -> Self {
        Self {
            token,
            api_base: api_base.unwrap_or_else(|| "https://api.github.com".into()),
            http: reqwest::Client::new(),
        }
    }

    /// Post or update a coverage comment on a PR.
    ///
    /// If an existing Omnivore comment is found (by marker), it updates it.
    /// Otherwise, it creates a new comment.
    pub async fn post_or_update_comment(
        &self,
        repo: &str,
        pr_number: u64,
        body: &str,
    ) -> Result<(), String> {
        // Try to find an existing Omnivore comment
        let existing_id = self.find_existing_comment(repo, pr_number).await?;

        match existing_id {
            Some(comment_id) => {
                self.update_comment(repo, comment_id, body).await?;
                tracing::info!("Updated existing PR comment {comment_id} on {repo}#{pr_number}");
            }
            None => {
                self.create_comment(repo, pr_number, body).await?;
                tracing::info!("Created new PR comment on {repo}#{pr_number}");
            }
        }

        Ok(())
    }

    /// Find an existing comment with the Omnivore marker.
    async fn find_existing_comment(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> Result<Option<i64>, String> {
        let url = format!(
            "{}/repos/{}/issues/{}/comments?per_page=100",
            self.api_base, repo, pr_number
        );

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "omnivore-dashboard")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("GitHub API returned {status}: {body}"));
        }

        let comments: Vec<CommentListItem> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse comments: {e}"))?;

        Ok(comments
            .iter()
            .find(|c| c.body.as_deref().unwrap_or("").contains(COMMENT_MARKER))
            .map(|c| c.id))
    }

    async fn create_comment(
        &self,
        repo: &str,
        pr_number: u64,
        body: &str,
    ) -> Result<(), String> {
        let url = format!(
            "{}/repos/{}/issues/{}/comments",
            self.api_base, repo, pr_number
        );

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "omnivore-dashboard")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&CommentBody {
                body: body.to_string(),
            })
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("GitHub API returned {status}: {body}"));
        }

        Ok(())
    }

    async fn update_comment(
        &self,
        repo: &str,
        comment_id: i64,
        body: &str,
    ) -> Result<(), String> {
        let url = format!(
            "{}/repos/{}/issues/comments/{}",
            self.api_base, repo, comment_id
        );

        let resp = self
            .http
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "omnivore-dashboard")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&CommentBody {
                body: body.to_string(),
            })
            .send()
            .await
            .map_err(|e| format!("GitHub API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("GitHub API returned {status}: {body}"));
        }

        Ok(())
    }
}

#[derive(Deserialize)]
struct CommentListItem {
    id: i64,
    body: Option<String>,
}
