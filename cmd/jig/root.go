package main

import (
	"fmt"

	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "jig",
	Short: "Claude Code session configurator",
	Long:  "Jig manages profiles for configuring Claude Code sessions. Run without arguments to launch the TUI.",
	RunE: func(cmd *cobra.Command, args []string) error {
		// Default: launch TUI
		return runTUI()
	},
	Version: version,
}

func init() {
	rootCmd.SetVersionTemplate(fmt.Sprintf("jig %s\n", version))
	rootCmd.AddCommand(runCmd)
	rootCmd.AddCommand(profilesCmd)
	rootCmd.AddCommand(initCmd)
	rootCmd.AddCommand(doctorCmd)
}
