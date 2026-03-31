package main

import (
	"fmt"
	"os"
	"os/exec"

	"github.com/jdforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var (
	createGlobal      bool
	createModel       string
	createEffort      string
	createPerm        string
	createDescription string
)

var profilesCreateCmd = &cobra.Command{
	Use:   "create <name>",
	Short: "Create a new profile",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		name := args[0]
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		p := &config.Profile{
			Name:           name,
			Description:    createDescription,
			Model:          createModel,
			Effort:         createEffort,
			PermissionMode: createPerm,
		}

		// If no flags provided, open $EDITOR
		if createModel == "" && createEffort == "" && createPerm == "" && createDescription == "" {
			return createWithEditor(p, cwd)
		}

		if err := config.Validate(p); err != nil {
			return err
		}

		if err := config.SaveProfile(p, cwd, createGlobal); err != nil {
			return err
		}

		scope := "project"
		if createGlobal {
			scope = "global"
		}
		fmt.Printf("Created %s profile: %s\n", scope, name)
		return nil
	},
}

func init() {
	profilesCreateCmd.Flags().BoolVarP(&createGlobal, "global", "g", false, "Create as global profile")
	profilesCreateCmd.Flags().StringVar(&createModel, "model", "", "Model")
	profilesCreateCmd.Flags().StringVar(&createEffort, "effort", "", "Effort level")
	profilesCreateCmd.Flags().StringVar(&createPerm, "permission-mode", "", "Permission mode")
	profilesCreateCmd.Flags().StringVarP(&createDescription, "description", "d", "", "Description")
}

func createWithEditor(p *config.Profile, cwd string) error {
	// Save a template file, open in editor, then load and validate
	global := createGlobal
	if err := config.SaveProfile(p, cwd, global); err != nil {
		return err
	}

	var dir string
	if global {
		gd, err := config.GlobalProfilesDir()
		if err != nil {
			return err
		}
		dir = gd
	} else {
		dir = config.ProjectProfilesDir(cwd)
	}

	path := config.ProfilePath(dir, p.Name)

	editor := os.Getenv("EDITOR")
	if editor == "" {
		editor = "vi"
	}

	cmd := exec.Command(editor, path)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		return fmt.Errorf("editor exited with error: %w", err)
	}

	// Reload and validate
	edited, err := config.LoadProfileFromFile(path)
	if err != nil {
		return fmt.Errorf("parsing edited profile: %w", err)
	}

	if err := config.Validate(edited); err != nil {
		return fmt.Errorf("validation failed: %w", err)
	}

	scope := "project"
	if global {
		scope = "global"
	}
	fmt.Printf("Created %s profile: %s\n", scope, p.Name)
	return nil
}
