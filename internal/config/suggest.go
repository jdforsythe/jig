package config

import "github.com/agnivade/levenshtein"

// SuggestProfile finds the closest matching profile name using Levenshtein distance.
// Returns the suggestion and whether it's close enough to suggest (distance <= 3).
func SuggestProfile(name string, profiles []Profile) (string, bool) {
	if len(profiles) == 0 {
		return "", false
	}

	best := ""
	bestDist := 999

	for _, p := range profiles {
		d := levenshtein.ComputeDistance(name, p.Name)
		if d < bestDist {
			bestDist = d
			best = p.Name
		}
	}

	if bestDist <= 3 {
		return best, true
	}
	return "", false
}
