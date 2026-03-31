package main

import (
	"fmt"
	"os"

	"github.com/jdforsythe/jig/internal/claude"
	"github.com/jdforsythe/jig/internal/config"
	"github.com/jdforsythe/jig/internal/scanner"
	"github.com/jdforsythe/jig/internal/tui"
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

	pluginDir, settingsPath, err := claude.GeneratePluginDir(p, mcpIndex)
	if err != nil {
		return err
	}

	args := claude.BuildCLIArgs(p, pluginDir, settingsPath, nil)

	cleanup := claude.NewCleanup()
	cleanup.Register(pluginDir)
	defer cleanup.Run()

	cmd, err := claude.Start(detected.Path, args)
	if err != nil {
		return err
	}
	cleanup.RegisterProcess(cmd.Process)

	exitCode, launchErr := claude.Wait(cmd)
	if launchErr != nil {
		return launchErr
	}
	if exitCode != 0 {
		os.Exit(exitCode)
	}
	return nil
}

func runPickerTUI() error {
	cwd, err := os.Getwd()
	if err != nil {
		return err
	}

	disc, _ := scanner.Scan(cwd) // non-fatal if scan fails

	app := tui.NewPickerApp(disc, cwd)
	result, err := app.Run()
	if err != nil {
		return fmt.Errorf("TUI error: %w", err)
	}

	if result == nil || result.Profile == nil {
		return nil // user quit without launching
	}

	p := result.Profile
	// Apply defaults if not set by picker
	if p.Effort == "" {
		p.Effort = "high"
	}
	if p.PermissionMode == "" {
		p.PermissionMode = "default"
	}

	detected, err := claude.Detect()
	if err != nil {
		return err
	}

	mcpIndex, err := claude.BuildMCPIndex(cwd)
	if err != nil {
		return fmt.Errorf("building MCP index: %w", err)
	}

	pluginDir, settingsPath, err := claude.GeneratePluginDir(p, mcpIndex)
	if err != nil {
		return fmt.Errorf("generating plugin dir: %w", err)
	}

	args := claude.BuildCLIArgs(p, pluginDir, settingsPath, nil)

	cleanup := claude.NewCleanup()
	cleanup.Register(pluginDir)
	defer cleanup.Run()

	cmd, err := claude.Start(detected.Path, args)
	if err != nil {
		return err
	}
	cleanup.RegisterProcess(cmd.Process)

	exitCode, launchErr := claude.Wait(cmd)
	if launchErr != nil {
		return launchErr
	}
	if exitCode != 0 {
		os.Exit(exitCode)
	}
	return nil
}
