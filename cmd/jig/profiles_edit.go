package main

import (
	"fmt"
	"os"
	"os/exec"

	"github.com/jdforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var profilesEditCmd = &cobra.Command{
	Use:   "edit <name>",
	Short: "Edit an existing profile in $EDITOR",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		name := args[0]
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		// Find the profile to get its file path
		p, err := config.LoadProfile(name, cwd)
		if err != nil {
			return fmt.Errorf("profile %q not found: %w", name, err)
		}

		editor := os.Getenv("EDITOR")
		if editor == "" {
			editor = "vi"
		}

		editorCmd := exec.Command(editor, p.FilePath())
		editorCmd.Stdin = os.Stdin
		editorCmd.Stdout = os.Stdout
		editorCmd.Stderr = os.Stderr

		if err := editorCmd.Run(); err != nil {
			return fmt.Errorf("editor exited with error: %w", err)
		}

		// Reload and validate
		edited, err := config.LoadProfileFromFile(p.FilePath())
		if err != nil {
			fmt.Fprintf(os.Stderr, "Warning: edited profile has parse errors: %v\n", err)
			return nil
		}

		if err := config.Validate(edited); err != nil {
			fmt.Fprintf(os.Stderr, "Warning: edited profile has validation errors: %v\n", err)
		}

		fmt.Printf("Updated profile: %s\n", name)
		return nil
	},
}
