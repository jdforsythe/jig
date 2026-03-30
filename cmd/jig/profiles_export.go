package main

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/jforsythe/jig/internal/claude"
	"github.com/jforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var exportFormat string

var profilesExportCmd = &cobra.Command{
	Use:   "export <name>",
	Short: "Export profile as CLI args or plugin dir path",
	Args:  cobra.ExactArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		name := args[0]
		cwd, err := os.Getwd()
		if err != nil {
			return err
		}

		p, err := config.Resolve(name, cwd, nil)
		if err != nil {
			return err
		}

		mcpIndex, err := claude.BuildMCPIndex(cwd)
		if err != nil {
			return err
		}

		switch exportFormat {
		case "args":
			cliArgs := claude.BuildCLIArgs(p, "<PLUGIN_DIR>", "<SETTINGS_PATH>", nil)
			fmt.Println("claude \\")
			for i, a := range cliArgs {
				if i < len(cliArgs)-1 {
					fmt.Printf("  %s \\\n", a)
				} else {
					fmt.Printf("  %s\n", a)
				}
			}
		case "plugin":
			dir, _, err := claude.GeneratePluginDir(p, mcpIndex)
			if err != nil {
				return err
			}
			fmt.Println(dir)
			fmt.Fprintf(os.Stderr, "Plugin dir created at: %s\n", dir)
			fmt.Fprintf(os.Stderr, "Note: this dir is not auto-cleaned. Remove it when done.\n")
		case "json":
			data, err := json.MarshalIndent(p, "", "  ")
			if err != nil {
				return err
			}
			fmt.Println(string(data))
		default:
			return fmt.Errorf("unknown format %q (valid: args, plugin, json)", exportFormat)
		}

		return nil
	},
}

func init() {
	profilesExportCmd.Flags().StringVarP(&exportFormat, "format", "f", "args", "Export format: args, plugin, json")
}
