use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;

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

// -- Repo tree cache for file path resolution --

#[derive(Deserialize)]
struct TreeResponse {
    tree: Vec<TreeEntry>,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
    #[serde(rename = "type")]
    entry_type: String,
}

/// In-memory cache: repo → { filename → full_path }.
/// Keyed by "owner/repo" so each repo is fetched once per server lifetime.
static TREE_CACHE: std::sync::LazyLock<Mutex<HashMap<String, HashMap<String, Vec<String>>>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Fetch the repo's file tree (single API call, cached) and find the best match
/// for a coverage file path like `com/example/Foo.kt`.
///
/// Returns the full repo-relative path (e.g., `app/src/main/java/com/example/Foo.kt`).
pub async fn resolve_file_path(
    github_repo: &str,
    coverage_path: &str,
    github_token: Option<&str>,
) -> Option<String> {
    let tree = get_or_fetch_tree(github_repo, github_token).await?;

    // Exact suffix match: find entries ending with the coverage path
    let suffix = format!("/{}", coverage_path);
    let mut matches: Vec<&String> = tree.values()
        .flatten()
        .filter(|full_path| full_path.ends_with(&suffix) || *full_path == coverage_path)
        .collect();

    if matches.len() == 1 {
        return Some(matches[0].clone());
    }

    // Multiple matches — prefer src/main paths (Java/Kotlin convention)
    if matches.len() > 1 {
        let src_main: Vec<&&String> = matches.iter()
            .filter(|p| p.contains("/src/main/"))
            .collect();
        if src_main.len() == 1 {
            return Some((**src_main[0]).clone());
        }
        // If still ambiguous, return the shortest path
        matches.sort_by_key(|p| p.len());
        return Some(matches[0].clone());
    }

    // No suffix match — try matching just the filename
    let filename = coverage_path.rsplit('/').next()?;
    if let Some(paths) = tree.get(filename) {
        if paths.len() == 1 {
            return Some(paths[0].clone());
        }
        // Multiple files with same name — try to match partial path
        let parts: Vec<&str> = coverage_path.split('/').collect();
        let mut best: Option<(&String, usize)> = None;
        for path in paths {
            let path_parts: Vec<&str> = path.split('/').collect();
            let overlap = parts.iter().rev().zip(path_parts.iter().rev())
                .take_while(|(a, b)| a == b)
                .count();
            if best.is_none() || overlap > best.unwrap().1 {
                best = Some((path, overlap));
            }
        }
        return best.map(|(p, _)| p.clone());
    }

    None
}

async fn get_or_fetch_tree(
    github_repo: &str,
    github_token: Option<&str>,
) -> Option<HashMap<String, Vec<String>>> {
    // Check cache
    {
        let cache = TREE_CACHE.lock().ok()?;
        if let Some(tree) = cache.get(github_repo) {
            return Some(tree.clone());
        }
    }

    // Fetch from GitHub Git Trees API (recursive, single call)
    let client = Client::new();
    let url = format!(
        "https://api.github.com/repos/{}/git/trees/HEAD?recursive=1",
        github_repo
    );

    let mut req = client
        .get(&url)
        .header("User-Agent", "omnivore-dashboard")
        .header("Accept", "application/vnd.github+json");

    if let Some(token) = github_token {
        req = req.header("Authorization", format!("Bearer {}", token));
    }

    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        tracing::warn!(repo = %github_repo, status = %resp.status(), "Failed to fetch repo tree");
        return None;
    }

    let tree_resp: TreeResponse = resp.json().await.ok()?;

    // Build filename → [full_paths] index (only blobs, not directories)
    let mut index: HashMap<String, Vec<String>> = HashMap::new();
    for entry in &tree_resp.tree {
        if entry.entry_type == "blob" {
            let filename = entry.path.rsplit('/').next().unwrap_or(&entry.path);
            index.entry(filename.to_string())
                .or_default()
                .push(entry.path.clone());
        }
    }

    tracing::info!(repo = %github_repo, files = index.values().map(|v| v.len()).sum::<usize>(), "Cached repo file tree");

    // Cache it
    {
        if let Ok(mut cache) = TREE_CACHE.lock() {
            cache.insert(github_repo.to_string(), index.clone());
        }
    }

    Some(index)
}

/// Clear the cached tree for a repo (e.g., after a new ingest with a different commit).
pub fn invalidate_tree_cache(github_repo: &str) {
    if let Ok(mut cache) = TREE_CACHE.lock() {
        cache.remove(github_repo);
    }
}
