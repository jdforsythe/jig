# jig: Intentional Context Utilization

## Product Requirements Document v1.3.0

**Date:** 2026-03-27
**Supersedes:** jig-prd-v1.2.0.md
**Status:** Phase 1 Complete — Phase 2 Mostly Complete — Phase 3 Complete — Phase 4 Planning

---

## Changelog from v1.2.0

### 1. Phase 3 completion

Phase 3 ("Editor Mode + Skill Registry + Distribution") is now **fully complete** with 197 passing tests (18 jig-cli + 143 jig-core + 36 jig-tui), 1 ignored (git integration test gated on `JIG_RUN_GIT_TESTS=1`).

**P0 — TUI Editor Mode (complete):**
- `[Custom / ad-hoc]` entry added at index 1 in TUI template list (below `None (no template)`)
- Full Editor Mode TUI: 9 sections (Skills, MCP Servers, Allowed Tools, Disallowed Tools, Persona, Context Fragments, Hooks, Model, Passthrough Flags)
- Vim keybindings: `j`/`k` within section, `J`/`K`/Tab/Shift-Tab between sections, `gg`/`G` first/last, `a` append, `d` delete, `i`/Enter edit, Ctrl-Z undo
- `:w`/Ctrl-S save with scope selector popup (Global/Project/Local)
- `?` which-key popup showing all editor bindings
- `UndoStack<EditorDraft>` capped at 50 snapshots
- Live preview pane with 100ms debounce
- `e` key on any non-sentinel template in main TUI loads it into Editor Mode
- `jig template new` / `jig template edit <name>` CLI entry points
- `EditorDraft` ↔ `JigConfig` round-trip via `to_jig_config()` / `from_jig_config()`
- `save_as_template()` with scope-aware path resolution

**P1 — Infrastructure (complete):**
- Schema migration: `Migration` trait with `from_version`/`to_version`/`migrate`; `migration_chain(from)` chaining; `apply_migration_chain(path, from, confirm)` with atomic YAML backup (`<file>.bak.<timestamp>`)
- `V1ToV2` placeholder migration (infrastructure ready; real v2 schema in Phase 4)
- `jig doctor --migrate`: scans all config files, runs migration chain with interactive confirmation
- Global `~/.config/jig/jig.lock` JSONL: per-session `{ pid, session_id, started_at, cwd }` entries; `write_global_lock` + `remove_global_lock` (atomic rewrite); `active_sessions` filters by PID liveness; cleanup in `SessionGuard::drop()` (Category A)
- GitHub Actions CI: test + clippy + size-gate on every PR/push to master (`.github/workflows/ci.yml`)
- GitHub Actions Release: matrix build (linux-x86_64, linux-aarch64 via cross, macos-x86_64, macos-arm64), universal macOS via `lipo`, GitHub Releases attachment on tag push (`.github/workflows/release.yml`)
- `install.sh` curl installer: platform-detecting, downloads from GitHub Releases, installs to `~/.local/bin` by default
- `cargo binstall` metadata in `crates/jig-cli/Cargo.toml` with per-target URL overrides
- `repository` field added to `[workspace.package]` in root `Cargo.toml`

**P2 — Skill Registry + Sync (complete):**
- `SourceConfig` schema addition to `Profile`: `url`, `path` (subdirectory), `rev` (branch/tag/commit)
- `source_resolver.rs`: `resolve_from_source_skills(skills)` maps `(source, skill)` → `~/.config/jig/skills/<src>/<skill>.md` (or override path); `SourceResolveError::SkillNotSynced` with actionable message
- `sync.rs`: `sync_sources(sources, opts)` shells out to `git` (`--no-recurse-submodules`); clone on first sync; fetch + reset on update; `--frozen` fails if behind; `--check` reports staleness without pulling
- `skills_lock.rs`: TOML `~/.config/jig/skills.lock` with `{ url, fetched_at, sha, rev, skills: { name: { sha256, size_bytes } } }`; `verify_skill_integrity(source, skill, path)` checks SHA-256
- `skill_meta.rs`: soft-parse YAML frontmatter (`name`, `description`, `tags`, `version`) from skill `.md` files; malformed or missing frontmatter returns `SkillMeta::default()` (never errors)
- `skill_index.rs`: JSON index at `~/.config/jig/state/skill-index.json`; `rebuild_index()` scans `~/.config/jig/skills/<source>/*.md`; `search(index, query)` substring matches name/description/tags (case-insensitive)
- Assembly pipeline integrates source skill resolution non-fatally (warn + continue on `SkillNotSynced`)
- `jig sync [--frozen] [--check]`: resolves config sources → sync → update skills.lock → rebuild index
- `jig skill list [--source S]`: lists skills from index
- `jig skill search <query> [--json]`: substring search across name/description/tags
- `jig skill info <source> <skill>`: metadata + lock info + integrity status
- `jig skill override <source> <skill>`: copies upstream to `~/.config/jig/skills-override/<src>/<skill>.md`
- `jig skill diff <source> <skill>`: inline unified diff of override vs upstream
- `jig skill reset <source> <skill> [-y]`: removes override file

