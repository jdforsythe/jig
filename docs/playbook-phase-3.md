# jig Phase 2 + Phase 3 Testing Playbook

A step-by-step guide for manually verifying all features implemented across Phase 2 and Phase 3.

---

## Prerequisites

```bash
# Build the full binary (with TUI)
cargo build --workspace

# Alias for convenience
alias jig="./target/debug/jig"

# Verify binary exists and responds
jig --version
```

---

## 1. Core Session Launch (Phase 1 baseline)

### 1.1 Template selection via TUI

```bash
jig
```

**Expected:** TUI opens. Left pane shows template list starting with `None (no template)`, then `[Custom / ad-hoc]`, then built-in templates. Right pane shows live preview. Use `j`/`k` to navigate, `Enter` to select.

### 1.2 Template selection via CLI

```bash
jig -t code-review
```

**Expected:** Launches Claude with the `code-review` template applied.

### 1.3 Headless binary (no TUI)

```bash
cargo build --no-default-features -p jig-cli
./target/debug/jig -t code-review --dry-run --json
```

**Expected:** JSON output with `command`, `args`, `system_prompt`, `token_estimate`, `mcp_servers`, `hooks_to_run`. No panic.

### 1.4 Headless binary size gate

```bash
cargo build --profile release-headless --no-default-features -p jig-cli
ls -lh target/release-headless/jig
```

**Expected:** File size under 5 MB.

---

## 2. Phase 2 Features

### 2.1 "None" options in TUI

```bash
jig
```

**Expected:** First template entry is `None (no template)`. First persona entry is `None (no persona)`. Selecting None for template skips template config overlay. Selecting None for persona omits `--append-system-prompt-file`.

Verify via dry-run: `jig --dry-run --json` with no template selected should produce a minimal command without `--append-system-prompt-file`.

### 2.2 Config precedence

```bash
# Template sets model to opus, CLI overrides to sonnet
jig -t code-review --model claude-sonnet-4-5 --dry-run --json | jq .args
```

**Expected:** `args` array contains `--model claude-sonnet-4-5` (CLI flag wins over template config).

### 2.3 Env var expansion in MCP

Create `.jig.yaml`:
```yaml
mcp_servers:
  my-tool:
    command: npx
    args: ["-y", "@my/tool"]
    env:
      API_KEY: "${MY_API_KEY}"
      ENDPOINT: "${BASE_URL:-https://default.example.com}"
```

```bash
export MY_API_KEY=secret123
jig --dry-run --json | jq '.mcp_servers'
```

**Expected:** `API_KEY` is `secret123`, `ENDPOINT` is `https://default.example.com` (default applied). In dry-run output, values are masked as `***`.

### 2.4 Hook execution

Create `.jig.yaml`:
```yaml
pre_launch_hooks:
  - exec: [echo, "pre-launch hook fired"]
post_exit_hooks:
  - command: "echo post-exit hook fired"
    shell: true
```

```bash
jig --dry-run --json | jq .hooks_to_run
```

**Expected:** Hooks listed in output. When launched for real, `pre-launch hook fired` prints before Claude starts; `post-exit hook fired` prints after Claude exits.

### 2.5 Session history

```bash
# After running at least one jig session:
jig history
jig history --limit 5
jig history --verbose
jig history --json
```

**Expected:** History entries with session ID, template, persona, CWD, duration, exit code. `--verbose` adds persona and exit code columns. `--json` outputs JSON array.

### 2.6 Session relaunching

```bash
jig --last          # relaunch most recent session
jig --last -p strict-security   # relaunch with different persona
jig --session <UUID>   # relaunch specific session by ID
jig --resume        # relaunch and pass --resume to Claude
```

**Expected:** Each reuses the config from the original session (template, model, MCP, skills). `--last -p P` overrides the persona.

### 2.7 TUI history overlay

```bash
jig
# Press h
```

**Expected:** History overlay opens showing last 20 sessions (date, template, persona, cwd). Press `Esc` to close. Press `L` from main screen to relaunch the last session.

### 2.8 `jig config` commands

```bash
# Create a test config
cat > .jig.yaml << 'EOF'
model: claude-sonnet-4-5
EOF

jig config show
jig config show --json

jig config set model claude-opus-4-5 --scope project
jig config show | grep model

jig config add allowed_tools Read --scope project
jig config remove allowed_tools Read --scope project
```

**Expected:** `show` displays resolved config with provenance. `set` modifies the YAML file at the target scope. `add`/`remove` modify list fields.

