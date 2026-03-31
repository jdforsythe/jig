# jig

![CI](https://github.com/jforsythe/jig/actions/workflows/ci.yml/badge.svg)
[![Go Report Card](https://goreportcard.com/badge/github.com/jforsythe/jig)](https://goreportcard.com/report/github.com/jforsythe/jig)
[![Go Version](https://img.shields.io/badge/go-%3E%3D1.23-blue)](https://go.dev/dl/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

![demo](./demo.gif)

> Manage and launch Claude Code sessions with profiles — define your tools, MCP servers, skills, agents, and permissions once, then launch in one keystroke.

---

## Requirements

| Requirement | Version / Notes |
|-------------|-----------------|
| Go          | ≥ 1.23 |
| Claude Code | latest (`claude` binary in `$PATH`) |
| OS          | macOS, Linux |
| Terminal    | 256-color or true-color recommended; `NO_COLOR` supported |

---

## Installation

```sh
# go install
go install github.com/jdforsythe/jig@latest

# homebrew
brew tap jdforsythe/jig && brew install jig

# binary — https://github.com/jdforsythe/jig/releases/latest
```

---

## Quick Start

```sh
# Launch the TUI profile manager
jig

# Run a profile directly
jig run my-profile

# Ad-hoc session — pick skills, agents, MCP servers on the fly
jig run --pick

# Initialize project-local profiles in the current directory
jig init
```

---

## How It Works

Jig resolves a profile's configuration — including inherited settings, selected skills, agents, and MCP servers — then generates a temporary Claude Code plugin directory and launches `claude` with the appropriate flags. Cleanup happens automatically on exit.

```
jig run my-profile
  └─ load & resolve profile
       └─ generate plugin dir  (symlinks skills, agents, commands)
            └─ launch: claude --plugin-dir /tmp/jig-xyz [...flags]
                 └─ cleanup on exit
```

---

## Commands

| Command | Description |
|---------|-------------|
| `jig` | Open the TUI profile manager |
| `jig run [profile]` | Launch a Claude Code session with the given profile |
| `jig run --pick` | Ad-hoc picker — select skills/agents/MCP servers without a saved profile |
| `jig run --dry-run` | Show the generated config and command without launching |
| `jig profiles list` | List all available profiles |
| `jig profiles create <name>` | Create a new profile (opens `$EDITOR` if no flags given) |
| `jig profiles edit <name>` | Open a profile in `$EDITOR` |
| `jig profiles show <name>` | Display the fully resolved profile YAML |
| `jig profiles export <name>` | Export a profile as CLI args, plugin dir path, or JSON |
| `jig profiles validate <name>` | Validate a profile and report any errors |
| `jig profiles delete <name>` | Delete a profile |
| `jig init` | Create `.jig/profiles/` in the current directory |
| `jig doctor` | Check Claude installation, config dirs, profiles, and MCP servers |
| `jig completion [bash\|zsh\|fish]` | Generate shell completion scripts |

### `jig run` Flags

| Flag | Type | Description |
|------|------|-------------|
| `--dry-run` | bool | Print config and CLI args without launching |
| `--pick` | bool | Open ad-hoc picker (no profile name required) |
| `--model` | string | Override the profile's model |
| `--effort` | string | Override the profile's effort level |
| `--permission-mode` | string | Override the profile's permission mode |

Pass additional flags directly to `claude` after `--`:

```sh
jig run my-profile -- --no-stream
```

---

## Keybindings

### Home (Profile List)

| Key | Action |
|-----|--------|
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `enter` | Launch selected profile |
| `n` | Create new profile |
| `e` | Edit selected profile |
| `d` | Delete profile (prompts for confirmation) |
| `y` | Confirm deletion |
| `v` | Preview profile |
| `q` / `ctrl+c` | Quit |

### Editor (Profile Editor)

| Key | Action |
|-----|--------|
| `tab` / `shift+tab` | Next / previous tab |
| `↑` / `k` | Previous field |
| `↓` / `j` | Next field |
| `enter` | Edit field / toggle value |
| `s` | Save and return to home |
| `esc` | Discard changes and return |

**While editing a field:**

| Key | Action |
|-----|--------|
| `enter` | Confirm edit |
| `esc` | Cancel edit |

**Components tab:**

| Key | Action |
|-----|--------|
| `space` | Toggle skill / agent / command |
| `/` | Enter filter mode |

**Plugins tab:**

| Key | Action |
|-----|--------|
| `enter` / `→` | Expand plugin |
| `←` / `esc` | Collapse plugin |
| `f` | Toggle full plugin enable |
| `space` | Toggle individual component |

### Preview

| Key | Action |
|-----|--------|
| `↑` / `k` | Scroll up |
| `↓` / `j` | Scroll down |
| `enter` | Launch profile |
| `esc` / `q` | Return to home |

### Picker (Ad-hoc Mode)

| Key | Action |
|-----|--------|
| `↑` / `k` | Previous item |
| `↓` / `j` | Next item |
| `space` | Toggle selection |
| `/` | Filter |
| `enter` | Launch with selected items |
| `esc` / `q` | Quit |

---

## Profile Configuration

Profiles live in `~/.jig/profiles/` (global) or `.jig/profiles/` (project-local).

```yaml
name: my-profile
description: "Full-stack session with MCP tools"
extends: base-profile           # inherit from another profile

# Claude CLI settings
model: claude-opus-4-6
effort: high
permission_mode: default

# System prompt
append_system_prompt: "Always explain your reasoning."

# Tool control
allowed_tools:
  - Bash
  - Edit
disallowed_tools:
  - WebSearch

# MCP servers
mcp_servers:
  - ref: my-mcp-server          # reference from ~/.mcp.json
  - name: custom-server
    command: npx
    args: ["-y", "@my/mcp-server"]
    env:
      API_KEY: "${MY_API_KEY}"

# Skills, agents, and commands to include
skills:
  - path: ~/.claude/plugins/my-plugin/skills/my-skill.md
agents:
  - path: ~/.claude/plugins/my-plugin/agents/my-agent.md

# Plugin integration
enabled_plugins:
  my-plugin: true

plugin_components:
  my-plugin:
    skills:
      - specific-skill
    agents:
      - specific-agent

# Extra flags passed directly to claude
extra_flags:
  - "--no-stream"
```

### Profile Search Order

1. Project-local: `.jig/profiles/<name>.yaml`
2. Global: `~/.jig/profiles/<name>.yaml`

Profiles support inheritance via `extends`. Inherited settings are merged, with the child profile taking precedence.

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `EDITOR` | `vi` | Editor used by `jig profiles create` and `jig profiles edit` |
| `NO_COLOR` | — | If set, switches to monochrome mode (bold/underline instead of colors) |

---

## Shell Completions

```sh
# bash
jig completion bash > /etc/bash_completion.d/jig

# zsh
jig completion zsh > "${fpath[1]}/_jig"

# fish
jig completion fish > ~/.config/fish/completions/jig.fish
```

---

## Contributing

```sh
git clone https://github.com/jforsythe/jig.git
cd jig
go build ./...
go test ./...
```

Bug reports and pull requests are welcome at [github.com/jforsythe/jig](https://github.com/jforsythe/jig).

---

## License

MIT — see [LICENSE](LICENSE).
