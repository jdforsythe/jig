package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/jforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var listJSON bool

var profilesListCmd = &cobra.Command{
	Use:   "list",
	Short: "List available profiles",
	RunE: func(cmd *cobra.Command, args []string) error {
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		profiles, err := config.ListProfiles(cwd)
		if err != nil {
			return err
		}

		if len(profiles) == 0 {
			fmt.Println("No profiles found.")
			fmt.Println("Create one with: jig profiles create <name>")
			return nil
		}

		if listJSON {
			data, err := json.MarshalIndent(profiles, "", "  ")
			if err != nil {
				return err
			}
			fmt.Println(string(data))
			return nil
		}

		for _, p := range profiles {
			source := ""
			switch p.Source() {
			case config.SourceGlobal:
				source = " (global)"
			case config.SourceProject:
				source = " (project)"
			case config.SourceShortcut:
				source = " (shortcut)"
			}

			desc := ""
			if p.Description != "" {
				desc = " - " + p.Description
			}

			fmt.Printf("  %s%s%s\n", p.Name, source, desc)
		}

		return nil
	},
}

func init() {
	profilesListCmd.Flags().BoolVar(&listJSON, "json", false, "Output as JSON")
}
