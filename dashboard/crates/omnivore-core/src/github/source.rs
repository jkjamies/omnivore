use reqwest::Client;

/// Fetch a file's content from GitHub using raw.githubusercontent.com.
///
/// This is faster than the Contents API — no JSON encoding/decoding overhead.
/// URL format: `https://raw.githubusercontent.com/{owner}/{repo}/{ref}/{path}`
///
/// Falls back to HEAD of the default branch if no commit SHA is provided.
pub async fn fetch_source(
    github_repo: &str,
    file_path: &str,
    commit_sha: Option<&str>,
    github_token: Option<&str>,
) -> Option<String> {
    let client = Client::new();
    let git_ref = commit_sha.unwrap_or("HEAD");
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}",
        github_repo, git_ref, file_path
    );

    let mut req = client
        .get(&url)
        .header("User-Agent", "omnivore-dashboard");

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
