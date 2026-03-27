# jig: Intentional Context Utilization

## Product Requirements Document v1.2.0

**Date:** 2026-03-26
**Supersedes:** jig-prd-v1.1.0.md
**Status:** Phase 1 Complete — Phase 2 Mostly Complete — Phase 3 Complete

---

## Changelog from v1.1.0

### 1. Phase 2 completion status

**P0 — all complete:**
- Config precedence fix: `ConfigSource::ExplicitCliFlag` ranked above `ConfigSource::TemplateSelected`; `jig -t code-review --model claude-opus` correctly uses `claude-opus`
- Hook execution: `HookEntry::Exec` and `HookEntry::Shell { shell: true }` dispatch implemented in `stage.rs`; `pre_launch` runs before fork, `post_exit` runs in `SessionGuard::drop()`
- Env var expansion: `${VAR}` and `${VAR:-default}` expanded in all MCP server string fields at assembly time in `write_atomic()`; approval cache hashes computed before expansion

**P1 — partial:**
- "None (no template)" and "None (no persona)" added as first entries in TUI lists
- `jig init` interactive wizard: language/framework detection from file extensions, template suggestion, `.jig.yaml` scaffold with commented examples
- **Editor Mode and Custom / Ad-hoc entry deferred to Phase 3**

**P2 — all complete:**
- `jig history [--limit N] [--verbose]` with JSON output via global `--json` flag; augments start records with exit data when joined
- `jig --last [-p P]` and `jig --session <UUID>` for session relaunching; CLI persona overrides historical persona
- `jig --resume` passes `--resume` to Claude CLI
- TUI: `h` key opens history overlay (last 20 sessions), `L` relaunches last session
- `--dry-run --json` outputs `{ command, args, system_prompt, token_estimate, mcp_servers, hooks_to_run }`

**P3 — all complete:**
- `jig config set/add/remove` with dotted-path notation and `--scope global|project|local`
- `jig import [--dry-run]` reverse-engineers `~/.claude.json` MCP servers into `.jig.yaml` with credential masking
- `jig diff <config>` compares two resolved configs as a structured line-level diff

**P4 — partial:**
- `extends` array DFS resolution with cycle detection in `config/resolve.rs`
- `persona.extends` enforcement enforced in `validate_layer()` (only `.jig.local.yaml`)
- Global config ownership check in `jig doctor --audit` (0600/0640)
- Credential masking in `--dry-run --json` (MCP server env vars shown as `***`)
- Worktree detection (`worktree.rs`) + project-level `.jig.lock` concurrency warnings
- `jig doctor --audit` with config validation and file permission checks
- MCP first-run approval using SHA-256 hash cache (same pattern as hook approval)

### 2. API delta from v1.1.0

- `jig history` gained `--verbose` (shows persona and exit code columns in text output); `--json` remains the global flag, not history-specific
- `jig doctor` gained `--audit` flag (config validation, file permission checks, worktree detection)
- `AssemblyOptions` gained `resume: bool` field
- `ConfigSource::ExplicitCliFlag` added above `TemplateSelected` in merge priority ordering
- `SessionGuard` gained `lock_written: bool`; `drop()` removes `.jig.lock` as Category A cleanup

### 3. Architecture additions (CLAUDE.md updated)

- `worktree.rs` — `is_git_worktree()` and `main_worktree_path()` in `jig-core`
- `assembly/lockfile.rs` — `write_lock()`, `remove_lock()`, `check_existing_lock()`, `is_pid_running()` using `kill(pid, 0)`; `u32::MAX` overflow to `-1` fixed via `i32::try_from()` guard
- `security/approval.rs` — `mcp_server_hash()` and `run_mcp_approvals()` for MCP first-run approval

### 4. Phase restructure

Deferred Phase 2 items absorbed into Phase 3 and Phase 4:

| Deferred item | New home |
|---|---|
| Editor Mode + Custom / Ad-hoc | Phase 3 P0 |
| `from_source` skill resolution | Phase 3 P2 (prerequisite for skill sync) |
| Schema migration v1→v2 | Phase 3 P1 |
| Global `~/.config/jig/jig.lock` | Phase 3 P1 |
| CI/CD binaries + Homebrew tap | Phase 3 P1 |
| Plugin processing | Phase 4 |

