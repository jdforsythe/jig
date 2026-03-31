package main

import (
	"fmt"
	"os"

	"github.com/jdforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var profilesValidateCmd = &cobra.Command{
	Use:   "validate <name>",
	Short: "Validate a profile",
	Long:  "Loads and fully resolves a profile, reporting any validation or inheritance errors.",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		name := args[0]
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		if _, err := config.Resolve(name, cwd, nil); err != nil {
			return fmt.Errorf("profile %q is invalid: %w", name, err)
		}

		fmt.Printf("Profile %q is valid.\n", name)
		return nil
	},
}
