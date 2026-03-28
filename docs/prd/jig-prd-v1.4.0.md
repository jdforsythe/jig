# jig: Intentional Context Utilization

## Product Requirements Document v1.4.0

**Date:** 2026-03-28
**Supersedes:** jig-prd-v1.3.0.md
**Status:** Phase 1-3 Complete — Phase 4 In Progress

---

## Changelog from v1.3.0

### 1. Conceptual redesign: Profiles, Toolboxes, Personas

The mental model is restructured around three concepts:

- **Toolbox**: The capabilities/tools configuration. Contains allowed/disallowed tools, MCP servers, skills, context fragments, hooks, model, env vars, flags. Named for the abilities it enables (e.g., "read-only", "full-devops"). Replaces the tooling portion of what "templates" were.
- **Persona**: Behavioral guidance rules. Unchanged from v1.3.0. Named for the real-world role (e.g., "code-reviewer", "pair-programmer").
- **Profile**: A named combination of toolbox + persona. The primary user-facing concept. Named for the task the human is trying to accomplish (e.g., "code-review" = "read-only" toolbox + "code-reviewer" persona). Replaces "templates" as the top-level concept.

This is a **breaking change** for the CLI: `-t`/`--template` is renamed to `-t`/`--toolbox`. A new `--profile` flag selects a profile by name.

### 2. Bug fixes from Phase 3 testing

Seven bugs discovered during playbook testing are fixed in a dedicated PR before the redesign:

| ID | Bug | Severity |
|----|-----|----------|
| B1 | `jig history` crashes on string slicing | Critical |
| B2 | `jig config show` omits model and most fields in non-JSON mode | High |
| B3 | `jig config add allowed_tools` uses wrong YAML key (snake_case vs camelCase) | High |
| B4 | `?` in editor mode exits to main screen, losing all progress | High |
| B5 | Ctrl+S / `:w` save popup never renders (implemented but not wired) | Critical |
| B6 | No way to launch a session from editor mode | Critical |
| B7 | Preview pane omits model, hooks, MCP servers, context, flags | Medium |

### 3. TUI redesign

The main TUI screen changes from a two-list layout (templates + personas) to a profiles-first layout:

- **Left pane**: Single profile list (builtin profiles + "Custom / ad-hoc")
- **Right pane**: Preview showing the composed result (toolbox config + persona rules)
- **Custom / ad-hoc**: Opens a sub-screen with toolbox picker + persona picker
- **Editor mode**: Two-panel layout — toolbox sections (left) + persona sections (right) + preview

### 4. Phase restructure

Phase 4 is split into two sub-phases:
- **Phase 4a**: Bug fixes (PR 1)
- **Phase 4b**: Profiles/Toolboxes redesign (PR 2)

Phase 5 (Team + Bootstrapping) renumbered from old Phase 4. Phase 6 (Ecosystem) from old Phase 5.

---

## Phase 4a — Bug Fixes [PLANNED]

### B1: `jig history` crash

- [ ] Fix dangling string reference in `subcommands.rs:412-414` — store `as_str().unwrap_or("?")` in let binding before slicing
- [ ] Tests: `test_history_short_session_id_no_panic`, `test_history_missing_fields_no_panic`

### B2: `jig config show` incomplete non-JSON output

- [ ] Expand non-JSON display in `subcommands.rs:657-666` to include: model, allowed_tools, disallowed_tools, hooks count, context_fragments count, env count, claude_flags, persona rules count
- [ ] Tests: `test_config_show_resolved_has_all_fields`

### B3: `jig config add/set/remove` key normalization

- [ ] Add `normalize_config_key()` mapping `allowed_tools` → `allowedTools`, `disallowed_tools` → `disallowedTools`
- [ ] Apply `normalize_dotted_key()` to key arguments in `handle_config()` Set/Add/Remove branches
- [ ] Tests: `test_normalize_dotted_key`, `test_add_config_value_with_normalized_key`

### B4: `?` in editor mode help overlay

- [ ] Add `show_help: bool` field to `EditorState`
- [ ] `?` toggles `show_help` instead of setting `AppMode::WhichKey`
- [ ] Any key while `show_help==true` dismisses the overlay
- [ ] `render_editor()` renders help overlay when `show_help` is true
- [ ] Tests: `test_question_mark_toggles_help_not_mode`

### B5: Save popup wiring (Ctrl+S / `:w`)

