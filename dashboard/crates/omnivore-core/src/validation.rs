//! Input validation for values that get interpolated into outbound URLs.
//!
//! Several code paths build GitHub API / raw.githubusercontent.com URLs by
//! formatting caller-supplied strings straight into a path
//! (`{api_base}/repos/{repo}/issues/{n}/comments`). Without validation a value
//! like `owner/repo/../../user` lets a caller pivot the request to a different
//! API endpoint once the HTTP client normalises the path. These helpers are the
//! single gate every such value goes through.

/// Validate a GitHub repository slug (`owner/name`).
///
/// Accepts exactly two non-empty segments made of characters GitHub actually
/// allows in owner and repository names. Notably rejects `.`/`..` segments,
/// extra slashes, whitespace, and anything that could alter the shape of a URL
/// path it is interpolated into.
pub fn is_valid_repo_slug(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    is_valid_owner(owner) && is_valid_repo_name(name)
}

fn is_valid_owner(owner: &str) -> bool {
    !owner.is_empty()
        && owner.len() <= 39
        && owner != "."
        && owner != ".."
        && owner
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_valid_repo_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Validate a repository-relative file path before it is appended to a URL.
///
/// Rejects absolute paths, `..` traversal, empty segments, and control
/// characters. Coverage reports supply these paths, so they are attacker
/// controlled on an open dashboard.
pub fn is_safe_repo_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 1024 || path.starts_with('/') {
        return false;
    }
    if path.chars().any(|c| c.is_control() || c == '\\') {
        return false;
    }
    path.split('/')
        .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_slugs() {
        assert!(is_valid_repo_slug("jkjamies/omnivore"));
        assert!(is_valid_repo_slug("some-org/my_repo.js"));
    }

    #[test]
    fn rejects_path_pivots_and_junk() {
        // The traversal cases are the reason this function exists: a client
        // controlled slug must not be able to retarget the API path.
        assert!(!is_valid_repo_slug("owner/repo/../../user"));
        assert!(!is_valid_repo_slug("../../user"));
        assert!(!is_valid_repo_slug("owner"));
        assert!(!is_valid_repo_slug("owner/repo/extra"));
        assert!(!is_valid_repo_slug("owner/"));
        assert!(!is_valid_repo_slug("/repo"));
        assert!(!is_valid_repo_slug(""));
        assert!(!is_valid_repo_slug("own er/repo"));
        assert!(!is_valid_repo_slug("owner/re po"));
        assert!(!is_valid_repo_slug("owner/repo?x=1"));
        assert!(!is_valid_repo_slug("owner/repo#frag"));
        assert!(!is_valid_repo_slug("owner:pass@host/repo"));
    }

    #[test]
    fn accepts_ordinary_paths() {
        assert!(is_safe_repo_path("app/src/main/kotlin/Foo.kt"));
        assert!(is_safe_repo_path("main.go"));
    }

    #[test]
    fn rejects_traversal_paths() {
        assert!(!is_safe_repo_path("../secrets"));
        assert!(!is_safe_repo_path("app/../../etc/passwd"));
        assert!(!is_safe_repo_path("/etc/passwd"));
        assert!(!is_safe_repo_path("app//foo"));
        assert!(!is_safe_repo_path(""));
        assert!(!is_safe_repo_path("a\nb"));
    }
}
