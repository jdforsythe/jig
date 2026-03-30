package claude

import (
	"fmt"
	"os"
	"os/exec"
)

// Launch starts the claude subprocess with full I/O passthrough.
// It blocks until the subprocess exits and returns its exit code.
func Launch(claudePath string, args []string) (int, error) {
	cmd := exec.Command(claudePath, args...)
	cmd.Stdin = os.Stdin
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr

	if err := cmd.Run(); err != nil {
		if exitErr, ok := err.(*exec.ExitError); ok {
			return exitErr.ExitCode(), nil
		}
		return 1, fmt.Errorf("launching claude: %w", err)
	}

	return 0, nil
}