- [ ] Add `save_popup: Option<SavePopupState>` to `EditorState`
- [ ] Ctrl+S and `:w` create `SavePopupState` + set `AppMode::EditorSave`
- [ ] Add `handle_save_key()` routing keys to `SavePopupState::handle_key()`
- [ ] On `SavePopupResult::Save`, call `draft.save_as_template(scope, project_dir)`
- [ ] Route `EditorSave` keys separately in `app.rs` and `run_editor_tui()`
- [ ] `render_editor()` renders popup overlay when `save_popup.is_some()`
- [ ] Tests: `test_ctrl_s_creates_save_popup`, `test_save_popup_cancel_returns_to_editor`

### B6: Launch from editor mode

- [ ] Add `EditorResult` enum (`Cancelled`, `Launch`) and `result: Option<EditorResult>` to `EditorState`
- [ ] `Ctrl+Enter` sets `result=Launch` + `mode=Normal`
- [ ] `:launch` and `:l` colon commands do the same
- [ ] `app.rs`: when editor exits with `result==Launch`, convert draft to `launch_selection`
- [ ] `run_editor_tui()`: return `Some(draft)` when `result==Launch`
- [ ] Update which-key help to show launch bindings
- [ ] Tests: `test_ctrl_enter_sets_launch_result`, `test_colon_launch_sets_launch_result`

### B7: Preview completeness

- [ ] Expand `PreviewData` struct with: `model`, `mcp_server_names: Vec<String>`, `context_fragment_count`, `pre_launch_hook_count`, `post_exit_hook_count`, `claude_flags`, `allowed_tools`, `disallowed_tools`
- [ ] Populate new fields in `build_preview_data()` and `resolve_draft_preview()`
- [ ] Update `refresh_preview()` in `app.rs` to include all fields in markdown output
- [ ] Update `refresh_preview()` in `editor/mod.rs` to include all fields
- [ ] Tests: `test_resolve_draft_preview_includes_model_and_mcp`, `test_build_preview_data_includes_all_summary_fields`

---

## Phase 4b — Profiles/Toolboxes Redesign [PLANNED]

### P0: Schema + Builtins

- [ ] Add `Toolbox` struct to `config/schema.rs` (name, description, config: JigConfig)
- [ ] Add `ProfileDef` struct to `config/schema.rs` (name, description, toolbox: String, persona: String)
- [ ] Add `builtin_toolboxes()` to `defaults.rs`: full-access, read-only, full-devops, full-frontend
- [ ] Add `builtin_profile_defs()` to `defaults.rs`: code-review (read-only + code-reviewer), security-audit (read-only + strict-security), pair-programming (full-access + pair-programmer), tdd (full-access + tdd), documentation (full-access + docs-writer), devops (full-devops + default), frontend (full-frontend + default)
- [ ] Keep `builtin_templates()` as deprecated alias
- [ ] Tests: `test_builtin_toolbox_count`, `test_builtin_profile_defs_reference_valid_toolboxes`, `test_builtin_profile_defs_reference_valid_personas`

### P1: CLI Flags + Subcommands

- [ ] Rename `-t`/`--template` to `-t`/`--toolbox` in `Cli` struct (breaking)
- [ ] Add `--profile` flag to `Cli` struct
- [ ] Add `Profile` subcommand (list, show)
- [ ] Add `Toolbox` subcommand (list, show)
- [ ] Keep `Template` subcommand as deprecated alias
- [ ] Add dispatch handlers: `handle_profile_cmd()`, `handle_toolbox_cmd()`
- [ ] Tests: `test_cli_toolbox_flag_parses`, `test_cli_profile_flag_parses`

### P2: Config Resolver Updates

- [ ] Add `toolbox: Option<String>` and `profile: Option<String>` to `CliOverrides`
- [ ] When `profile` is set: resolve to (toolbox, persona) from `builtin_profile_defs()`; look up toolbox config from `builtin_toolboxes()`; look up persona from `builtin_personas()`
- [ ] When `toolbox` is set: load from `builtin_toolboxes()` (replaces template lookup)
- [ ] Add `profile_name: Option<String>` and `toolbox_name: Option<String>` to `ResolvedConfig`
- [ ] Wire in `main.rs`: `cli.toolbox` → `overrides.toolbox`, `cli.profile` → `overrides.profile`
- [ ] Tests: `test_profile_resolves_to_toolbox_and_persona`, `test_toolbox_override_resolves_correctly`

### P3: TUI Main Screen — Profiles-First Layout

