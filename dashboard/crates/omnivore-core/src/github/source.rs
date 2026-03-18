use reqwest::Client;

/// Fetch a file's content from GitHub at a specific commit.
///
/// Uses the GitHub REST API: `GET /repos/{owner}/{repo}/contents/{path}?ref={sha}`
/// Returns the decoded file content, or None if not found / error.
pub async fn fetch_source(
    github_repo: &str,
    file_path: &str,
    commit_sha: Option<&str>,
    github_token: Option<&str>,
) -> Option<String> {
    let client = Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/contents/{}",
        github_repo, file_path
    );

    let mut req = client
        .get(&url)
        .header("Accept", "application/vnd.github.v3.raw")
        .header("User-Agent", "omnivore-dashboard");

    if let Some(sha) = commit_sha {
        req = req.query(&[("ref", sha)]);
    }

    if let Some(token) = github_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let resp = req.send().await.ok()?;
    if resp.status().is_success() {
        resp.text().await.ok()
    } else {
        None
    }
}