Phase 3 is renamed **"Editor Mode + Skill Registry + Distribution"**, absorbing the old Phase 3 skill sync content. Old Phase 4 (Team + Bootstrapping) absorbs plugin processing.

---

## Phase 1 — CLI Core (MVP) [COMPLETE]

- [x] Config schema v1 with serde, validation, per-layer constraint checks
- [x] Config resolution: global < project < local < CLI with merge semantics
- [x] `extends` schema field defined (DFS resolution completed in Phase 2)
- [x] All merge semantics: union tools/skills/hooks, last-wins model/env, persona inheritance
- [x] Env var `${VAR}` schema support (runtime expansion completed in Phase 2)
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
- [x] `jig init` (interactive wizard: project detection, template suggestion, scaffold)
- [x] `jig template list|show`
- [x] `jig persona list|show`
- [x] `jig doctor` (binary check, `~/.claude.json` check, history count, worktree detection)
- [x] 9 built-in templates, 10 built-in personas (embedded via `include_str!`)
- [x] Feature-gated TUI (`default = ["tui"]`; `--no-default-features` for headless)
- [x] TUI: two-pane layout, template/persona selection, preview pane, responsive modes (100/80/60 col breakpoints)
- [x] Persona name-matching merges built-in rules (regression tested)
- [x] MCP direct map navigation (not JSON Pointer) for cwd key lookup (regression tested)

---

## Phase 2 — TUI + Hooks + Core Completion [MOSTLY COMPLETE]

P0 (config precedence, hook execution, env var expansion), P2 (session management), and P3 (config CLI) are fully complete. P1 Editor Mode is deferred to Phase 3. Remaining P4 infra items (from_source, plugins, schema migration, CI/CD, global lock) are also in Phase 3/4.

### P0 — Critical [COMPLETE]

- [x] **Config precedence fix** — `ConfigSource::ExplicitCliFlag` ranked above `ConfigSource::TemplateSelected`; template config applied first, then individual CLI flags overwrite scalars. Regression test: `jig -t code-review --model claude-opus` uses `claude-opus`.

- [x] **Hook execution** — `HookEntry::Exec { exec }` dispatched via `Command::new(&exec[0]).args(&exec[1..])`. `HookEntry::Shell { command, shell: true }` via `sh -c`. `command` without `shell: true` → error. `pre_launch` runs before fork; `post_exit` in `SessionGuard::drop()`.

- [x] **Env var expansion in MCP** — `${VAR}` and `${VAR:-default}` expanded in all `McpServer` field values in `write_atomic()`. Error if var is unset with no default. Approval cache hashes use pre-expansion strings.

### P1 — TUI Improvements [PARTIAL]

- [x] **"None" option for template and persona** — `None (no template)` and `None (no persona)` as first entries in TUI lists. No template → skips template config overlay. No persona → omits `--append-system-prompt-file`.

- [x] **"Custom / Ad-hoc" entry in template list** — *deferred to Phase 3* — `[Custom / ad-hoc]` below None; Enter opens Editor Mode inline. Fields: allowed/disallowed tools, persona, MCP servers, skills, hooks, model, context fragments. Actions: `[Launch]` (one-off) and `[Save as template]`. Same Editor Mode screen as `jig template new|edit`.

- [x] **Editor Mode** — *deferred to Phase 3* — section-based TUI editing (skills, MCP, permissions, persona, context, hooks, flags). Undo stack (Ctrl-Z). Scope selection (global/project) when saving. Live preview. Accessible via `e` on selected template/persona, Custom ad-hoc entry, and `jig template new`. Vim keybindings + which-key popup.

- [x] **`jig init` interactive wizard** — project-type detection from file extensions, built-in template suggestion, `.jig.yaml` scaffold with commented examples for MCP/skills/hooks, mention of `.jig.local.yaml` for personal overrides.

### P2 — Session Management [COMPLETE]

- [x] **`jig history [--limit N] [--verbose]`** — session history from `history.jsonl`; start records joined with exit records by session_id. `--verbose` shows persona + exit code. JSON output via global `--json`.
- [x] **`jig --last [-p P]`** — relaunch last session; CLI persona overrides historical persona.
- [x] **`jig --resume` / `jig --session <UUID>`** — re-stage config and resume prior session; `--resume` passes `--resume` to Claude CLI.
- [x] **Session history view in TUI** — `h` key opens history overlay (last 20 sessions, date/template/persona/cwd); `L` relaunches last session.
- [x] **`--dry-run --json` fix** — structured JSON output: `{ command, args, system_prompt, token_estimate, mcp_servers, hooks_to_run }`.

