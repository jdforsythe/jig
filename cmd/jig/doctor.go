package main

import (
	"fmt"
	"os"

	"github.com/jdforsythe/jig/internal/claude"
	"github.com/jdforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var doctorCmd = &cobra.Command{
	Use:   "doctor",
	Short: "Check configuration and diagnose issues",
	RunE: func(cmd *cobra.Command, args []string) error {
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		issues := 0

		// Check claude binary
		fmt.Print("Claude Code: ")
		detected, err := claude.Detect()
		if err != nil {
			fmt.Printf("NOT FOUND\n  %v\n", err)
			issues++
		} else {
			fmt.Printf("OK (%s at %s)\n", detected.Version, detected.Path)
		}

		// Check global config dir
		fmt.Print("Global config: ")
		globalDir, err := config.GlobalDir()
		if err != nil {
			fmt.Printf("ERROR (%v)\n", err)
			issues++
		} else if _, err := os.Stat(globalDir); os.IsNotExist(err) {
			fmt.Printf("not initialized (~/.jig/ does not exist)\n")
		} else {
			fmt.Printf("OK (%s)\n", globalDir)
		}

		// Check project config dir
		fmt.Print("Project config: ")
		projDir := config.ProjectDir(cwd)
		if _, err := os.Stat(projDir); os.IsNotExist(err) {
			fmt.Println("not initialized (run 'jig init')")
		} else {
			fmt.Printf("OK (%s)\n", projDir)
		}

		// Check profiles
		fmt.Print("Profiles: ")
		profiles, err := config.ListProfiles(cwd)
		if err != nil {
			fmt.Printf("ERROR (%v)\n", err)
			issues++
		} else {
			fmt.Printf("%d found\n", len(profiles))
		}

		// Validate all profiles
		for _, p := range profiles {
			p := p
			if err := config.Validate(&p); err != nil {
				fmt.Printf("  INVALID: %s - %v\n", p.Name, err)
				issues++
			}
		}

		// Check MCP servers
		fmt.Print("MCP servers: ")
		mcpIndex, err := claude.BuildMCPIndex(cwd)
		if err != nil {
			fmt.Printf("ERROR (%v)\n", err)
			issues++
		} else {
			fmt.Printf("%d found\n", len(mcpIndex.Servers))
		}

		fmt.Println()
		if issues > 0 {
			fmt.Printf("%d issue(s) found.\n", issues)
			return fmt.Errorf("doctor found issues")
		}
		fmt.Println("All checks passed.")
		return nil
	},
}