### 2. API delta from v1.2.0

**New CLI flags/commands:**
- `jig doctor --migrate` — runs schema migration chain on all config files
- `jig template new` — opens Editor Mode TUI for a new template
- `jig template edit <name>` — opens Editor Mode TUI for an existing template
- `jig sync --check` — check staleness without pulling
- `jig skill list|search|info|override|diff|reset` — full skill management surface

**New config schema fields:**
- `Profile.sources: Option<HashMap<String, SourceConfig>>` — skill source registry
- `SourceConfig { url: String, path: Option<String>, rev: Option<String> }`

**New jig-core modules:**
- `config::migration` — `Migration` trait, `MigrationError`, `MigrationOutcome`, `all_migrations()`, `migration_chain()`, `apply_migration_chain()`
- `assembly::global_lock` — `GlobalLockRecord`, `global_lock_path()`, `write_global_lock()`, `remove_global_lock()`, `active_sessions()`
- `assembly::source_resolver` — `resolve_from_source_skills()`, `skill_file_path()`, `override_skill_path()`
- `assembly::sync` — `sync_sources()`, `SyncOptions`, `SyncAction`, `SyncOutcome`
- `assembly::skills_lock` — `SkillsLock`, `SourceLockEntry`, `SkillLockEntry`, TOML I/O, `verify_skill_integrity()`
- `assembly::skill_meta` — `SkillMeta`, `parse_frontmatter()`, `parse_frontmatter_str()`
- `assembly::skill_index` — `SkillIndex`, `IndexedSkill`, `rebuild_index()`, `read_index()`, `write_index()`, `search()`
- `editor` — `EditorDraft`, `SaveScope`, `to_jig_config()`, `from_jig_config()`, `save_as_template()`, `resolve_draft_preview()`, `load_draft_for_template()`

**New jig-tui modules:**
- `editor` — `EditorState`, `EditorSection`, `SectionInputMode`, `EditorEntryPoint`, `run_editor_tui()`
- `editor::undo` — `UndoStack<T: Clone>` capped at 50
- `editor::sections` — `ListSectionState` helpers
- `editor::render` — all editor rendering functions
- `editor::save_popup` — `SavePopupState`, `render_save_popup()`
- `editor::which_key` — `editor_bindings()` key binding table

**New files added:**
- `.github/workflows/ci.yml` — CI pipeline
- `.github/workflows/release.yml` — release pipeline
- `install.sh` — curl installer

### 3. Architecture additions

**New constraints (update CLAUDE.md when surfaced):**
- `assembly::global_lock` records are JSONL. `remove_global_lock` must use atomic write-to-temp + rename to avoid partial reads. Same pattern as `lockfile.rs`.
- `skill_meta::parse_frontmatter_str` must never return `Err` — frontmatter parse failures yield `SkillMeta::default()`. This is intentional: skill files may not have frontmatter and that is valid.
- `sync.rs` shells out to `git` (not `git2`). CI cross-compilation targets must have `git` available. The `JIG_RUN_GIT_TESTS=1` env var gates integration tests that require a real git remote.
- `toml = "0.8"` added to workspace dependencies and `jig-core`. Required for `skills_lock.rs` TOML I/O.

### 4. Phase restructure

No structural changes in this version. Phase 4 (Team + Bootstrapping) remains as planned. The schema migration infrastructure (`V1ToV2` placeholder) is ready for Phase 4 to define the actual v2 schema changes.

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

P0, P2, P3 fully complete. P1 Editor Mode completed in Phase 3. Remaining P4 infra items moved to Phase 3 (complete) or Phase 4.

### P0 — Critical [COMPLETE]

- [x] **Config precedence fix** — `ConfigSource::ExplicitCliFlag` ranked above `ConfigSource::TemplateSelected`
- [x] **Hook execution** — `HookEntry::Exec` and `HookEntry::Shell { shell: true }` dispatch; `pre_launch` / `post_exit`
- [x] **Env var expansion in MCP** — `${VAR}` and `${VAR:-default}` expanded at assembly time; approval hashes use pre-expansion strings

### P1 — TUI Improvements [COMPLETE via Phase 3]

- [x] **"None" option for template and persona** — first entries in TUI lists
- [x] **"Custom / Ad-hoc" entry in template list** — completed in Phase 3 P0
- [x] **Editor Mode** — completed in Phase 3 P0
- [x] **`jig init` interactive wizard** — project-type detection, template suggestion, scaffold

### P2 — Session Management [COMPLETE]

