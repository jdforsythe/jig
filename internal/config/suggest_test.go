package config

import "testing"

func TestSuggestProfile(t *testing.T) {
	profiles := []Profile{
		{Name: "code-review"},
		{Name: "quick-fix"},
		{Name: "backend"},
		{Name: "base"},
	}

	tests := []struct {
		input    string
		wantName string
		wantOK   bool
	}{
		// Close match (distance ≤ 3)
		{"code-reveiw", "code-review", true},
		{"qick-fix", "quick-fix", true},
		{"backnd", "backend", true},
		{"bse", "base", true},

		// Exact match
		{"code-review", "code-review", true},

		// Too different (distance > 3)
		{"completely-different-xyz", "", false},
		{"frontend", "", false},

		// Empty query — distance to shortest name ("base") is 4, which exceeds threshold 3
		{"", "", false},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got, ok := SuggestProfile(tt.input, profiles)
			if ok != tt.wantOK {
				t.Errorf("SuggestProfile(%q) ok = %v, want %v", tt.input, ok, tt.wantOK)
			}
			if ok && got != tt.wantName {
				t.Errorf("SuggestProfile(%q) = %q, want %q", tt.input, got, tt.wantName)
			}
		})
	}
}

func TestSuggestProfile_EmptyList(t *testing.T) {
	_, ok := SuggestProfile("anything", nil)
	if ok {
		t.Error("SuggestProfile() with nil profiles should return ok=false")
	}

	_, ok = SuggestProfile("anything", []Profile{})
	if ok {
		t.Error("SuggestProfile() with empty profiles should return ok=false")
	}
}

func TestSuggestProfile_SingleProfile(t *testing.T) {
	profiles := []Profile{{Name: "only-one"}}

	// Close enough
	got, ok := SuggestProfile("only-on", profiles)
	if !ok {
		t.Fatal("expected suggestion")
	}
	if got != "only-one" {
		t.Errorf("got %q, want only-one", got)
	}

	// Too different
	_, ok = SuggestProfile("completely-different-name", profiles)
	if ok {
		t.Error("expected no suggestion for very different name")
	}
}

func TestSuggestProfile_PicksClosest(t *testing.T) {
	profiles := []Profile{
		{Name: "staging"},
		{Name: "prod"},
		{Name: "production"},
	}

	// "prduction" is closer to "production" (1 edit) than "prod" (4 edits)
	got, ok := SuggestProfile("prduction", profiles)
	if !ok {
		t.Fatal("expected suggestion")
	}
	if got != "production" {
		t.Errorf("got %q, want production", got)
	}
}
