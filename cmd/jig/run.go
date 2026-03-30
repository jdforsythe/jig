package main

import (
	"fmt"
	"os"

	"github.com/jforsythe/jig/internal/claude"
	"github.com/jforsythe/jig/internal/config"
	"github.com/spf13/cobra"
)

var (
	runDryRun bool
	runPick   bool
	runModel  string
	runEffort string
	runPerm   string
)

var runCmd = &cobra.Command{
	Use:   "run [profile] [-- flags...]",
	Short: "Launch Claude Code with a profile",
	Long:  "Launches a Claude Code session configured with the specified profile. Everything after -- is passed directly to claude.",
	Args:  cobra.MaximumNArgs(1),
	RunE: func(cmd *cobra.Command, args []string) error {
		if runPick {
			return runPickMode()
		}

		if len(args) == 0 {
			return fmt.Errorf("profile name required (or use --pick for ad-hoc selection)")
		}

		return runProfile(args[0], cmd.ArgsLenAtDash())
	},
}

func init() {
	runCmd.Flags().BoolVar(&runDryRun, "dry-run", false, "Show what would be generated without launching")
	runCmd.Flags().BoolVar(&runPick, "pick", false, "Ad-hoc picker mode")
	runCmd.Flags().StringVar(&runModel, "model", "", "Override model")
	runCmd.Flags().StringVar(&runEffort, "effort", "", "Override effort level")
	runCmd.Flags().StringVar(&runPerm, "permission-mode", "", "Override permission mode")
}

func runProfile(name string, dashAt int) error {
	cwd, err := os.Getwd()
	if err != nil {
		return err
	}

	// Build overrides from flags
	overrides := &config.Profile{
		Model:          runModel,
		Effort:         runEffort,
		PermissionMode: runPerm,
	}

	// Resolve profile
	p, err := config.Resolve(name, cwd, overrides)
	if err != nil {
		// Try to suggest a similar profile name
		profiles, listErr := config.ListProfiles(cwd)
		if listErr == nil {
			if suggestion, ok := config.SuggestProfile(name, profiles); ok {
				return fmt.Errorf("profile %q not found. Did you mean %q?", name, suggestion)
			}
		}
		return fmt.Errorf("resolving profile %q: %w", name, err)
	}

	// Get passthrough flags
	var passthrough []string
	if dashAt >= 0 {
		passthrough = os.Args[dashAt+1:]
	}

	// Detect claude
	detected, err := claude.Detect()
	if err != nil {
		return err
	}

	// Build MCP index
	mcpIndex, err := claude.BuildMCPIndex(cwd)
	if err != nil {
		return fmt.Errorf("building MCP index: %w", err)
	}

	// Generate plugin dir
	pluginDir, err := claude.GeneratePluginDir(p, mcpIndex)
	if err != nil {
		return fmt.Errorf("generating plugin dir: %w", err)
	}

	// Build CLI args
	args := claude.BuildCLIArgs(p, pluginDir, passthrough)

	if runDryRun {
		fmt.Println("Profile:", p.Name)
		fmt.Println("Claude:", detected.Path, "("+detected.Version+")")
		fmt.Println("Plugin dir:", pluginDir)
		fmt.Println("Command:", detected.Path)
		for _, a := range args {
			fmt.Println("  ", a)
		}
		return nil
	}

	// Setup cleanup
	cleanup := claude.NewCleanup()
	cleanup.Register(pluginDir)
	defer cleanup.Run()

	// Launch
	exitCode, err := claude.Launch(detected.Path, args)
	if err != nil {
		return err
	}
	if exitCode != 0 {
		os.Exit(exitCode)
	}
	return nil
}

func runPickMode() error {
	fmt.Println("Ad-hoc picker not yet implemented (requires TUI)")
	return nil
}