- [x] **`jig history [--limit N] [--verbose]`**
- [x] **`jig --last [-p P]`** and **`jig --resume`** / **`jig --session <UUID>`**
- [x] **Session history view in TUI** — `h` key overlay, `L` relaunch
- [x] **`--dry-run --json`** structured output

### P3 — Config Management CLI [COMPLETE]

- [x] **`jig config set/add/remove`** — dotted path notation, `--scope` flag
- [x] **`jig import [--dry-run]`** — reverse-engineers `~/.claude.json` MCP servers
- [x] **`jig diff <config>`** — structured line-level diff

### P4 — Security + Infrastructure [PARTIAL → COMPLETE via Phase 3]

- [x] `from_source` skill resolution — completed in Phase 3 P2
- [ ] Plugin processing — moved to Phase 4
- [x] `extends` array DFS resolution + cycle detection
- [x] `persona.extends` enforcement
- [x] Global config ownership check (0600/0640)
- [x] Credential masking in dry-run JSON output
- [x] Worktree detection + project `.jig.lock` concurrency warnings
- [x] Schema migration — completed in Phase 3 P1
- [x] `jig doctor --audit`
- [x] MCP first-run approval (SHA-256 hash cache)
- [x] CI/CD: GitHub Releases binaries — completed in Phase 3 P1
- [x] Homebrew tap + curl installer + `cargo binstall` — completed in Phase 3 P1
- [x] Global `~/.config/jig/jig.lock` — completed in Phase 3 P1

---

## Phase 3 — Editor Mode + Skill Registry + Distribution [COMPLETE]

### P0 — TUI Editor Mode [COMPLETE]

- [x] **"Custom / Ad-hoc" entry in template list** — `[Custom / ad-hoc]` at index 1 in TUI
- [x] **Editor Mode** — 9-section TUI with undo, vim keybindings, which-key, live preview, scope selector

### P1 — Infrastructure [COMPLETE]

- [x] **Schema migration v1→v2** — `jig doctor --migrate`; `Migration` trait; atomic backup; chainable
- [x] **Global `~/.config/jig/jig.lock`** — JSONL per-session records; Category A cleanup
- [x] **CI/CD: GitHub Releases binaries** — 4-platform matrix + universal macOS; size gate < 5 MB
- [x] **Homebrew tap + curl installer + `cargo binstall`** — `install.sh`, binstall metadata; Homebrew in `jdforsythe/homebrew-jig`

### P2 — Skill Registry + Sync [COMPLETE]

- [x] **`from_source` skill resolution** — `SourceConfig` in schema; `source_resolver.rs`; actionable error if not synced
- [x] **`jig sync [--frozen] [--check]`** — git-based clone/update; lock update; index rebuild
- [x] **Skill indexing from frontmatter** — soft-parse `name`/`description`/`tags`/`version`; JSON index
- [x] **Full-copy override layer** — `~/.config/jig/skills-override/`; `jig skill override|diff|reset`
- [x] **`jig skill list|search|info|override|diff|reset`** — full skill management surface
- [x] **SHA-256 integrity verification** — `verify_skill_integrity(source, skill, path)` in skills_lock
- [x] **Lock file update on sync** — `~/.config/jig/skills.lock` TOML with source SHAs + per-skill hashes

---

## Phase 4 — Team + Bootstrapping [PLANNED]

- [ ] Plugin processing (`--plugin-dir`, `installed_plugins.json` lookup, install prompt) — discover installed Claude Code plugins; validate against `installed_plugins.json`; prompt to install missing plugins referenced in config.
- [ ] Dependency resolution + install prompts for missing skills/plugins
- [ ] Plugin marketplace integration (`claude plugin install`)
- [ ] `jig template export|import` (share via URL/gist)
- [ ] Shell completions (bash/zsh/fish; < 100ms, returns empty on error, CWD-aware)
- [ ] Context versioning in history (note fragment changes since last session)
- [ ] Structured audit events for SOC2 (config hash, MCP servers in history)
- [ ] Schema v2 definition — when a real schema change is needed, implement the `V1ToV2` migration body (currently a placeholder that bumps version only)
- [ ] `jig ps` — list active sessions from `~/.config/jig/jig.lock` (infrastructure already in place)

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
- `assembly::global_lock` and `assembly::lockfile` both use atomic write-to-temp + rename for mutations — never partial writes
- `skill_meta::parse_frontmatter_str` must never return `Err` — soft parse, return `SkillMeta::default()` on any failure
- `sync.rs` shells out to `git` (not `git2`); integration tests gated on `JIG_RUN_GIT_TESTS=1`

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
> - Git integration tests gated on `JIG_RUN_GIT_TESTS=1` env var

### Test count at Phase 3 completion

| Crate | Tests | Ignored |
|-------|-------|---------|
| jig-cli | 18 | 0 |
| jig-core | 143 | 1 |
| jig-tui | 36 | 0 |
| **Total** | **197** | **1** |