- [ ] Replace `templates: FilterableListState` with `profiles: FilterableListState` in `App`
- [ ] Populate from `builtin_profile_defs()` + "Custom / ad-hoc" sentinel
- [ ] Replace `PaneFocus::Templates` with `PaneFocus::Profiles`
- [ ] Enter on profile → resolve to (toolbox, persona) → set `launch_selection` → quit
- [ ] Enter on "Custom / ad-hoc" → enter `AppMode::CustomPicker`
- [ ] `refresh_preview()` resolves selected profile to toolbox+persona for preview
- [ ] Preview shows composed result: toolbox config summary + persona rules

### P4: Custom / Ad-Hoc Picker Sub-Screen

- [ ] Add `AppMode::CustomPicker` to mode enum
- [ ] Add `toolboxes: FilterableListState` to `App` (from `builtin_toolboxes()` + "None")
- [ ] Keep `personas: FilterableListState` (already exists)
- [ ] CustomPicker renders two panes: toolboxes (left-top) + personas (left-bottom) + preview (right)
- [ ] Tab switches between toolbox and persona focus
- [ ] Enter launches with selected toolbox+persona
- [ ] `e` enters editor with toolbox pre-loaded
- [ ] Esc returns to main profile list

### P5: Editor Two-Panel Layout

- [ ] Add `EditorPanel` enum (Toolbox, Persona) to `editor/mod.rs`
- [ ] Add `active_panel: EditorPanel` to `EditorState`
- [ ] Toolbox panel sections: AllowedTools, DisallowedTools, Model, McpServers, ContextFragments, Hooks, PassthroughFlags
- [ ] Persona panel sections: PersonaName, PersonaRules
- [ ] `H`/`L` switches between panels
- [ ] `render_editor()` draws two panels side-by-side
- [ ] Preview section shows toolbox config summary (top) + persona rules (bottom)
- [ ] Tests: `test_panel_switch_key`, `test_toolbox_sections_in_toolbox_panel`

### P6: Update `jig import` scope

- [ ] Expand `import_from_claude_json()` to also import: allowed/disallowed tools, model preferences from `~/.claude.json` project settings (if they exist)
- [ ] Generate `profile.settings` block in addition to `profile.mcp` block

---

## Phase 5 — Team + Bootstrapping [PLANNED]

(Renumbered from old Phase 4)

- [ ] Plugin processing (`--plugin-dir`, `installed_plugins.json` lookup, install prompt)
- [ ] Dependency resolution + install prompts for missing skills/plugins
- [ ] Plugin marketplace integration (`claude plugin install`)
- [ ] `jig profile export|import` (share via URL/gist)
- [ ] Shell completions (bash/zsh/fish)
- [ ] Context versioning in history
- [ ] Structured audit events for SOC2
- [ ] Schema v2 definition
- [ ] `jig ps` — list active sessions

---

## Phase 6 — Ecosystem [PLANNED]

(Renumbered from old Phase 5)

- [ ] `jig serve --mcp` — MCP server over stdio
- [ ] jig-config-helper plugin
- [ ] persona-crafter plugin
- [ ] Dynamic context injection
- [ ] CI/CD headless mode
- [ ] JSON Schema published to SchemaStore
- [ ] jig.dev landing page
- [ ] `jig profile share`

---

## Architecture Constraints

All constraints from v1.3.0 remain. Additional:

- **Toolbox configs must not contain persona data.** A `Toolbox.config.persona` must be `None`. Persona configuration is always separate. The assembly pipeline merges them at resolution time.
- **Profile names must be unique across builtins and user-defined profiles.** User profiles in `.jig.yaml` override builtins with the same name.
- **`-t` is a breaking rename.** Users with scripts using `-t template-name` must update to either `-t toolbox-name` or `--profile profile-name`. Migration guidance in changelog.

---

## Builtin Reference

### Toolboxes

| Name | Description | Allowed | Disallowed |
|------|-------------|---------|------------|
| full-access | No restrictions | (all) | (none) |
| read-only | Read-only access | Read, Grep, Glob | Bash, Edit, Write |
| full-devops | DevOps tooling | Bash, Edit, Read | (none) |
| full-frontend | Frontend development | Bash, Edit, Read, Write | (none) |

### Personas

(Unchanged from v1.3.0: default, strict-security, mentor, pair-programmer, code-reviewer, architect, minimalist, tdd, docs-writer, performance)

### Profiles

| Name | Toolbox | Persona | Description |
|------|---------|---------|-------------|
| code-review | read-only | code-reviewer | Read-only code review |
| security-audit | read-only | strict-security | Security-focused audit |
| pair-programming | full-access | pair-programmer | Collaborative coding |
| tdd | full-access | tdd | Test-driven development |
| documentation | full-access | docs-writer | Documentation writing |
| devops | full-devops | default | DevOps session |
| frontend | full-frontend | default | Frontend development |