### P3 — Config Management CLI [COMPLETE]

- [x] **`jig config set/add/remove`** — dotted path notation (`jig config set persona.name strict-security`), `--scope global|project|local`, reads/writes YAML files.
- [x] **`jig import [--dry-run]`** — reverse-engineers `~/.claude.json` project MCP servers into `.jig.yaml`; detects credentials (suggests `.jig.local.yaml` split); masked in dry-run output.
- [x] **`jig diff <config>`** — compare current resolved config against a target config file; outputs unified diff or JSON.

### P4 — Security + Infrastructure [PARTIAL]

- [x] `from_source` skill resolution — *completed in Phase 3 P2*
- [ ] Plugin processing — *moved to Phase 4*
- [x] `extends` array DFS resolution + cycle detection (`config/resolve.rs`)
- [x] `persona.extends` enforcement (only `.jig.local.yaml`, validated in `validate_layer()`)
- [x] Global config ownership check (0600/0640, in `jig doctor --audit`)
- [x] Credential masking in dry-run JSON output (MCP env vars shown as `***`; history stores server names only)
- [x] Worktree detection + project `.jig.lock` concurrency warnings (`worktree.rs`, `assembly/lockfile.rs`)
- [x] Schema migration (v1→v2 with confirmation + timestamped backup) — *completed in Phase 3 P1*
- [x] `jig doctor --audit` (config validation, file permission checks, worktree detection)
- [x] MCP first-run approval (SHA-256 hash cache, same pattern as hook approval)
- [x] CI/CD: GitHub Releases binaries (macOS/Linux x86/arm64), headless size gate (< 5 MB) — *completed in Phase 3 P1*
- [x] Homebrew tap + curl installer + `cargo binstall` support — *completed in Phase 3 P1* (Homebrew formula in separate `jdforsythe/homebrew-jig` repo)
- [x] Project `.jig.lock` lock file (global `~/.config/jig/jig.lock` — *completed in Phase 3 P1*)

---

## Phase 3 — Editor Mode + Skill Registry + Distribution [COMPLETE]

### P0 — TUI Editor Mode (deferred from Phase 2 P1)

- [x] **"Custom / Ad-hoc" entry in template list** — `[Custom / ad-hoc]` below `None (no template)`. Pressing Enter opens Editor Mode inline rather than launching. Actions: `[Launch]` (one-off, no save) and `[Save as template]`. Must use the same Editor Mode screen as `jig template new|edit` — no separate screen.

- [x] **Editor Mode** — section-based TUI editing screen. Sections: skills, MCP servers, permissions (allowed/disallowed tools), persona, context fragments, hooks, model, passthrough flags. Undo stack (Ctrl-Z). Scope selector (global/project/local) shown when saving. Live preview pane updates as fields change. Entry points: `e` on selected template/persona in main TUI, `[Custom / ad-hoc]` entry, `jig template new`. Vim keybindings throughout; which-key popup shows available keys.

### P1 — Infrastructure (deferred from Phase 2 P4)

- [x] **Schema migration v1→v2** — `jig doctor --migrate` command: detect outdated schema, show diff of changes, prompt confirmation, write backup (`<file>.bak.<timestamp>`), write migrated file. Must be chainable for future versions. Currently `v1` is the only version; infrastructure implemented so adding v2 is a one-file change.

- [x] **Global `~/.config/jig/jig.lock`** — alongside the project-level `.jig.lock`, write a global lock entry in `~/.config/jig/jig.lock` (JSONL, one record per active session). Enables `jig ps` in the future and cross-directory session awareness. Cleanup in `SessionGuard::drop()` (Category A).

- [x] **CI/CD: GitHub Releases binaries** — GitHub Actions workflow: build macOS (arm64/x86_64) and Linux (arm64/x86_64) binaries; universal macOS binary via `lipo`; headless binary size gate (< 5 MB, `--no-default-features`); attach to GitHub Releases on tag push.

