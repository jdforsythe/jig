package main

import (
	"fmt"
	"os"

	"github.com/jforsythe/jig/internal/claude"
	"github.com/jforsythe/jig/internal/config"
	"github.com/jforsythe/jig/internal/tui"
)

func runTUI() error {
	cwd, err := os.Getwd()
	if err != nil {
		return err
	}

	profiles, err := config.ListProfiles(cwd)
	if err != nil {
		profiles = nil // non-fatal, TUI can show empty list
	}

	app := tui.New(profiles, cwd)
	result, err := app.Run()
	if err != nil {
		return fmt.Errorf("TUI error: %w", err)
	}

	if result == nil || result.ProfileName == "" {
		return nil // user quit without selecting
	}

	// Resolve and launch the selected profile
	p, err := config.Resolve(result.ProfileName, cwd, nil)
	if err != nil {
		return err
	}

	detected, err := claude.Detect()
	if err != nil {
		return err
	}

	mcpIndex, err := claude.BuildMCPIndex(cwd)
	if err != nil {
		return err
	}

	pluginDir, err := claude.GeneratePluginDir(p, mcpIndex)
	if err != nil {
		return err
	}

	args := claude.BuildCLIArgs(p, pluginDir, nil)

	cleanup := claude.NewCleanup()
	cleanup.Register(pluginDir)
	defer cleanup.Run()

	exitCode, launchErr := claude.Launch(detected.Path, args)
	if launchErr != nil {
		return launchErr
	}
	if exitCode != 0 {
		os.Exit(exitCode)
	}
	return nil
}
