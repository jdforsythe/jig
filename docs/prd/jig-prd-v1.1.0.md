# jig: Intentional Context Utilization

## Product Requirements Document v1.1.0

**Date:** 2026-03-26
**Supersedes:** jig-prd-v1.0.0.md
**Status:** Phase 1 Complete — Phase 2 In Progress

---

## Changelog from v1.0.0

### 1. Config precedence — CLI vs template priority (needs code fix in Phase 2)

- **PRD intended:** `CLI flags > UI template > .jig.local.yaml > .jig.yaml > global config`
- **Current implementation:** templates applied at `ConfigSource::CliFlag` priority (same level as explicit CLI flags). Running `jig -t code-review --model claude-opus` may not use `claude-opus` because the template's model was applied at the same priority.
- **Fix:** add `ConfigSource::ExplicitCliFlag` ranked above `ConfigSource::TemplateSelected`; apply template config first, then individual CLI flags overwrite scalars.

### 2. `jig init` wizard was simplified to a stub

- PRD described an interactive role-detection wizard. Shipped as a minimal stub that creates an empty `.jig.yaml`.
- Full wizard moved to Phase 2.

### 3. Hook execution is TODO

- Hook approval/caching infrastructure is complete. Actual `exec[]` and `shell: true` dispatch is not yet implemented (`stage.rs:108`).
- Moved to top of Phase 2.

### 4. New TUI features added (not in v1.0.0)

- "None" option for template and persona — allow launching with no template overlay or no persona
- "Custom / Ad-hoc" entry in template list — pressing Enter opens Editor Mode inline to configure a one-off session; option to "Save as template" when done
- Both use the same Editor Mode screen as `jig template new|edit` — no separate screen

---

## Phase 1 — CLI Core (MVP) [COMPLETE]

- [x] Config schema v1 with serde, validation, per-layer constraint checks
- [x] Config resolution: global < project < local < CLI with merge semantics
- [x] `extends` schema field defined (DFS resolution to be completed in Phase 2)
- [x] All merge semantics: union tools/skills/hooks, last-wins model/env, persona inheritance
- [x] Env var `${VAR}` schema support (runtime expansion: Phase 2)
- [x] System prompt composition (persona rules + context fragments, ordered)
- [x] Token estimation with configurable budget warnings (`text.len() / 4`, labeled `~`)
- [x] Skill symlinking via `--add-dir` to staged temp dir (path-jailed)
- [x] MCP via `~/.claude.json` (atomic write, fd-lock on `.jig.lock`, session-unique backup, refcount)
- [x] MCP conflict namespacing (8-hex session suffix)
- [x] Permissions via `--allowedTools` / `--disallowedTools` CLI flags
- [x] Fork+wait with signal forwarding (no `setpgid` — inherits parent process group)
- [x] `SessionGuard` with Category A (always) / Category B (clean exit) cleanup; panic hook for Category A
- [x] Hook trust tiers + approval JSONL cache infrastructure (`security/approval.rs`)
- [x] `jig -t T [-p P]`, `jig --go`, `jig --dry-run`
- [x] `jig config show` with `resolution_trace` provenance map (JSON via `--json`)
- [x] `jig init` (minimal stub — creates `.jig.yaml` with `schema: 1`)
- [x] `jig template list|show`
- [x] `jig persona list|show`
- [x] `jig doctor` (minimal: binary check, `~/.claude.json` check, history count)
- [x] 9 built-in templates, 10 built-in personas (embedded via `include_str!`)
- [x] Feature-gated TUI (`default = ["tui"]`; `--no-default-features` for headless)
- [x] TUI: two-pane layout, template/persona selection, preview pane, responsive modes (100/80/60 col breakpoints)
- [x] Persona name-matching merges built-in rules (regression tested)
- [x] MCP direct map navigation (not JSON Pointer) for cwd key lookup (regression tested)

**Not completed — moved to Phase 2:**

