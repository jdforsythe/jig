package main

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/jdforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var deleteForce bool

var profilesDeleteCmd = &cobra.Command{
	Use:   "delete <name>",
	Short: "Delete a profile",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		name := args[0]
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		// Find the profile first
		p, err := config.LoadProfile(name, cwd)
		if err != nil {
			return fmt.Errorf("profile %q not found: %w", name, err)
		}

		if !deleteForce {
			fmt.Printf("Delete profile %q from %s? [y/N] ", name, p.FilePath())
			reader := bufio.NewReader(os.Stdin)
			answer, _ := reader.ReadString('\n')
			answer = strings.TrimSpace(strings.ToLower(answer))
			if answer != "y" && answer != "yes" {
				fmt.Println("Cancelled.")
				return nil
			}
		}

		if err := os.Remove(p.FilePath()); err != nil {
			return fmt.Errorf("deleting profile: %w", err)
		}

		fmt.Printf("Deleted profile: %s\n", name)
		return nil
	},
}

func init() {
	profilesDeleteCmd.Flags().BoolVarP(&deleteForce, "force", "f", false, "Skip confirmation")
}
