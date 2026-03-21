package validation

import (
	"strings"
	"testing"
)

func TestValidateTitle_Valid(t *testing.T) {
	if err := ValidateTitle("Fix the bug"); err != nil {
		t.Errorf("expected valid, got: %v", err)
	}
}

func TestValidateTitle_Empty(t *testing.T) {
	if err := ValidateTitle(""); err == nil {
		t.Error("expected error for empty title")
	}
	if err := ValidateTitle("   "); err == nil {
		t.Error("expected error for whitespace-only title")
	}
}

func TestValidateTitle_TooLong(t *testing.T) {
	long := strings.Repeat("a", 201)
	if err := ValidateTitle(long); err == nil {
		t.Error("expected error for long title")
	}
}

func TestValidateTitle_Newline(t *testing.T) {
	if err := ValidateTitle("line1\nline2"); err == nil {
		t.Error("expected error for newline in title")
	}
}

func TestValidateTag_Valid(t *testing.T) {
	if err := ValidateTag("bug-fix"); err != nil {
		t.Errorf("expected valid, got: %v", err)
	}
	if err := ValidateTag("v2_feature"); err != nil {
		t.Errorf("expected valid, got: %v", err)
	}
}

func TestValidateTag_Empty(t *testing.T) {
	if err := ValidateTag(""); err == nil {
		t.Error("expected error for empty tag")
	}
}

func TestValidateTag_SpecialChars(t *testing.T) {
	if err := ValidateTag("bug fix"); err == nil {
		t.Error("expected error for space in tag")
	}
	if err := ValidateTag("tag@name"); err == nil {
		t.Error("expected error for @ in tag")
	}
}

func TestSanitizeDescription(t *testing.T) {
	got := SanitizeDescription("  hello   world  ")
	want := "hello world"
	if got != want {
		t.Errorf("got %q, want %q", got, want)
	}
}

// Intentionally NOT testing: SanitizeDescription with tabs/newlines, ValidateTag long input