- [x] **Homebrew tap + curl installer + `cargo binstall` support** — `jdforsythe/homebrew-jig` tap with formula (separate repo); `install.sh` curl installer script targeting GitHub Releases; `cargo binstall` metadata in `Cargo.toml`.

### P2 — Skill Registry + Sync (was Phase 3, absorbs `from_source`)

- [x] **`from_source` skill resolution** — (moved from Phase 2 P4) — resolve named skill sources to local paths via `SourceConfig` in global/project config. Error with actionable message if source not yet synced.

- [x] **`jig sync`** — fetch/update skills from configured git sources (shell out to `git`, not `git2`; `--no-recurse-submodules`). `--frozen` refuses to update (CI mode). `--check` reports staleness without pulling.

- [x] **Skill indexing from `SKILL.md` frontmatter** — parse YAML frontmatter (`name`, `description`, `tags`, `version`) from each skill file. Used by `jig skill search`. Soft parse: malformed frontmatter returns defaults, never errors.

- [x] **Full-copy override layer** — skills from sources cached in `~/.config/jig/skills/<source>/`; overrides tracked in `~/.config/jig/skills-override/<source>/`; `jig skill diff` shows unified diff from upstream; `jig skill reset` removes override to restore upstream.

- [x] **`jig skill search|info|override|diff|reset`** — search indexed skills by tag/keyword; show metadata; create local override; diff vs upstream; restore upstream. Plus `jig skill list [--source S]`.

- [x] **SHA-256 integrity verification** — verify fetched skill files against lockfile hashes on load; warn if tampered.

- [x] **Lock file update on sync** — `jig sync` updates `~/.config/jig/skills.lock` (TOML) with source SHAs and fetched-at timestamps.

---

## Phase 4 — Team + Bootstrapping [PLANNED]

- [ ] Plugin processing (`--plugin-dir`, `installed_plugins.json` lookup, install prompt) — (moved from Phase 2 P4) — discover installed Claude Code plugins; validate against `installed_plugins.json`; prompt to install missing plugins referenced in config.
- [ ] Dependency resolution + install prompts for missing skills/plugins
- [ ] Plugin marketplace integration (`claude plugin install`)
- [ ] `jig template export|import` (share via URL/gist)
- [ ] Shell completions (bash/zsh/fish; < 100ms, returns empty on error, CWD-aware)
- [ ] Context versioning in history (note fragment changes since last session)
- [ ] Structured audit events for SOC2 (config hash, MCP servers in history)

---

## Phase 5 — Ecosystem [PLANNED]

- [ ] `jig serve --mcp` — 14-tool MCP server over stdio transport
- [ ] jig-config-helper plugin (craft `.jig.yaml` inside Claude)
- [ ] persona-crafter plugin (design custom personas interactively)
- [ ] Dynamic context injection (git branch, recent commits into fragments)
- [ ] CI/CD headless mode for `claude -p` pipelines
- [ ] `jig doctor --audit` full security review (elevated beyond Phase 3 checks)
- [ ] JSON Schema published to SchemaStore
- [ ] jig.dev landing page + 15-second GIF demo
- [ ] `jig template share` (gist URL generation)

---

## Architecture Constraints

These constraints apply across all phases and are maintained in `CLAUDE.md`:

- `jig-core` must compile without TUI deps — never import ratatui types there
- `~/.claude.json` is always `serde_json::Value` — never a typed struct (unknown fields must survive round-trips)
- `process::exit()` is forbidden after `SessionGuard` is live — only `exec` or normal return are valid exits
- The fd-lock guard on `~/.claude.json.jig.lock` must be dropped before `fork_and_exec`
- MCP cwd key lookups must use direct map navigation, not JSON Pointer (RFC 6901 `/` separator conflicts with absolute paths)
- All jig-written MCP entries must carry the session suffix — cleanup identifies entries by `name.ends_with(&suffix_marker)`
- Every `Option<T>` field on `McpServer` must have `#[serde(skip_serializing_if = "Option::is_none")]`
- `kill(pid, 0)` for PID liveness: cast through `i32::try_from(pid)` with `p > 0` guard before calling — `u32::MAX as i32 = -1` causes `kill(-1, 0)` to succeed against all processes

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
> - `jig-cli` and `jig-tui` must not remain at 0 tests
