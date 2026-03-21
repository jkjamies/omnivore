package validation

import (
	"strings"
	"unicode"

	"github.com/jkjamies/omnivore/go-test-rig/model"
)

// ValidateTitle checks that a task title is valid.
func ValidateTitle(title string) error {
	trimmed := strings.TrimSpace(title)
	if trimmed == "" {
		return model.ErrValidation("title cannot be empty")
	}
	if len(trimmed) > 200 {
		return model.ErrValidation("title cannot exceed 200 characters")
	}
	if strings.ContainsRune(trimmed, '\n') || strings.ContainsRune(trimmed, '\r') {
		return model.ErrValidation("title cannot contain newlines")
	}
	return nil
}

// ValidateTag checks that a tag name is valid.
func ValidateTag(tag string) error {
	trimmed := strings.TrimSpace(tag)
	if trimmed == "" {
		return model.ErrValidation("tag cannot be empty")
	}
	if len(trimmed) > 50 {
		return model.ErrValidation("tag cannot exceed 50 characters")
	}
	for _, ch := range trimmed {
		if !unicode.IsLetter(ch) && !unicode.IsDigit(ch) && ch != '-' && ch != '_' {
			return model.ErrValidation("tag can only contain alphanumeric characters, hyphens, and underscores")
		}
	}
	return nil
}

// SanitizeDescription trims and collapses whitespace runs.
func SanitizeDescription(desc string) string {
	trimmed := strings.TrimSpace(desc)
	var result strings.Builder
	lastWasSpace := false
	for _, ch := range trimmed {
		if unicode.IsSpace(ch) {
			if !lastWasSpace {
				result.WriteRune(' ')
				lastWasSpace = true
			}
		} else {
			result.WriteRune(ch)
			lastWasSpace = false
		}
	}
	return result.String()
}
