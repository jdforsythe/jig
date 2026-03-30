package main

import (
	"fmt"
	"os"

	"github.com/jforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var initCmd = &cobra.Command{
	Use:   "init",
	Short: "Initialize jig in the current directory",
	Long:  "Creates .jig/profiles/ in the current directory for project-level profiles.",
	RunE: func(cmd *cobra.Command, args []string) error {
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		dir := config.ProjectProfilesDir(cwd)
		if _, err := os.Stat(dir); err == nil {
			fmt.Println("Already initialized:", dir)
			return nil
		}

		if err := config.EnsureDir(dir); err != nil {
			return fmt.Errorf("creating directory: %w", err)
		}

		// Write a .gitkeep so the directory is tracked
		gitkeep := dir + "/.gitkeep"
		if err := os.WriteFile(gitkeep, []byte(""), 0644); err != nil {
			return err
		}

		fmt.Println("Initialized jig project config:", dir)
		fmt.Println("Create a profile: jig profiles create <name>")
		return nil
	},
}
