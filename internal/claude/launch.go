package claude

import (
	"fmt"
	"os"
	"os/exec"
)

// Start starts the claude subprocess and returns the cmd.
// The caller must call Wait to collect the exit code.
func Start(claudePath string, args []string) (*exec.Cmd, error) {
	cmd := exec.Command(claudePath, args...)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Start(); err != nil {
		return nil, fmt.Errorf("starting claude: %w", err)
	}
	return cmd, nil
}

// Wait waits for a claude subprocess started by Start and returns its exit code.
func Wait(cmd *exec.Cmd) (int, error) {
	if err := cmd.Wait(); err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			return exitErr.ExitCode(), nil
		}
		return 1, fmt.Errorf("waiting for claude: %w", err)
	}
	return 0, nil
}

// Launch starts the claude subprocess with full I/O passthrough.
// It blocks until the subprocess exits and returns its exit code.
func Launch(claudePath string, args []string) (int, error) {
	cmd, err := Start(claudePath, args)
	if err != nil {
		return 1, err
	}
	return Wait(cmd)
}
