package main

import (
	"fmt"
	"os"

	"github.com/jforsythe/jig/internal/config"
	"github.com/spf13/cobra"
	"gopkg.in/yaml.v3"
)

var profilesShowCmd = &cobra.Command{
	Use:   "show <name>",
	Short: "Show resolved profile details",
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

		data, err := yaml.Marshal(p)
		if err != nil {
			return err
		}

		fmt.Printf("# Profile: %s (resolved)\n", p.Name)
		fmt.Println(string(data))
		return nil
	},
}
