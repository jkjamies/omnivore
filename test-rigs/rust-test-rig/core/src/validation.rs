use crate::model::TaskError;

/// Validate a task title.
pub fn validate_title(title: &str) -> Result<(), TaskError> {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return Err(TaskError::ValidationError("Title cannot be empty".into()));
    }
    if trimmed.len() > 200 {
        return Err(TaskError::ValidationError(
            "Title cannot exceed 200 characters".into(),
        ));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(TaskError::ValidationError(
            "Title cannot contain newlines".into(),
        ));
    }
    Ok(())
}

/// Validate a tag name.
pub fn validate_tag(tag: &str) -> Result<(), TaskError> {
    let trimmed = tag.trim();
    if trimmed.is_empty() {
        return Err(TaskError::ValidationError("Tag cannot be empty".into()));
    }
    if trimmed.len() > 50 {
        return Err(TaskError::ValidationError(
            "Tag cannot exceed 50 characters".into(),
        ));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(TaskError::ValidationError(
            "Tag can only contain alphanumeric characters, hyphens, and underscores".into(),
        ));
    }
    Ok(())
}

/// Sanitize a description by trimming and collapsing whitespace runs.
pub fn sanitize_description(desc: &str) -> String {
    let trimmed = desc.trim();
    let mut result = String::with_capacity(trimmed.len());
    let mut last_was_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_title() {
        assert!(validate_title("Fix the bug").is_ok());
    }

    #[test]
    fn empty_title_rejected() {
        assert!(validate_title("").is_err());
        assert!(validate_title("   ").is_err());
    }

    #[test]
    fn long_title_rejected() {
        let long = "a".repeat(201);
        assert!(validate_title(&long).is_err());
    }

    #[test]
    fn newline_title_rejected() {
        assert!(validate_title("line1\nline2").is_err());
    }

    #[test]
    fn valid_tag() {
        assert!(validate_tag("bug-fix").is_ok());
        assert!(validate_tag("v2_feature").is_ok());
    }

    #[test]
    fn empty_tag_rejected() {
        assert!(validate_tag("").is_err());
    }

    #[test]
    fn special_chars_tag_rejected() {
        assert!(validate_tag("bug fix").is_err());
        assert!(validate_tag("tag@name").is_err());
    }

    #[test]
    fn sanitize_collapses_whitespace() {
        assert_eq!(sanitize_description("  hello   world  "), "hello world");
    }

    // Intentionally NOT testing: sanitize_description with tabs/newlines, validate_tag long input
}
