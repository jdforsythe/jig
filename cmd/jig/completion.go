package main

import (
	"os"

	"github.com/spf13/cobra"
)

var completionCmd = &cobra.Command{
	Use:   "completion [bash|zsh|fish]",
	Short: "Generate shell completion script",
	Long: `Generate shell completion scripts for jig.

To load completions:

Bash:
  $ source <(jig completion bash)

  # To load completions for each session, execute once:
  # Linux:
  $ jig completion bash > /etc/bash_completion.d/jig
  # macOS:
  $ jig completion bash > $(brew --prefix)/etc/bash_completion.d/jig

Zsh:
  $ source <(jig completion zsh)

  # To load completions for each session, execute once:
  $ jig completion zsh > "${fpath[1]}/_jig"

Fish:
  $ jig completion fish | source

  # To load completions for each session, execute once:
  $ jig completion fish > ~/.config/fish/completions/jig.fish
`,
	Args:      cobra.ExactValidArgs(1),
	ValidArgs: []string{"bash", "zsh", "fish"},
	RunE: func(cmd *cobra.Command, args []string) error {
		switch args[0] {
		case "bash":
			return rootCmd.GenBashCompletion(os.Stdout)
		case "zsh":
			return rootCmd.GenZshCompletion(os.Stdout)
		case "fish":
			return rootCmd.GenFishCompletion(os.Stdout, true)
		}
		return nil
	},
}

func init() {
	rootCmd.AddCommand(completionCmd)
}
