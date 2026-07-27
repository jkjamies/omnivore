use crate::validation::{is_safe_repo_path, is_valid_repo_slug};
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
    // Both values land in the URL path, and both originate from data the
    // dashboard accepted over the network (project settings and coverage
    // reports). Validate before building the request.
    if !is_valid_repo_slug(github_repo) || !is_safe_repo_path(file_path) {
        tracing::warn!(repo = %github_repo, path = %file_path, "Rejected unsafe source fetch");
        return None;
    }
    if commit_sha.is_some_and(|sha| !is_valid_git_ref(sha)) {
        return None;
    }

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

/// A git ref safe to interpolate into a URL path: a commit SHA, tag, or branch
/// name without traversal or separators of its own.
fn is_valid_git_ref(git_ref: &str) -> bool {
    !git_ref.is_empty()
        && git_ref.len() <= 100
        && git_ref != "."
        && git_ref != ".."
        && git_ref
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
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

/// In-memory cache: repo → (fetched_at, { filename → full_paths }).
///
/// This used to be keyed by repo with no expiry and no bound: every repo ever
/// viewed kept its entire file list resident for the life of the process, and a
/// file added or renamed upstream never appeared until a restart
/// (`invalidate_tree_cache` existed but nothing called it). Entries now expire,
/// and the map is capped.
static TREE_CACHE: std::sync::LazyLock<Mutex<HashMap<String, CachedTree>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

struct CachedTree {
    fetched_at: std::time::Instant,
    index: HashMap<String, Vec<String>>,
}

/// How long a repo's file listing stays usable.
///
/// The tree is only used to map a coverage path onto a repo path, so staleness
/// costs a re-resolve, not correctness. Fifteen minutes keeps a browsing
/// session to one API call while still picking up renames.
const TREE_TTL: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Cap on distinct repos held in memory. A dashboard with many linked projects
/// would otherwise accumulate every one of their file listings.
const TREE_CACHE_MAX_REPOS: usize = 32;

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
    if !is_valid_repo_slug(github_repo) {
        tracing::warn!(repo = %github_repo, "Rejected tree fetch for invalid repo slug");
        return None;
    }

    // Check cache
    {
        let mut cache = TREE_CACHE.lock().ok()?;
        // Drop anything past its TTL while we hold the lock — cheap, and it
        // keeps the map from growing with repos nobody looks at any more.
        cache.retain(|_, entry| entry.fetched_at.elapsed() < TREE_TTL);
        if let Some(entry) = cache.get(github_repo) {
            return Some(entry.index.clone());
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
            if cache.len() >= TREE_CACHE_MAX_REPOS {
                // Evict the oldest rather than clearing everything, so one new
                // repo doesn't cost every active session its cache.
                if let Some(oldest) = cache
                    .iter()
                    .min_by_key(|(_, e)| e.fetched_at)
                    .map(|(k, _)| k.clone())
                {
                    cache.remove(&oldest);
                }
            }
            cache.insert(
                github_repo.to_string(),
                CachedTree {
                    fetched_at: std::time::Instant::now(),
                    index: index.clone(),
                },
            );
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