### 2.9 `jig import`

```bash
jig import --dry-run
jig import
```

**Expected:** Dry-run outputs what would be written to `.jig.yaml` with credentials masked. Real import writes `.jig.yaml` with MCP servers from `~/.claude.json` for the current directory.

### 2.10 `jig diff`

```bash
cat > /tmp/test-config.yaml << 'EOF'
model: claude-opus-4-5
EOF
jig diff /tmp/test-config.yaml
```

**Expected:** Unified diff showing differences between current resolved config and the target file.

### 2.11 `extends` resolution

Create `base.yaml`:
```yaml
model: claude-sonnet-4-5
allowed_tools: [Read, Write]
```

Create `.jig.yaml`:
```yaml
extends: [./base.yaml]
allowed_tools: [Edit]
```

```bash
jig config show | grep -A5 allowed_tools
```

**Expected:** `allowed_tools` is the union `[Read, Write, Edit]`.

### 2.12 `jig doctor`

```bash
jig doctor
jig doctor --audit
```

**Expected:** `doctor` checks for `claude` binary, `~/.claude.json`, history count, worktree detection. `--audit` adds file permission checks (warns if global config is not 0600/0640).

---

## 3. Phase 3 P0 — TUI Editor Mode

### 3.1 Custom / Ad-hoc entry

```bash
jig
```

**Expected:** Template list shows `[Custom / ad-hoc]` at index 1 (below `None (no template)`). Navigate to it with `j`, press `Enter`.

**Expected:** Editor Mode TUI opens (not a launch). No template is loaded — starts with empty `EditorDraft`.

### 3.2 Editor Mode navigation

In Editor Mode:

| Key | Expected behavior |
|-----|------------------|
| `j`/`↓` | Move cursor down within current section |
| `k`/`↑` | Move cursor up within current section |
| `J`/Tab | Next section |
| `K`/Shift-Tab | Previous section |
| `gg` | Jump to first section (Skills) |
| `G` | Jump to last section (Passthrough Flags) |
| `?` | Which-key popup showing all bindings |
| `Esc` | Close which-key; or prompt to exit if dirty |

### 3.3 Editing a list section (Skills)

In Editor Mode on the Skills section:

| Key | Expected behavior |
|-----|------------------|
| `a` | Append item — enter insert mode, type a path, Enter to confirm |
| `i` | Edit selected item — enter edit mode on existing item |
| `d` | Delete selected item |

### 3.4 Editing a single-line section (Model)

Navigate to Model section:
- Press `i` or `Enter` — cursor appears, type a model name
- Press `Enter` or `Esc` to confirm/cancel

### 3.5 Undo

Make some changes in Editor Mode (e.g., add a tool to Allowed Tools), then press `Ctrl-Z`.

**Expected:** Last change is reverted. Undo up to 50 times.

### 3.6 Live preview

In Editor Mode, add an allowed tool or context fragment.

**Expected:** Right pane updates within ~100ms showing the updated preview (system prompt text, token estimate).

### 3.7 Save with scope selector

Press `Ctrl-S` or type `:w` (colon then w):

**Expected:** Save scope popup appears with three options: Global (`~/.config/jig/templates/`), Project (`.jig.yaml`), Local (`.jig.local.yaml`). Navigate with `j`/`k`, confirm with `Enter`.

After saving with a name: the template file is written at the selected scope path.

### 3.8 Launch from Editor Mode (Custom / Ad-hoc)

When in Editor Mode accessed via `[Custom / ad-hoc]`, navigate to the `[Launch]` action.

**Expected:** Session launches with the current draft config, no save prompt, no template file written.

### 3.9 Edit existing template

In main TUI, navigate to an existing template (not `None` or `[Custom / ad-hoc]`), press `e`.

**Expected:** Editor Mode opens with that template's config pre-loaded into the draft.

### 3.10 `jig template new` and `jig template edit`

```bash
jig template new
jig template edit code-review
```

**Expected:** Editor Mode TUI opens. `new` starts with empty draft. `edit` loads the `code-review` template.

---

## 4. Phase 3 P1 — Infrastructure

### 4.1 Schema migration

```bash
jig doctor --migrate
```

**Expected:** Scans global config, `.jig.yaml`, `.jig.local.yaml` for schema versions. If any are at version 1 (current), shows "no migration needed" (since v1 is current). Infrastructure is verified to work.

To test migration logic:
```bash
# Manually set schema to 0 in .jig.yaml (force outdated)
# (This requires direct YAML edit since there's no older version)
jig doctor --migrate
```

