package main

import "github.com/spf13/cobra"

var profilesCmd = &cobra.Command{
	Use:   "profiles",
	Short: "Manage profiles",
	Long:  "Commands for creating, editing, listing, and deleting profiles.",
}

func init() {
	profilesCmd.AddCommand(profilesListCmd)
	profilesCmd.AddCommand(profilesCreateCmd)
	profilesCmd.AddCommand(profilesEditCmd)
	profilesCmd.AddCommand(profilesDeleteCmd)
	profilesCmd.AddCommand(profilesShowCmd)
	profilesCmd.AddCommand(profilesExportCmd)
	profilesCmd.AddCommand(profilesValidateCmd)
}
