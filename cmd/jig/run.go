package main

import (
	"fmt"
	"os"

	"github.com/jdforsythe/jig/internal/claude"
	"github.com/jdforsythe/jig/internal/config"
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
	Use:   "run [profile] [flags] [-- passthrough...]",
	Short: "Launch Claude Code with a profile",
	Long:  "Launches a Claude Code session configured with the specified profile. Everything after -- is passed directly to claude.",
	Args: func(cmd *cobra.Command, args []string) error {
		// Only count args before the -- separator
		count := len(args)
		if d := cmd.ArgsLenAtDash(); d >= 0 {
			count = d
		}
		if count > 1 {
			return fmt.Errorf("accepts at most 1 arg(s), received %d", count)
		}
		return nil
	},
	RunE: func(cmd *cobra.Command, args []string) error {
		if runPick {
			return runPickMode()
		}

		dashAt := cmd.ArgsLenAtDash()

		preCount := len(args)
		if dashAt >= 0 {
			preCount = dashAt
		}

		if preCount == 0 {
			return fmt.Errorf("profile name required (or use --pick for ad-hoc selection)")
		}

		var passthrough []string
		if dashAt >= 0 {
			passthrough = args[dashAt:]
		}

		return runProfile(args[0], passthrough)
	},
}

func init() {
	runCmd.Flags().BoolVar(&runDryRun, "dry-run", false, "Show what would be generated without launching")
	runCmd.Flags().BoolVar(&runPick, "pick", false, "Ad-hoc picker mode")
	runCmd.Flags().StringVar(&runModel, "model", "", "Override model")
	runCmd.Flags().StringVar(&runEffort, "effort", "", "Override effort level")
	runCmd.Flags().StringVar(&runPerm, "permission-mode", "", "Override permission mode")
}

func runProfile(name string, passthrough []string) error {
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
	pluginDir, settingsPath, err := claude.GeneratePluginDir(p, mcpIndex)
	if err != nil {
		return fmt.Errorf("generating plugin dir: %w", err)
	}

	// Build CLI args
	cliArgs := claude.BuildCLIArgs(p, pluginDir, settingsPath, passthrough)

	if runDryRun {
		fmt.Println("Profile:", p.Name)
		fmt.Println("Claude:", detected.Path, "("+detected.Version+")")
		fmt.Println("Plugin dir:", pluginDir)
		fmt.Println("Settings:", settingsPath)
		fmt.Println("Command:", detected.Path)
		for _, a := range cliArgs {
			fmt.Println("  ", a)
		}
		return nil
	}

	// Setup cleanup (registers signal handlers)
	cleanup := claude.NewCleanup()
	cleanup.Register(pluginDir)
	defer cleanup.Run()

	// Start claude and register its process for signal forwarding
	cmd, err := claude.Start(detected.Path, cliArgs)
	if err != nil {
		return err
	}
	cleanup.RegisterProcess(cmd.Process)

	exitCode, err := claude.Wait(cmd)
	if err != nil {
		return err
	}
	if exitCode != 0 {
		os.Exit(exitCode)
	}
	return nil
}

func runPickMode() error {
	return runPickerTUI()
}