**Expected:** Detects outdated version, shows migration description, prompts for confirmation (y/n). On yes: creates `<file>.bak.<timestamp>`, writes migrated file. On no: leaves file unchanged.

### 4.2 Global session lock

While a `jig` session is running, in another terminal:

```bash
cat ~/.config/jig/jig.lock
```

**Expected:** JSONL file with one line per active session containing `{ "pid": <N>, "session_id": "<UUID>", "started_at": "<ISO8601>", "cwd": "<path>" }`.

After the session ends, the entry is removed.

```bash
# Check there are no stale entries
cat ~/.config/jig/jig.lock
```

**Expected:** File is empty or absent after all sessions end.

### 4.3 CI workflow files

```bash
ls .github/workflows/
cat .github/workflows/ci.yml
cat .github/workflows/release.yml
```

**Expected:** `ci.yml` exists with jobs for test, clippy, size-gate. `release.yml` exists with matrix build and GitHub Releases upload on tag push.

### 4.4 Install script

```bash
bash install.sh --help 2>&1 || head -20 install.sh
```

**Expected:** Script has platform detection (darwin-arm64, darwin-x86_64, linux-x86_64, linux-aarch64), downloads from GitHub Releases, installs to `~/.local/bin`.

Test dry run by setting `JIG_VERSION=v1.0.0` and pointing to a test directory:
```bash
INSTALL_DIR=/tmp/jig-test-install JIG_VERSION=v1.0.0 bash install.sh
```

### 4.5 cargo-binstall metadata

```bash
grep -A20 'package.metadata.binstall' crates/jig-cli/Cargo.toml
```

**Expected:** `[package.metadata.binstall]` section with `pkg-url` template and per-target overrides for all 4 platforms.

---

## 5. Phase 3 P2 — Skill Registry + Sync

### 5.1 Source config in YAML

Create `.jig.yaml`:
```yaml
sources:
  my-skills:
    url: https://github.com/example/skills-repo
    path: skills
    rev: main
```

```bash
jig config show --json | jq .sources
```

**Expected:** Sources field present in resolved config output.

### 5.2 `jig sync`

```bash
# Requires git and network access; skip in offline environments
export JIG_RUN_GIT_TESTS=1
jig sync
```

**Expected:** Clones or updates each configured source. Prints per-source status (Cloned/Updated/AlreadyUpToDate). Updates `~/.config/jig/skills.lock`.

```bash
cat ~/.config/jig/skills.lock
```

**Expected:** TOML file with `[sources.<name>]` sections containing `url`, `fetched_at`, `sha`, `rev`, and per-skill hashes.

### 5.3 `jig sync --check`

```bash
jig sync --check
```

**Expected:** Reports staleness (whether local is behind remote) without pulling. Does not modify any files.

### 5.4 `jig sync --frozen`

```bash
jig sync --frozen
```

**Expected:** Fails with error if any source is behind remote (CI mode — refuses to update). Passes if all sources are up to date.

### 5.5 Skill listing

After `jig sync` populates `~/.config/jig/skills/`:

```bash
jig skill list
jig skill list --source my-skills
```

**Expected:** Lists all indexed skills with name, source, description.

### 5.6 Skill search

```bash
jig skill search "typescript"
jig skill search "test" --json
```

**Expected:** Returns skills where name, description, or tags contain the query (case-insensitive). `--json` outputs JSON array.

### 5.7 Skill info

```bash
jig skill info my-skills some-skill
```

**Expected:** Shows metadata (name, description, tags, version), lock info (SHA, fetched_at), integrity status (verified/tampered).

### 5.8 Skill override

```bash
jig skill override my-skills some-skill
```

**Expected:** Copies `~/.config/jig/skills/my-skills/some-skill.md` to `~/.config/jig/skills-override/my-skills/some-skill.md`. Subsequent assembly uses the override.

### 5.9 Skill diff

```bash
# After creating an override and modifying it:
jig skill diff my-skills some-skill
```

**Expected:** Unified diff showing override vs upstream. Empty diff if no changes.

### 5.10 Skill reset

```bash
jig skill reset my-skills some-skill
# Without -y flag, prompts for confirmation
jig skill reset my-skills some-skill -y
```

**Expected:** `-y` skips confirmation. Removes override file. `jig skill diff` shows empty diff afterward.

### 5.11 Skill integrity warning

Manually tamper with a synced skill:
```bash
echo "tampered" >> ~/.config/jig/skills/my-skills/some-skill.md
jig skill info my-skills some-skill
```