- [ ] Config precedence fix: explicit CLI flags must rank above template defaults
- [ ] Hook execution (`exec[]` and `shell: true` dispatch at runtime)
- [ ] `${VAR}` / `${VAR:-default}` expansion at MCP assembly time
- [ ] `from_source` skill resolution (named sources → actual paths)
- [ ] Plugin processing (`--plugin-dir`, `installed_plugins.json` lookup)
- [ ] `extends` array DFS resolution + cycle detection (schema validates, resolver doesn't walk yet)
- [ ] `persona.extends` enforcement (only in `.jig.local.yaml`)
- [ ] MCP first-run approval (reuse hook approval pattern)
- [ ] `jig config set/add/remove` (dotted path, `--scope`)
- [ ] `jig --last [-p P]`, `jig --session <UUID>`, `jig --resume`
- [ ] `jig history [--json] [--limit N]`
- [ ] `--dry-run --json` (currently outputs text, not JSON)
- [ ] `jig init` interactive wizard (role-first, project detection, template suggestion)
- [ ] `jig import [--dry-run]` (reverse-engineer existing claude config)
- [ ] `jig diff <config>` (compare two resolved configs)
- [ ] Schema migration (v1→v2 chained with timestamped backup)
- [ ] `jig doctor --audit` (full config validation, security checks)
- [ ] Global config ownership check (0600/0640)
- [ ] Credential masking in history and dry-run output
- [ ] Worktree detection + concurrency warnings
- [ ] CI: GitHub Releases (macOS/Linux x86/arm), headless binary size gate
- [ ] Homebrew tap + curl installer + `cargo binstall` support
- [ ] Project `.jig.lock` + global `~/.config/jig/jig.lock` lock files

---

## Phase 2 — TUI + Hooks + Core Completion [IN PROGRESS]

### P0 — Critical (complete what Phase 1 started)

- [ ] **Config precedence fix** — add `ConfigSource::ExplicitCliFlag` ranked above `ConfigSource::TemplateSelected` in `config/resolve.rs`. Apply template config first in `apply_cli_overrides()`, then apply individual CLI flag scalars afterward. Additive fields (tools, skills) remain union regardless. Regression test: `jig -t code-review --model claude-opus` uses `claude-opus`.

- [ ] **Hook execution** — in `stage.rs:108`, implement the `TODO`. Dispatch `HookEntry::Exec { exec }` via `Command::new(&exec[0]).args(&exec[1..])`. Dispatch `HookEntry::Shell { command, shell: true }` via `sh -c`. `command` without `shell: true` → error. Run approval check before execution. `pre_launch` hooks run before fork; `post_exit` hooks run in `SessionGuard::drop()`. `--dry-run` already prints hooks, no change needed.

- [ ] **Env var expansion in MCP** — at assembly time in `write_atomic()`, expand `${VAR}` and `${VAR:-default}` in `McpServer` field values. Error if var is unset with no default. Use pre-expansion strings in approval cache hashes.

### P1 — TUI Improvements

- [ ] **"None" option for template and persona** — add `None (no template)` at top of template list and `None (no persona)` at top of persona list. Launching with no template skips template config overlay. Launching with no persona omits `--append-system-prompt-file` entirely.

- [ ] **"Custom / Ad-hoc" entry in template list** — add `[Custom / ad-hoc]` entry below None. Pressing Enter on it opens Editor Mode inline instead of launching. Editor Mode fields: allowed/disallowed tools, persona, MCP servers, skills, hooks, model, context fragments. Actions: `[Launch]` (one-off, no save) and `[Save as template]`. **This is the same Editor Mode screen used by `jig template new|edit`** — no separate screen should be created.

- [ ] **Editor Mode** — section-based TUI editing (skills, plugins, MCP, permissions, persona, context, hooks, flags). Undo stack (Ctrl-Z). Scope selection (global/project) when saving. Live preview of composed output. Accessible via `e` on selected template/persona, the Custom ad-hoc entry, and `jig template new`. Vim keybindings + which-key popup.

- [ ] **`jig init` interactive wizard** — replace the stub. Ask: what kind of project? (detect language/framework from file extensions). Suggest matching built-in template. Ask: which persona? Scaffold `.jig.yaml` with template + persona + commented examples for MCP/skills/hooks. Mention `.jig.local.yaml` for personal overrides. Total: < 30s for typical project.

### P2 — Session Management

- [ ] **`jig history [--json] [--limit N]`** — display session history from `history.jsonl`. Records: template, persona, directory, duration, exit code. JSON output with `--json`.
- [ ] **`jig --last [-p P]`** — relaunch last session, optionally override persona.
- [ ] **`jig --resume` / `jig --session <UUID>`** — re-stage config and resume prior session by UUID.
- [ ] **Session history view in TUI** — `h` key opens session history, `L` relaunches last.
- [ ] **`--dry-run --json` fix** — output valid JSON: `{ command, args, system_prompt, token_estimate, mcp_servers, hooks_to_run }`.

### P3 — Config Management CLI

- [ ] **`jig config set/add/remove`** — dotted path notation (`jig config set persona.name strict-security`), `--scope global|project|local`, atomic YAML write.
- [ ] **`jig import [--dry-run]`** — reverse-engineer `~/.claude.json` project config into `.jig.yaml`, detect credentials, suggest `.jig.local.yaml` split, prompt hook approval.
- [ ] **`jig diff <config>`** — compare two resolved configs as unified diff or structured JSON.

### P4 — Security + Infrastructure

- [ ] `from_source` skill resolution (named sources → git URLs or local paths)
- [ ] Plugin processing (`--plugin-dir`, `installed_plugins.json` lookup, install prompt)
- [ ] `extends` array DFS resolution + cycle detection in config layer walking
- [ ] `persona.extends` enforcement (only `.jig.local.yaml`)
- [ ] Global config ownership check (0600/0640 enforcement)
- [ ] Credential masking in history and dry-run output
- [ ] Worktree detection + concurrency warnings
- [ ] Schema migration (v1→v2 with confirmation + timestamped backup)
- [ ] `jig doctor --audit` (full validation pass)
- [ ] MCP first-run approval (reuse hook approval pattern)
- [ ] CI/CD: GitHub Releases binaries (macOS/Linux x86/arm64), headless size gate (< 5 MB)
- [ ] Homebrew tap + curl installer + `cargo binstall` support
- [ ] Lock files: project `.jig.lock` + global `~/.config/jig/jig.lock`

---

## Phase 3 / Phase 4 / Phase 5

### Phase 3 — Skill Registry + Sync

- [ ] `jig sync` from git sources (shell out to git CLI, not git2; `--no-recurse-submodules`)
- [ ] Skill indexing from SKILL.md frontmatter
- [ ] Full-copy override layer with runtime diff + staleness warnings
- [ ] `jig skill search|info|override|diff|reset`
- [ ] SHA256 integrity verification
- [ ] Lock file update on sync (`jig sync` updates, `--frozen` refuses, `--check` reports)

### Phase 4 — Team + Bootstrapping

- [ ] Dependency resolution + install prompts for missing skills/plugins
- [ ] Plugin marketplace integration (`claude plugin install`)
- [ ] `jig template export|import` (share via URL/gist)
- [ ] Shell completions (bash/zsh/fish; < 100ms, returns empty on error, CWD-aware)
- [ ] Context versioning in history (note fragment changes since last session)
- [ ] Structured audit events for SOC2 (config hash, MCP servers in history)

### Phase 5 — Ecosystem

- [ ] `jig serve --mcp` — 14-tool MCP server over stdio transport
- [ ] jig-config-helper plugin (craft `.jig.yaml` inside Claude)
- [ ] persona-crafter plugin (design custom personas interactively)
- [ ] Dynamic context injection (git branch, recent commits into fragments)
- [ ] CI/CD headless mode for `claude -p` pipelines
- [ ] `jig doctor --audit` full security review
- [ ] JSON Schema published to SchemaStore
- [ ] jig.dev landing page + 15-second GIF demo
- [ ] `jig template share` (gist URL generation)

---

## Testing Requirements

> **Every phase increment must pass `cargo test --workspace` with zero failures before being considered done.**
>
> Test commands:
> ```bash
> cargo test -p jig-core          # after jig-core changes
> cargo test --workspace          # before every commit
> ```
>
> Requirements:
> - Each bug fix must include a regression test that would have caught the bug
> - Each new feature must include happy path + key edge case tests
> - No test relies on mocking where a real implementation is practical
> - jig-cli and jig-tui must not remain at 0 tests