**Expected:** Warning shown that SHA-256 does not match lockfile value.

### 5.12 Source skill resolution in assembly

With skills from source configured in `.jig.yaml`:
```yaml
skills:
  from_source:
    my-skills: [some-skill]
```

```bash
jig --dry-run --json | jq .args
```

**Expected:** `--add-dir` argument in the args with the path to the staged skills temp directory containing a symlink to `some-skill.md`.

If the source has not been synced:
```bash
jig --dry-run
```

**Expected:** Warning printed (non-fatal) about skill not synced, but assembly continues without that skill.

### 5.13 Skill frontmatter parsing

Create a test skill file:
```bash
mkdir -p ~/.config/jig/skills/test-source
cat > ~/.config/jig/skills/test-source/my-skill.md << 'EOF'
---
name: My Test Skill
description: A skill for testing frontmatter parsing
tags: [test, example]
version: "1.0.0"
---
# My Test Skill

This is the skill content.
EOF

jig skill list --source test-source
```

**Expected:** Skill listed with name, description from frontmatter.

---

## 6. End-to-End Scenarios

### 6.1 New project setup with skills

```bash
mkdir /tmp/jig-e2e-test && cd /tmp/jig-e2e-test
git init

# Initialize config
jig init

# Open TUI and use Custom / Ad-hoc to configure a one-off session
jig
# Navigate to [Custom / ad-hoc], Enter
# Add some allowed tools, pick a model, press Ctrl-S to save as a template
# Then use that template
```

### 6.2 Migration + doctor cycle

```bash
jig doctor
jig doctor --audit
jig doctor --migrate
```

**Expected:** All three complete without errors. `--audit` verifies file permissions. `--migrate` verifies all config files are at current schema version.

### 6.3 Full skill workflow

```bash
# Configure a source
jig config set sources.example.url https://github.com/example/skills --scope project
jig config set sources.example.rev main --scope project

# Sync
jig sync

# Search and use a skill
jig skill search keyword
jig skill override example some-skill
# edit ~/.config/jig/skills-override/example/some-skill.md
jig skill diff example some-skill
jig skill reset example some-skill -y
```

### 6.4 CI/headless mode

```bash
# Simulate CI: headless binary, frozen sync, dry-run
cargo build --no-default-features -p jig-cli --release
./target/release/jig sync --frozen
./target/release/jig -t code-review --dry-run --json
```

**Expected:** `sync --frozen` passes (or fails with clear error if behind). Dry-run outputs structured JSON.

---

## 7. Regression Tests

These verify known bugs from earlier phases don't regress.

### 7.1 MCP JSON Pointer bug (Phase 1)

Verify that MCP cwd lookup uses direct map navigation:
```bash
cd /private/tmp/some/path
jig --dry-run --json | jq .mcp_servers
```

**Expected:** No panic from JSON Pointer path separator issue. MCP servers from `~/.claude.json` for this CWD are correctly loaded.

### 7.2 Process group hang (Phase 1)

```bash
jig -t code-review
# Ctrl-C to interrupt
```

**Expected:** Ctrl-C is forwarded to Claude, jig exits cleanly. No hang.

### 7.3 Session cleanup (Phase 1+3)

After any jig session:
```bash
# Global lock entry removed
cat ~/.config/jig/jig.lock  # should be empty or absent

# Project lock removed
ls .jig.lock 2>/dev/null && echo "LEAK" || echo "cleaned up"

# MCP entries cleaned from ~/.claude.json
jq '.projects."'"$(pwd)"'".mcpServers | keys' ~/.claude.json 2>/dev/null
```

**Expected:** No stale lock entries. No jig-written MCP entries (they all end with `-XXXXXXXX` suffix which was removed on exit).

### 7.4 MCP optional field null (Phase 1)

```bash
jig --dry-run --json
cat ~/.claude.json | python3 -c "import sys,json; d=json.load(sys.stdin); print('ok')"
```

**Expected:** `~/.claude.json` is valid JSON with no `null` values in MCP server entries.

---

## 8. Automated Test Suite

```bash
# Run all tests
cargo test --workspace

# Run with git integration tests (requires network)
JIG_RUN_GIT_TESTS=1 cargo test --workspace

# Expected output at Phase 3 completion:
# jig-cli: 18 tests, 0 failed
# jig-core: 143 tests, 0 failed, 1 ignored (git integration)
# jig-tui: 36 tests, 0 failed
# Total: 197 tests, 0 failed
```
