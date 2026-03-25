# jig: Intentional Context Utilization

## Product Requirements Document v1.0.0

**Date:** 2026-03-25
**Supersedes:** jig-prd-v0.6.0.md
**License:** Dual MIT/Apache-2.0
**Package:** jig-icu | **Binary:** jig
**Monorepo:** github.com/jig-icu/jig (just + Cargo workspace)
**Platforms:** macOS (x86_64/ARM64), Linux (x86_64/ARM64). No Windows native — Claude Code on Windows is WSL2-only.
**Status:** Implementation Ready — All design decisions locked, all open questions resolved.

**What changed in v1.0.0:** All open questions from v0.6.0 are resolved. Seven decisions were finalized in the 2026-03-25 brainstorm. A full implementation planning round (24 research agents) resolved blocking issues in dependency selection, security model, assembly pipeline, TUI architecture, and agent-native parity. This document supersedes v0.6.0 entirely.

---

## 1. Vision

> *In woodworking, a jig is a custom-made tool that holds exactly the right piece in exactly the right position. Every jig is different — built for one specific cut, one specific joint. You make your own. It's reusable but disposable. It doesn't do the work; it makes the work precise.*

**jig** is a Rust TUI that orchestrates Claude Code sessions by composing templates, personas, skills, plugins, and context into a precise launch configuration. Zero CWD mutation. Clean git status. Every session exactly what you need.

### 1.1 Problem

Claude Code loads everything available at session start — all installed plugins, all skills, the full CLAUDE.md, every MCP server. Power users accumulate dozens of extensions across projects, but any given session needs a precise subset. Loading everything wastes context tokens, introduces irrelevant instructions, and dilutes Claude's focus. There is no native mechanism for selective, composable, per-session configuration.

### 1.2 What jig Is NOT

- **Not a session multiplexer.** CCManager and Claude Squad handle parallel sessions and worktree management. jig composes the *config* for a session, not the session lifecycle.
- **Not a plugin package manager.** Claude Code has `/plugin`, marketplaces, and `claude plugin install`. jig orchestrates which installed plugins are *active* for a given session and manages standalone skills that aren't in plugin format.
- **Not a replacement for CLAUDE.md.** jig composes and injects context through Claude Code's native mechanisms. It doesn't bypass Claude Code's built-in config loading.

### 1.3 Core Principle: Zero CWD Mutation

jig NEVER writes files to the working directory. `git status` is identical before, during, and after a jig session. This prevents:
- Coding agents accidentally committing jig's config files
- Injected config appearing as changes in editors
- Conflicts between concurrent sessions in the same directory
- `git add .` capturing session-specific files into PRs

All configuration is delivered through CLI flags, staged temp directories, and `~/.claude.json` (user-level file), never project files.

---

## 2. Core Concepts

### 2.1 Three Layers

jig separates *what tools Claude has* from *how Claude behaves* from *what Claude knows about the project*:

```
┌──────────────────────────────────────────────────────────┐
│                       SESSION                             │
│                                                           │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │   TEMPLATE    │  │   PERSONA    │  │    CONTEXT     │  │
│  │              │  │              │  │                │  │
│  │ Skills       │  │ Behavioral   │  │ Project info   │  │
│  │ Plugins      │  │ rules &      │  │ Docs & refs    │  │
│  │ MCP servers  │  │ instructions │  │ Knowledge      │  │
│  │ Permissions  │  │ Tone, depth  │  │ bases          │  │
│  │ Hooks        │  │ Guardrails   │  │                │  │
│  │              │  │              │  │                │  │
│  │ "The Toolbox"│  │ "The Mindset"│  │ "The Briefing" │  │
│  └──────────────┘  └──────────────┘  └────────────────┘  │
└──────────────────────────────────────────────────────────┘
```

- **Template** = what tools/skills/plugins are available. *The toolbox.*
- **Persona** = how Claude should behave. Rules, tone, guardrails, approach. *The mindset.*
- **Context** = what Claude knows about this project/task. Fragments of project-specific information. *The briefing.*

These compose independently. A "code-review" persona can pair with a "devops" template. A "mentoring" persona (explain everything, never shortcut) can pair with the same "devops" template for a junior dev. Template stacking is supported: `extends: [base-devops, code-review]` merges both in order.

### 2.2 One Schema, Scoped by Location

There is ONE config format. The only difference is override scope:

```
CLI flags  >  .jig.local.yaml  >  .jig.yaml  >  ~/.config/jig/config.yaml
     ↑             ↑                  ↑                   ↑
  Per-session   Personal          Project team         User global
  (highest)     (gitignored)      (committed)          (lowest)
```

Every file uses the same schema. Higher specificity wins. No global config is required — a committed `.jig.yaml` is self-contained and bootstrappable.

### 2.3 Self-Bootstrapping Projects

A `.jig.yaml` declares everything needed to reproduce a session, including *where to get* dependencies:

```yaml
schema: 1
profile:
  skills:
    from_source:
      composio: [docker, kubernetes, terraform]
  plugins:
    - name: formatter
      marketplace: claude-plugins-official
```

When a teammate clones the repo and runs `jig`:

```
$ jig

  This project needs 3 items not yet installed:
    Plugin: formatter (from claude-plugins-official)
    Skill: docker (from composio)
    Skill: kubernetes (from composio)

  Install? [Y/n]
```

After install, the session launches with exactly those tools active.

---

## 3. Mechanisms — How jig Maps to Claude Code

### 3.1 Complete Mechanism Map

| Config dimension | Mechanism | CWD impact |
|-----------------|-----------|------------|
| Persona rules + context fragments | `--append-system-prompt-file /tmp/jig-xxx/prompt.md` | None (temp dir) |
| Skills | `--add-dir /tmp/jig-xxx/skills` | None (temp dir) |
| Installed plugins | `--plugin-dir ~/.claude/plugins/cache/mkt/plugin/ver` | None (existing cache) |
| Local/staged plugins | `--plugin-dir /tmp/jig-xxx/plugins/X` | None (temp dir) |
| Permissions (allow) | `--allowedTools "Bash(pnpm *),Bash(turbo *)"` | None (CLI flag) |
| Permissions (deny) | `--disallowedTools "Read(./.env)"` | None (CLI flag) |
| MCP servers | `~/.claude.json` → `projects."<cwd>".mcpServers` | None (user-level file) |
| Model selection | `--model claude-sonnet-4-20250514` | None (CLI flag) |
| Environment variables | Exported by parent process | None |
| Knowledge bases / reference dirs | `--add-dir ~/path/to/docs` | None (CLI flag) |

The ONLY file jig mutates is `~/.claude.json` — a user-level file outside the project, modified atomically with file locking.

### 3.2 Plugin Discovery

jig reads `~/.claude/plugins/installed_plugins.json` to find where installed plugins are cached:

```json
{
  "formatter@claude-plugins-official": [{
    "scope": "user",
    "installPath": "/home/user/.claude/plugins/cache/claude-plugins-official/formatter/1.2.0",
    "version": "1.2.0"
  }]
}
```

When a profile references a plugin, jig looks up the `installPath` and passes it as `--plugin-dir`. Multiple `--plugin-dir` flags are supported and additive.

For plugins NOT yet installed, jig prompts to install via `claude plugin install`.

### 3.3 `~/.claude.json` Safety Protocol

```
1. flock(~/.claude.json.jig.lock, LOCK_EX)     # dedicated lock file — never the target
2. Read existing JSON into raw serde_json::Value  # NEVER through a typed struct
3. Write backup atomically: content → .jig-backup-<pid>.tmp → rename
4. Detect MCP naming conflicts → build RenameMap
5. Apply renames to new server entries
6. Rewrite permission entries for renamed servers (allowedTools/disallowedTools)
7. Merge into projects."<abs_cwd>".mcpServers
8. Write to ~/.claude.json.jig-<pid>.tmp         # sibling file, same filesystem
9. sync_data() on tmp file
10. Atomic rename: .tmp → ~/.claude.json
11. Increment ref count: ~/.config/jig/state/<cwd-hash>.refcount  # MUST be inside flock
12. Release lock
13. Drop lock guard BEFORE exec("claude")         # critical: prevents fd inheritance
```

**Lock file:** Use a dedicated stable file (`~/.claude.json.jig.lock`) — never flock the target. POSIX rename replaces the inode, so flocking the target leaves other processes holding a lock on a stale inode.

**`~/.claude.json` value type:** Always use `serde_json::Value` (raw JSON) for the entire file. Never deserialize through a typed struct — unknown keys from future Claude Code versions would be silently dropped. Only extract/inject the `projects."<cwd>".mcpServers` subtree.

**Atomic tmp file placement:** The tmp file must be a sibling of the target (same directory) so the rename is always on the same filesystem.

**`sync_data()` before rename:** Call `file.sync_data()` on the tmp file before renaming. Without this, a kernel crash between write and rename can leave `~/.claude.json` as a zero-byte file.

On cleanup (session exit): re-acquire flock → re-read ref count under lock (never trust in-memory value) → decrement → if zero: remove jig-owned entries → `sync_data()` → atomic write → release.

**Backup files:** Use session-unique names (`~/.claude.json.jig-backup-<pid>`). Write backup atomically (tmp+rename within flock). Clean up own backup on successful exit; leave on error for `jig doctor` recovery.

Recovery: `jig doctor` detects orphaned entries by checking ref count files against running processes. Can restore `~/.claude.json` from `.jig-backup-<pid>`.

### 3.4 MCP Conflict Resolution

**Decision (2026-03-25 brainstorm §1):** When two jig sessions in the same working directory define an MCP server with the same name, the second session namespaces its entry with a session suffix. Both sessions get full MCP config; neither silently overwrites the other.

**Session suffix:** 8 hex characters generated from `getrandom` (e.g., `jig_a3f1b2c9`). Generated inside the flock, checking existing `mcpServers` keys for collisions. Retry up to 32 times; hard error if exhausted. Stored alongside PID in the ref count file.

**Permission rewriting (critical):** Claude Code names MCP tools as `mcp__<server-name>__<tool-name>`. When a server is renamed (e.g., `postgres` → `postgres__jig_a3f1b2c9`), the assembly pipeline MUST rewrite all `allowedTools`/`disallowedTools` entries that reference the original server name — including glob patterns (`mcp__postgres__*`). The rewrite runs immediately after conflict detection, before building any permission flags.

**Why namespace (not last-wins or first-wins):**
- "Last writer wins" silently breaks session 1 mid-flight
- "First writer wins" requires a blocking UX (error/refuse to launch)
- Namespacing gives both sessions full functionality with zero breakage

### 3.5 Ref Count Correctness

The ref count increment MUST happen while the `~/.claude.json` flock is held. If done outside the flock, two concurrent sessions can both read `count=0` and one's cleanup will delete the other's MCP entries. The critical section covers: read → conflict-detect → merge → write-rename → refcount-write → unlock.

CWD hash for ref count files uses `std::fs::canonicalize()` to handle symlinks and mount points. The canonical path is stored as a human-readable field inside the ref count file for `jig doctor` display.

### 3.6 `--resume` Behavior

Claude Code reloads CLAUDE.md, skills, MCP, and extensions fresh on resume. `jig --resume` MUST re-stage the temp dir and re-apply all config (MCP to `~/.claude.json`, CLI flags) before passing `--resume` to claude. Uses the most recent complete entry from `history.jsonl` for config (same as `--last`). Combining `--resume` with `-t T` is an error.

---

## 4. Concurrency

### 4.1 Recommended: Worktrees for Isolation

Claude Code itself has known concurrency issues with same-directory sessions (shell snapshot conflicts, shared project state). jig recommends git worktrees (via lazyworktree) for fully isolated concurrent sessions.

### 4.2 Same-Directory Sessions

When multiple jig sessions run from the same directory:

**Per-process (safe, no collision):** system prompt (`--append-system-prompt-file`), skills (`--add-dir`), plugins (`--plugin-dir`), permissions (`--allowedTools`/`--disallowedTools`), model (`--model`), env vars — all CLI flags with separate temp dirs per session.

**Shared (via `~/.claude.json`):** MCP servers. Written under the same project path key. Concurrent sessions see the union of all active sessions' servers, with conflict namespacing. Ref counted — last session to exit cleans up.

### 4.3 Startup Warnings

```
⚠ You're not in a git worktree. For isolated concurrent sessions,
  consider using worktrees (e.g., lazyworktree). Concurrent jig
  sessions in the same directory will share MCP server configuration.

⚠ Another jig session is active in this directory.
  MCP servers will be shared between sessions.
  Permissions and system prompts remain independent.
  For full isolation, use a separate worktree.
```

---

## 5. Config Schema

### 5.1 Full Schema

```yaml
schema: 1

# ─── Template: The Toolbox ──────────────────────────────

profile:
  # Skill source shorthand (references named sources from global config)
  skills:
    from_source:
      composio: [docker, kubernetes, terraform]
      my-skills: [soc2-checklist]
    local:                             # Project-local skills
      - .jig/skills/api-testing

  # Verbose skill declarations (with optional integrity pinning)
  skills_explicit:
    - name: docker
      source: https://github.com/ComposioHQ/awesome-claude-skills
      path: skills/docker
      sha256: abc123...                # Optional integrity pin for team configs

  # Plugins (Claude Code plugin format)
  plugins:
    - name: formatter
      marketplace: claude-plugins-official
    - name: local-plugin
      source: local
      path: .jig/plugins/my-plugin

  # MCP servers
  mcp:
    mode: layer                        # "layer" (add to existing) or "replace"
    servers: [github, postgres]        # Named servers from user config
    definitions:                       # Inline server definitions
      project-db:
        command: npx
        args: ["-y", "@modelcontextprotocol/server-postgres", "${DATABASE_URL}"]

  # Permissions and settings
  settings:
    model: claude-sonnet-4-20250514
    allowedTools:
      - "Bash(pnpm *)"
      - "Bash(turbo *)"
    deny:
      - "Read(./.env)"

  # Additional directories Claude can access
  mount:
    - ~/infrastructure/shared-modules
    - ../api-contracts

  # Environment variables
  env:
    AWS_PROFILE: production

  # Raw Claude CLI flag passthrough (allowlisted flags only — jig-managed flags blocked)
  claude_flags:
    - "--max-tokens 8192"

# ─── Persona: The Mindset ───────────────────────────────

persona:
  name: code-review
  # Inline rules (injected via --append-system-prompt-file)
  rules: |
    You are in code review mode.
    Be critical and thorough. Flag every potential issue.
    Always check for: security vulnerabilities, performance
    regressions, missing error handling, test coverage gaps.

  # OR reference a persona file:
  # file: code-review.md

  # OR reference a global persona by name:
  # ref: code-review

  # extends: <name>  — only valid in .jig.local.yaml (see §5.3)

# ─── Context: The Briefing ──────────────────────────────

context:
  # Fragments composed in order (priority number prefix controls ordering)
  fragments:
    - 10-base.md                       # From ~/.config/jig/fragments/
    - 20-code-standards.md
    - .jig/fragments/project.md        # Project-specific (relative path)

  # Inline context
  inline: |
    This is a Turborepo monorepo using pnpm.
    Packages live in packages/ with shared configs in packages/config.

# ─── Hooks ──────────────────────────────────────────────

hooks:
  pre_launch:
    - exec: ["python", "scripts/pull-analytics.py"]     # direct exec (default, safe)
      description: "Refresh analytics context"
    - exec: ["./scripts/setup.sh"]
    # shell: true required for shell syntax — explicit opt-in
    - command: "echo session starting && date"
      shell: true
  post_exit:
    - exec: ["python", "scripts/log-session.py"]

# ─── Inheritance ────────────────────────────────────────

extends: [base-devops, code-review]    # Array: merged left to right

# ─── Token Budget ───────────────────────────────────────

token_budget:
  warn_threshold: 4000                 # Warn if injected context exceeds this
```

### 5.2 MCP Credential Pattern

Committed `.jig.yaml` uses `${ENV_VAR}` expansion (Claude Code's native pattern). Actual credential values go in `.jig.local.yaml` (gitignored):

```yaml
# .jig.yaml (committed — safe, uses env var placeholders)
profile:
  mcp:
    definitions:
      project-db:
        command: npx
        args: ["-y", "@modelcontextprotocol/server-postgres", "${DATABASE_URL}"]

# .jig.local.yaml (gitignored — actual values)
profile:
  env:
    DATABASE_URL: "postgresql://user:pass@localhost:5432/mydb"
```

jig expands `${VAR}` and `${VAR:-default}` at assembly time when writing to `~/.claude.json`. Expansion uses pre-expansion strings in approval cache hashes and history records — credential values are never stored. If a required var is unset and has no default, jig errors with a clear message.

At approval time for MCP servers containing env var substitutions, jig shows the current masked value: "Note: `${DATABASE_URL}` resolves to `postgresql://user:***@localhost/db` on this machine."

### 5.3 Persona `extends` (Local Override Inheritance)

**Decision (2026-03-25 brainstorm §2):** Persona inheritance via explicit `extends` in `.jig.local.yaml` only.

The base behavior is "last wins entirely" for persona declarations. But a persona in `.jig.local.yaml` can declare `extends: <name>` to inherit the named persona's rules and append additional ones. Conflicts (same rule key) use the extending persona's value.

```yaml
# .jig.local.yaml
persona:
  extends: project     # inherits .jig.yaml persona
  rules:
    - "Always use metric units in measurements"  # appended
```

**Scope constraint:** `persona.extends` is only valid in `.jig.local.yaml`. It is rejected with a hard error in `.jig.yaml` or global config. This prevents team configs from depending on personal global state.

**Why:** Pure "last wins" forces copy-pasting team rules into personal overrides. Full deep-merge everywhere is complex and hard to reason about. Explicit `extends` is opt-in, keeps the simple case simple, and makes inheritance legible.

### 5.4 Hook Execution Model

Hooks support two forms:

```yaml
hooks:
  pre_launch:
    - exec: ["python", "scripts/pull.py"]     # direct exec — no shell, no injection
    - exec: ["./scripts/setup.sh"]
    - command: "echo hello && date"            # requires shell: true
      shell: true                              # explicit opt-in for shell semantics
```

**`exec: []` is the default.** Uses `Command::new(args[0]).args(&args[1..])` — no shell invocation, no injection risk. **`command: string` without `shell: true` is an error** — jig rejects it with a clear message directing users to either use `exec: []` or add `shell: true`. This forces a conscious decision about injection risk.

### 5.5 Custom Content Storage

Users create personas and templates via `jig persona new` or `jig template new`. At creation time, they choose scope:

```bash
jig persona new security-hardened
# Where should this persona be stored?
# [1] Global (~/.config/jig/personas/security-hardened.md)
# [2] Project (.jig/personas/security-hardened.md)
```

Both locations use the same format. Global content is available everywhere; project content is scoped to the repo.

### 5.6 Merge Semantics

| Dimension | Merge strategy |
|-----------|---------------|
| Skills / Plugins | Union (additive). Higher specificity can only add. |
| MCP servers | `layer` unions with existing, `replace` substitutes entirely. |
| Settings (allowedTools, deny) | Union (additive). |
| Env vars | Higher specificity wins per key. |
| Persona | Last one wins entirely — **unless** `extends` declared in `.jig.local.yaml` |
| Context fragments | Union, ordered by priority number then appearance. |
| Hooks | Concatenated (all levels run in order). |
| `extends` array | Merged left to right, then project config merges on top. |

### 5.7 Schema Migration

When jig updates and the schema changes:

```
$ jig --template devops

  This config uses schema v1. jig now uses schema v2.
  Changes: [brief description of what changed]

  Upgrade ~/.config/jig/config.yaml to v2? [Y/n]

  (Original backed up to config.yaml.v1-backup-20260325T100000Z)
```

Migration functions are chained: `v1→v2`, `v2→v3`, etc. Each is a pure function tested with snapshot tests. Backup uses timestamped name for uniqueness.

### 5.8 `claude_flags` Passthrough Allowlist

`profile.settings.claude_flags` passes raw flags to the `claude` binary. Flags that jig manages itself (e.g., `--append-system-prompt-file`, `--add-dir`, `--allowedTools`) are blocked — passing them via `claude_flags` would conflict with jig's own assembled flags. jig maintains an explicit allowlist of permitted passthrough flags; anything not on the allowlist is rejected with a clear error.

---

## 6. Global Config

```yaml
# ~/.config/jig/config.yaml
schema: 1

# jig's own settings
jig:
  default_template: base
  history_retention_days: 30
  cleanup_staged: true
  theme: auto                          # auto | dark | light
  token_warn_threshold: 4000

# Named skill sources (referenced by shorthand in project configs)
sources:
  composio:
    url: https://github.com/ComposioHQ/awesome-claude-skills
    branch: main
    skill_path: skills/
  voltagent:
    url: https://github.com/VoltAgent/awesome-agent-skills
    branch: main
    skill_path: skills/
  alirezarezvani:
    url: https://github.com/alirezarezvani/claude-skills
    branch: main
    skill_path: skills/
  jig-community:
    url: https://github.com/jig-icu/jig
    branch: main
    path: community/skills/

# Default plugin marketplaces
default_marketplaces:
  - claude-plugins-official
  - jig-icu/jig

# Global defaults (same schema as project config)
profile:
  mcp:
    mode: layer
    servers: [github]

persona:
  name: default
  rules: |
    Follow existing code patterns and conventions.
    Prefer explicit over implicit.

context:
  fragments:
    - 10-base.md
```

---

## 7. Directory Layout

```
~/.config/jig/
├── config.yaml                    # Global config + defaults
├── templates/                     # Named templates (extendable)
│   ├── base.yaml
│   ├── base-devops.yaml
│   ├── base-frontend.yaml
│   ├── data-science.yaml
│   ├── technical-writing.yaml
│   ├── code-review.yaml
│   ├── marketing-growth.yaml
│   ├── sales-engineering.yaml
│   └── creative-writing.yaml
├── personas/                      # Reusable persona definitions
│   ├── default.md
│   ├── code-review.md
│   ├── mentoring.md
│   ├── debugging.md
│   ├── greenfield.md
│   ├── strict-security.md
│   ├── editorial.md
│   ├── growth-marketer.md
│   ├── sales-focused.md
│   └── storyteller.md
├── fragments/                     # Reusable context fragments
│   ├── 10-base.md
│   └── 20-code-standards.md
├── skills/                        # Synced skill cache
│   ├── composio/
│   ├── voltagent/
│   ├── alirezarezvani/
│   └── jig-community/
├── overrides/                     # User customizations over synced skills
├── jig.lock                       # Global state tracker (installed versions/hashes)
├── history.jsonl                  # Session launch history (0600 permissions)
└── state/
    ├── locks/                     # Session lock files
    ├── hook-approvals.jsonl       # Cached hook approvals (JSONL, append-only)
    ├── mcp-approvals.jsonl        # Cached MCP approvals (JSONL, append-only)
    └── <cwd-hash>.refcount        # Ref count per project path

<project>/
├── .jig.yaml                      # Project config (committed)
├── .jig.local.yaml                # Personal overrides (gitignored)
├── .jig.lock                      # Project lock (committed, reproducibility)
└── .jig/                          # Project-local extensions
    ├── skills/
    ├── plugins/
    ├── fragments/
    └── personas/
```

---

## 8. Assembly Pipeline

### 8.1 Full Launch Sequence

```
jig [--template T] [--persona P] [flags]
 │
 ├─ 1. DETECT ENVIRONMENT
 │     ├─ Is claude binary available? check $PATH + ~/.claude/local/claude + /usr/local/bin/claude
 │     │   If not found → error with install instructions
 │     │   If found → run claude --version, check against CLAUDE_MIN_VERSION
 │     ├─ Is this a git worktree? (if not → warning)
 │     └─ Is another jig session active in this CWD? (if so → warning + session suffix generation)
 │
 ├─ 2. RESOLVE CONFIG
 │     ├─ Load ~/.config/jig/config.yaml (global)
 │     ├─ Load .jig.yaml (project, if exists)
 │     ├─ Load .jig.local.yaml (personal overrides, if exists)
 │     ├─ Apply CLI overrides (--template, --persona, flags)
 │     └─ Resolve `extends` array (merge templates left to right, detect cycles with DFS)
 │
 ├─ 3. EXPAND
 │     ├─ Expand `from_source` shorthand → full skill paths
 │     └─ Expand ${ENV_VAR} in MCP definitions (error on missing without default)
 │
 ├─ 4. CHECK SCHEMA VERSION
 │     └─ If config schema < current → prompt to auto-migrate with timestamped backup
 │
 ├─ 5. CHECK DEPENDENCIES
 │     ├─ Skills: verify cached in ~/.config/jig/skills/ (if not → offer `jig sync`)
 │     ├─ Plugins: verify in ~/.claude/plugins/installed_plugins.json
 │     └─ If missing → prompt: "Install? [Y/n]"
 │
 ├─ 6. SECURITY APPROVALS
 │     ├─ Hooks: trust-tier evaluation → approval cache check → prompt if needed
 │     └─ MCP: first-run approval for project MCP definitions
 │
 ├─ 7. RUN PRE-LAUNCH HOOKS
 │     └─ On non-zero exit: abort, show stderr via miette diagnostic, exit non-zero
 │        No state written after a hook failure
 │
 ├─ 8. STAGE TEMP DIR (/tmp/jig-XXXXXX/, permissions 0700)
 │     ├─ composed-prompt.md        (persona rules + context fragments, ordered)
 │     ├─ skills/                    (symlinks to resolved skills — path-jailed)
 │     └─ plugins/                   (symlinks to resolved local plugin dirs — path-jailed)
 │
 ├─ 9. WRITE MCP TO ~/.claude.json
 │     └─ (Atomic write protocol — see §3.3 and §3.4)
 │        Refcount increment happens inside flock
 │
 ├─ 10. BUILD CLAUDE COMMAND
 │       claude \
 │         --append-system-prompt-file /tmp/jig-XXXXXX/composed-prompt.md \
 │         --add-dir /tmp/jig-XXXXXX/skills \
 │         --plugin-dir ~/.claude/plugins/cache/.../formatter/1.2.0 \
 │         --plugin-dir /tmp/jig-XXXXXX/plugins/local-plugin \
 │         --allowedTools "Bash(pnpm *),Bash(turbo *)" \
 │         --disallowedTools "Read(./.env)" \
 │         --model claude-sonnet-4-20250514 \
 │         [--resume]
 │
 ├─ 11. EXPORT ENV VARS
 ├─ 12. RECORD SESSION START (history.jsonl — pre-expansion strings only)
 │
 ├─ 13. FORK
 │       ├─ child: setpgid(0, 0), exec claude
 │       └─ Lock guard dropped BEFORE exec (critical — prevents fd inheritance)
 │
 ├─ 14. PARENT: SIGNAL HANDLERS
 │       ├─ SIGINT  → kill(-child_pgid, SIGINT)
 │       ├─ SIGTERM → kill(-child_pgid, SIGTERM)
 │       └─ SIGHUP  → kill(-child_pgid, SIGHUP)
 │
 ├─ 15. PARENT: waitpid(child) with EINTR retry
 │
 └─ 16. CLEANUP (SessionGuard::Drop — runs even on error after step 9):
        Category A (always, including after panic with installed panic hook):
          ├─ Re-acquire flock; re-read refcount under lock
          ├─ If refcount == 0: remove jig MCP entries → sync_data → atomic write
          ├─ Remove /tmp/jig-XXXXXX/ temp dir
          └─ Release lock
        Category B (clean exit only — after waitpid returns normally):
          ├─ Run post_exit hooks
          └─ Append exit record to history.jsonl (separate line, not a mutation)
```

**`process::exit()` is forbidden** after step 9 (once `SessionGuard` is constructed). The only valid exits are normal return (runs `Drop`), `exec` (drop guard first), or panic (panic hook runs Category A cleanup before aborting).

**`panic = "abort"` note:** With `panic = "abort"`, `Drop` is NOT called on panic. The installed panic hook must run Category A cleanup before the process aborts.

**`_exit(127)` on failed exec in child:** After a failed `execvp`, use `libc::_exit(127)` — not `process::exit()`. `process::exit()` would run `atexit` handlers and flush stdio buffers from the child, corrupting the parent's state.

### 8.2 System Prompt Composition

```markdown
<!-- jig persona: code-review -->
You are in code review mode.
Be critical and thorough. Flag every potential issue.
Always check for: security vulnerabilities, performance regressions,
missing error handling, test coverage gaps.
Suggest specific improvements, don't just identify problems.

---

<!-- jig context: 10-base.md -->
You are working in a Turborepo monorepo with pnpm.
Packages live in packages/ with shared configs in packages/config.

<!-- jig context: 20-code-standards.md -->
Always use TypeScript strict mode. Prefer explicit types.
Write tests before implementation (TDD).
```

Injected via `--append-system-prompt-file` — the highest precedence position in Claude's system prompt.

### 8.3 Skill Symlink Path Jail

Before creating any symlink in `/tmp/jig-XXXXXX/`, verify the target is a canonical subdirectory of the expected root:
- Synced skills: must be under `~/.config/jig/skills/<source-name>/`
- Local skills: must be under the directory containing `.jig.yaml`

Use `std::fs::canonicalize()` on each path and check the prefix. Abort with a clear error if escape is detected. The same check applies to `context.fragments`, `profile.plugins.path`, `persona.file`, and any user-supplied path field.

---

## 9. Skill Management

### 9.1 Hybrid Model

jig orchestrates installed Claude Code plugins (via `--plugin-dir`) AND manages standalone skills not in plugin format (via `--add-dir` + git sync).

### 9.2 Sync

```bash
jig sync                           # Pull all git sources, index skills
jig sync composio                  # Sync specific source
jig sync --check                   # Check for updates without pulling
jig sync --frozen                  # Refuse to update (CI mode)
jig sync --verify                  # Verify SHA256 hashes
```

**Security:** `jig sync` always passes `--no-recurse-submodules` to git. Skills are cloned to a temp directory, validated, then atomically renamed into place — a partial/malicious clone never replaces an existing good cache.

**URL validation:** Source URLs must match `https://`, `git://`, or `git@...` format. Validated at config parse time. git operations have a 30-second timeout. On partial sync failure, collect all errors and report at end.

### 9.3 Overrides — Full Copy + Runtime Diff

```bash
jig skill override composio/docker
# Copies to ~/.config/jig/overrides/composio/docker/
# Opens SKILL.md in $EDITOR

jig skill diff composio/docker     # Compare override vs upstream (runtime diff)
jig skill reset composio/docker    # Discard override, revert to upstream
```

The override is a complete copy of SKILL.md that the user edits as a normal markdown file. No patches, no merge tooling.

After `jig sync` updates upstream: "⚠ Upstream skill 'composio/docker' changed. Your override may be outdated. Run `jig skill diff` to review."

---

## 10. Lock Files

### 10.1 Project Lock: `.jig.lock` (committed)

Purpose: **Reproducibility.** Ensures teammates get the same skill/plugin versions.

```yaml
schema: 1
locked_at: 2026-03-25T10:00:00Z
locked_by: jig 1.0.0

skills:
  composio/docker:
    source: https://github.com/ComposioHQ/awesome-claude-skills
    commit: abc123def456
    sha256: a1b2c3d4e5f6...

plugins:
  formatter:
    marketplace: claude-plugins-official
    version: 1.2.0
```

`jig sync` updates the lock. `jig sync --frozen` refuses to update (for CI). When local skill cache hash differs from `.jig.lock` hash, jig reports: "Skill 'composio/docker' is cached but at wrong commit. Run `jig sync` to update." It does not auto-run sync.

### 10.2 Global Lock: `~/.config/jig/jig.lock`

Purpose: **State tracking.** Records what's installed globally.

Same format. Enables:
- `jig sync --check` — "3 skills have updates available"
- `jig doctor` — verify installed skills match recorded hashes
- Update notifications on startup
- Rollback reference if a sync breaks something

---

## 11. Security

### 11.1 Hook Trust Tiers

**Decision (2026-03-25 brainstorm §3):** Four-tier trust model with source-aware prompting.

| Source | Trust Tier | Behavior |
|--------|-----------|----------|
| `~/.config/jig/config.yaml` | Full | Prompt once on first encounter; cache by SHA-256 hash; ownership check required |
| `.jig.yaml` (committed to git) | Team | Prompt with "from team config (committed to git): `<cmd>`" |
| Synced skills | External | Prompt with "from skill `<name>` (source: `<url>`): `<cmd>`" |
| `.jig.local.yaml` | Personal | Prompt once; cache by SHA-256 hash |

**Note:** Global config hooks are NOT unconditionally auto-approved. The threat model "the user wrote it" is insufficient: `jig import` can write to global scope, a compromised process running as the same user can append to the file, and the file may have weak permissions on multiuser systems. First-time global hooks prompt once and cache by hash.

**Global config ownership check:** `~/.config/jig/config.yaml` must be owned by the current user (`getuid()`) and have permissions `0600` or `0640`. Uses `open-then-fstat` (not `stat-then-open`) to prevent TOCTOU races. If ownership check fails, error with a specific `chmod 600` instruction before reading.

**`--yes` flag scope:** `--yes` auto-approves only items already in the approval cache (previously manually approved). Items never approved before fail with an error in `--yes` mode, forcing first-run manual approval. `--yes-team` auto-approves global + team config hooks only. `--yes-all` (explicit, printed warning) auto-approves everything including external skills.

**`jig import` hooks:** `jig import` shows each hook verbatim and requires explicit confirmation before writing to any config scope.

### 11.2 Hook Approval Cache

Stored at `~/.config/jig/state/hook-approvals.jsonl` — JSONL format, append-only.

```
{"command_hash":"sha256:abc...","command":"python scripts/pull.py","source":"skill:docker-tools","approved_at":"2026-03-25T10:00:00Z","last_used_at":"2026-03-25T11:30:00Z"}
```

**Cache key:** `(command_hash, source)`. If the same command hash appears under a different source, prompt again — the source change could indicate a supply-chain substitution.

**Hash input:** SHA-256 of the UTF-8 bytes of the command string exactly as it appears in the config file, before any `${ENV_VAR}` substitution. Pre-expansion strings prevent per-machine cache misses for shared team hooks and prevent credential values from entering the cache.

**TTL:** External tier entries expire after 90 days of inactivity (based on `last_used_at`). Full/Personal entries expire after 1 year. `jig doctor --audit` reports and prunes expired entries.

**Concurrent writes:** JSONL with `O_APPEND`. Single-line appends are atomic for records smaller than `PIPE_BUF` (4096 bytes). Reads reconstruct the approved set by parsing all lines and deduplicating by `(command_hash, source)` using the most recent `last_used_at`.

**Re-approval diff display:** When a hook's hash changes, show a diff against the previously-approved version (the full command string is stored in the approval record, not just the hash).

### 11.3 MCP Approval

On first run of a project config with MCP server definitions, jig prompts with server name and command. Approval cache at `~/.config/jig/state/mcp-approvals.jsonl`. Same JSONL format. Cache key is `(server_name, command_hash, source)`. Re-prompts if the server definition changes or its source changes. Hash is computed on the pre-expansion definition.

### 11.4 Integrity Pinning

`sha256` field on skill declarations. `.jig.lock` records hashes. `jig sync --verify` checks all hashes.

### 11.5 Credential Masking

The following env var name patterns are masked in all output, history, and approval prompts (shown as `***`): `_TOKEN`, `_KEY`, `_SECRET`, `_URL`, `_PASSWORD`, `PGPASSWORD`, `MYSQL_PWD`, `MYSQL_PASSWORD`, `DOCKER_AUTH_CONFIG`, `AWS_SECRET_ACCESS_KEY`, `GCP_CREDENTIALS`, `ANTHROPIC_API_KEY`.

`history.jsonl` is created with `0600` permissions. Pre-expansion strings only — expanded credential values are never recorded.

### 11.6 `jig doctor --audit`

Flags:
- Hooks in project configs (shows actual commands with source tiers)
- Env vars overriding PATH or credential variables
- MCP servers connecting to non-localhost endpoints
- Skills from non-pinned sources in team configs
- Orphaned MCP entries in `~/.claude.json`
- Stale ref count files (cross-checked against running PIDs)
- `~/.claude.json` corruption (offers restore from backup)
- Expired approval cache entries
- Sessions with no matching exit record (crash indicators)

---

## 12. `jig init` — Role-First Design

```
$ jig init

  Welcome to jig — Intentional Context Utilization

  What best describes your work? (select all that apply)
  ❯ ☑ Software development
    ☐ Marketing & growth
    ☐ Sales & presales
    ☐ Creative writing
    ☐ Other

  Detected project: package.json (Node.js), tsconfig.json (TypeScript)

  Suggested template: base-frontend
  Suggested persona: default

  Create ~/.config/jig/config.yaml with these defaults? [Y/n]

  ✓ Created ~/.config/jig/config.yaml
  ✓ Default templates installed (9 templates)
  ✓ Default personas installed (10 personas)
  ✓ Default sources configured (4 skill repos)

  Run `jig` to launch, or `jig init --project` to create a .jig.yaml here.
```

**Requirements:** Complete in < 30 seconds. Auto-detect project type. Role selection drives template suggestion. Never require YAML editing to get started. Default templates and personas are embedded in the binary (via `include_str!`) and available without `jig init`.

---

## 13. `jig import` — Reverse Engineering

Imports existing Claude Code configuration into a `.jig.yaml`:

```
$ jig import

  Scanning existing Claude Code configuration...

  Found:
    CLAUDE.md (project)           → 2 context fragments
    .claude/skills/ (3 skills)    → 3 local skill declarations
    .mcp.json (2 servers)         → 2 MCP definitions
    .claude/settings.json         → permissions (5 allow, 2 deny)
    .claude/settings.json         → 1 hook (PostToolUse)
    ~/.claude.json project MCP    → 1 additional MCP server

  Generate .jig.yaml from this? [Y/n]
```

**Import mapping:**

| Source | Destination |
|--------|------------|
| `./CLAUDE.md` | Split into `context.fragments` (by `##` headers), stored as `.jig/fragments/` |
| `~/.claude/CLAUDE.md` | Offered as global fragment in `~/.config/jig/fragments/` |
| `.claude/skills/*` | `profile.skills.local` entries |
| `.mcp.json` servers | `profile.mcp.definitions` with `${ENV_VAR}` for detected credentials |
| `~/.claude.json` project MCP | Merged into definitions |
| `.claude/settings.json` allow/deny | `profile.settings.allowedTools` / `deny` |
| `.claude/settings.json` hooks | `hooks` section (shows verbatim, requires approval) |
| `.claude/settings.json` model | `profile.settings.model` |
| `.claude/settings.json` env | `profile.env` |

**Credential detection:** MCP args are scanned for connection strings, API keys, and tokens. These are replaced with `${DESCRIPTIVE_VAR_NAME}` and a `.jig.local.yaml` is generated with placeholders.

**`jig import --dry-run`** shows what would be generated without writing.

**Never silently drops config.** If something can't be mapped: "Could not import: 'spinnerTipsEnabled' (not a jig concept). Skipping."

---

## 14. Error Handling

Use `miette` for rich diagnostics. Every error includes: what failed, why, and what to do. YAML parse errors include file path, line number, and source snippet with a caret pointing at the offending line.

```
Error: Failed to resolve skill 'docker' from source 'composio'

  × Directory not found: ~/.config/jig/skills/composio/docker

  help: Run `jig sync composio` to fetch skills.
        Verify source URL in ~/.config/jig/config.yaml

  ─── config.yaml:12 ───
  composio:
    url: https://github.com/ComposioHQ/awesome-claude-skills
```

`--help`, `--version`, and `completions` are dispatched before any config I/O — these complete in < 50ms unconditionally.

---

## 15. TUI Design

### 15.1 Technology & Aesthetic

- **Framework:** ratatui 0.29.x + crossterm (feature-gated: `default = ["tui"]`)
- **Headless build:** `cargo install jig-icu --no-default-features`
- **Palette:** 16-color base (must be usable in any terminal), true-color enhancement when `COLORTERM=truecolor` or `COLORTERM=24bit`.
- **Responsive:** See §15.4 for layout breakpoints.
- **Input:** Vim keybindings. Optional mouse (click, scroll). Which-key popup for discoverability.

### 15.2 TUI Trigger Behavior

**Decision (2026-03-25 brainstorm §4):** Always show TUI when running `jig` with no arguments. Skip TUI with flags or subcommands.

```bash
jig                         # always opens TUI
jig -t base-devops          # skips TUI, launches directly
jig --last                  # skips TUI, repeats last session
jig --go                    # skips TUI, uses .jig.yaml defaults
jig --dry-run               # skips TUI, shows resolved command
jig <subcommand>            # skips TUI, runs subcommand
```

**Why:** Consistent mental model: `jig` = TUI, flags = headless. Scripting and CI naturally use flags. Power users who always want headless alias `jig --go`.

### 15.3 Launch Mode Layout

**Decision (2026-03-25 brainstorm §5):** Two-pane split with live scrollable preview.

```
╭─ Session ────────────────╮
│ Template: devops      [G] │
│ Persona:  debugging       │
│ Context:  3 frags (847t)  │
╰───────────────────────────╯
╭─ Templates ─────────╮╭─ Preview ──────────────────────────────────╮
│                      ││                                            │
│ ● base           [G] ││ devops                                     │
│   base-frontend  [G] ││ extends: base                              │
│ ▸ devops         [G] ││                                            │
│   soc2-audit     [G] ││ Skills: docker, kubernetes, terraform      │
│   marketing      [G] ││ Plugins: formatter, devops-tools           │
│                      ││ MCP: github, postgres [layer]              │
│ ── project ──        ││ Permissions: +3 allow, +2 deny             │
│   .jig.yaml      [P] ││ Model: claude-sonnet-4                     │
│                      ││                                            │
│ ── personas ──       ││ Persona: debugging                         │
│   code-review        ││ "Focus on root cause analysis..."          │
│ ▸ debugging          ││                                            │
│   mentoring          ││ Context: 3 fragments (~847 tokens)         │
│                      ││                                            │
│ [last: devops +      ││ ⚠ Not in a worktree                       │
│  debugging]          ││                                            │
╰──────────────────────╯╰────────────────────────────────────────────╯
╭────────────────────────────────────────────────────────────────────╮
│ ↑↓ nav  ⏎ launch  L relaunch last  Tab section  e edit  d dry-run │
│ p preview  s sync  h history  / search  ? help  q quit            │
╰────────────────────────────────────────────────────────────────────╯
```

Templates marked [G]lobal or [P]roject. Composition indicator always visible. Token budget warning at threshold. Worktree/concurrency warnings displayed.

### 15.4 Key Bindings

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate current list |
| `Tab` | Switch focus: Templates ↔ Personas |
| `Ctrl+D` / `Ctrl+U` | Scroll preview pane independently |
| `/` | Enter filter mode (fuzzy, nucleo-matcher) for current list |
| `Enter` | Launch with selected template + persona |
| `d` | Dry-run (show resolved command without launching) |
| `p` | Toggle preview pane (single-pane mode only) |
| `L` | Relaunch last session (headless) |
| `e` | Edit selected config file in `$EDITOR` (Phase 1); Editor Mode (Phase 2+) |
| `s` | Trigger `jig sync` |
| `h` | Open session history view |
| `?` | Which-key popup |
| `q` / `Esc` | Quit |

### 15.5 Responsive Layout

| Terminal width | Layout |
|---------------|--------|
| ≥ 100 cols | Full two-pane with scrollable preview |
| 80–99 cols | Narrower preview, abbreviated labels |
| < 80 cols | Single-pane; preview toggled with `p` |
| < 60 cols | Minimal list-only mode |
| < 40 cols or < 24 rows | Error message shown; TUI not entered |

Terminal size is checked before entering raw mode. If too small, display a clear error and exit cleanly without entering raw mode. Size is also enforced on every draw to handle resize-down.

### 15.6 Preview Pane Depth

**Decision (2026-03-25 brainstorm §6):** Always render the full composed system prompt, scrollable independently. No mode switching required.

Preview renders in order:
1. Token count estimate (prominent, top — labeled `~` for heuristic approximation)
2. Skills list (inline, comma-separated)
3. Permissions summary (tool count)
4. Full system prompt (persona rules + context fragments in assembly order)

The preview updates on every selection change after a 50ms debounce. Token count warning shown when budget threshold is triggered, with per-component breakdown:

```
~4,200 tokens [WARN]
  Persona: strict-security  1,800
  Fragment: code-standards.md  1,400
  Fragment: team-context.md  1,000
```

**Token estimation:** Use `text.len() / 4` (character heuristic, labeled `~`) for interactive use. Reserve tiktoken-rs (if feature-enabled) for `--dry-run --json` output.

**Preview caching:** Cache the rendered preview by content hash. Scroll operations only change the scroll offset; they do not re-render. Use a fast non-cryptographic hash (FxHasher) for content-change detection.

**`PreviewData` type boundary:** The `PreviewData` struct is defined in `jig-core` using stdlib-only types (`Vec<String>` for prompt lines, not `Vec<Line<'static>>`). Conversion to ratatui types happens inside `jig-tui` only. This keeps `jig-core` free of TUI dependencies and preserves the headless build.

### 15.7 Launch Transition Screen

**Decision (2026-03-25 brainstorm §7):** Brief assembly status screen before handing off to Claude Code.

```
Launching jig session...

  ✓ Config resolved
  ✓ Skills staged (3 symlinks)
  ⟳ Writing MCP to ~/.claude.json...
  ✓ MCP written (2 servers)
  ✓ Env vars exported

Forking claude...
```

**Timing rules:**
- Minimum display: 500ms (prevents flicker if assembly is fast)
- Maximum: unbounded — keep showing until done
- Each step transitions `⟳` → `✓` (done) or `✗` (failed)
- On failure: `✗` with error detail and elapsed time; TUI stays up for user to read
- `restore_terminal()` (disable raw mode + leave alternate screen + show cursor) MUST be called before `execv("claude")`. The restore order is: `DisableMouseCapture` → `LeaveAlternateScreen` → `Show cursor` → `disable_raw_mode()`.

### 15.8 Dry-Run Confirmation

Shows full staged directory layout, composed command, token breakdown, CWD mutation status (always "None"), MCP servers with status (including any suffixed names from concurrent sessions).

### 15.9 Editor Mode (Phase 2+)

Section-based editing (skills, plugins, MCP, permissions, persona, context, hooks, flags). Undo stack. Scope selection (global/project) when creating new content. Live preview of composed output.

### 15.10 Session History View

Shows recent launches with template, persona, directory, duration, exit code. Relaunch from history. Yank config. Details view showing full resolved config. Opened with `h` key in TUI.

---

## 16. CLI Interface

```bash
# ─── Launch ─────────────────────────────────────────────
jig                                    # TUI
jig -t T [-p P]                        # Direct (shorthand flags)
jig --template T [--persona P]         # Direct (full flags)
jig --go                               # Headless, use .jig.yaml defaults
jig --last [-p P]                      # Relaunch (optionally swap persona)
jig --session <UUID>                   # Relaunch specific session by UUID
jig --resume                           # Re-stage + claude --resume (uses most recent session)
jig --dry-run [--json]                 # Preview assembly (hooks shown, not run)

# ─── Config ─────────────────────────────────────────────
jig config set <key> <value> [--scope local|project|global]
jig config add <key> <value> [--scope local|project|global]
jig config remove <key> <value> [--scope local|project|global]
jig config show [--explain] [--json]   # Show resolved config (--explain shows provenance)

# ─── Templates ──────────────────────────────────────────
jig template list [--json]
jig template new <name>                # Interactive, scope selection
jig template edit <name>               # Opens in $EDITOR
jig template show <name> [--json]      # Print resolved config
jig template delete <name>
jig template export <name>             # YAML to stdout
jig template import <file|url>

# ─── Personas ───────────────────────────────────────────
jig persona list [--json]
jig persona new <name>                 # Interactive, scope selection
jig persona edit <name>
jig persona show <name> [--json]

# ─── Skills ─────────────────────────────────────────────
jig sync [source] [--check|--frozen|--verify]
jig skill list [--json]
jig skill search <query>
jig skill info <name> [--json]
jig skill override <name>              # Copy to override layer, open in $EDITOR
jig skill diff <name>                  # Show delta vs upstream
jig skill reset <name>                 # Discard override

# ─── Utilities ──────────────────────────────────────────
jig init [--project]                   # Role-first guided setup
jig import [--dry-run]                 # Reverse-engineer from .claude/
jig doctor [--audit]                   # Diagnostics + security review
jig history [--json] [--limit N]       # Session history (joined start+exit objects)
jig diff <config-file> [--json]        # Compare resolved configs
jig completions <shell>                # bash/zsh/fish (< 100ms, returns empty on error)
jig serve --mcp                        # Expose jig as MCP server (Phase 5)
```

**Global flags:** `--json` (machine-readable output), `--verbose / -v` (count: info/debug/trace), `--yes` (cache-only auto-approve), `--non-interactive`, `--dry-run`.

**`--json` as a global flag:** All list and show commands emit newline-delimited JSON or a JSON object when `--json` is passed. No human-readable text mixed in. Applies to: `jig template list`, `jig persona list`, `jig skill list`, `jig history`, `jig template show`, `jig persona show`, `jig skill info`, `jig config show`.

**`jig config set/add/remove`:** Uses dotted path notation (`profile.settings.model`, `profile.skills`, `context.fragments`). `set` handles scalar fields. `add`/`remove` handle array fields. Required for agent-native parity.

**`jig --session <UUID>`:** Replaces `--last-id <N>` — positional index is fragile across concurrent writes. UUID is already in every `history.jsonl` record. `jig history --json` emits the UUID in each record so agents can reference sessions.

**`jig history --json` output:** Emits joined session objects (start + exit records correlated by `session_id`), not raw JSONL lines. Records with no matching exit get `"status": "crash_or_running"`.

---

## 17. Observability

### 17.1 Structured Logging

`tracing` crate in `jig-core`. `tracing-subscriber` initialized in `jig-cli` only (not in the library). All tracing events go to **stderr** — never stdout (stdout is for machine-readable `--json` output).

Verbosity levels via `--verbose / -v` count or `RUST_LOG` env var:
- `-v` = `INFO`
- `-vv` = `DEBUG`
- `-vvv` = `TRACE`

Zero overhead when subscriber is not initialized. Does not affect binary size when the feature is disabled.

### 17.2 Config Resolution Trace

In `-v` mode, trace which layer set each resolved field:
```
[DEBUG] persona: .jig.local.yaml sets extends: project
[DEBUG] persona: merging base from .jig.yaml (name: strict-security, 3 rules)
[DEBUG] mcp: team config adds server "postgres"
[DEBUG] mcp: no conflicts detected
[TRACE] config: .jig.local.yaml layer loaded (file exists)
[TRACE] config: ~/.config/jig/config.yaml skipped (file absent)
```

Also exposed as `resolution_trace` in `--dry-run --json` output and via `jig config show --explain`.

### 17.3 `--dry-run --json` Schema (stable, versioned)

```json
{
  "schema_version": 1,
  "template": "base-devops",
  "persona": "strict-security",
  "claude_args": ["--append-system-prompt-file", "/tmp/jig-xxx/prompt.md", "..."],
  "system_prompt": "Full composed system prompt text",
  "skills": [{"name": "docker", "path": "/tmp/jig-xxx/skills/docker"}],
  "mcp_servers": [{"name": "postgres__jig_a3f1b2c9", "original": "postgres", "suffixed": true}],
  "token_count_estimate": 2400,
  "token_count_method": "heuristic",
  "token_breakdown": {"persona": 800, "fragments": [{"name": "code-standards.md", "tokens": 1400}]},
  "hooks": {
    "pre_launch": [{
      "command": "python scripts/pull.py",
      "source": "team",
      "tier": "team",
      "approval_status": "cached",
      "last_approved_at": "2026-03-24T09:00:00Z"
    }]
  },
  "warnings": [{"code": "CONCURRENT_SESSION", "message": "Another jig session is active in this directory"}],
  "env_vars": ["DATABASE_URL", "OPENAI_KEY"],
  "resolution_trace": {
    "persona.name": "PersonalLocal",
    "mcp.postgres": "TeamProject"
  }
}
```

### 17.4 `history.jsonl` Schema

**Start record (Step 12):**
```json
{"type":"start","session_id":"uuid","started_at":"2026-03-25T10:00:00Z","template":"base-devops","persona":"strict-security","cwd":"/Users/jforsythe/dev/project","mcp_servers":["postgres__jig_a3f1b2c9"],"skills":["docker","k8s"],"concurrent_sessions":[],"worktree":true}
```

**Exit record (Category B cleanup, separate appended line):**
```json
{"type":"exit","session_id":"uuid","exit_code":0,"duration_ms":14523,"ended_at":"2026-03-25T10:14:34Z","jig_version":"1.0.0","token_count_estimate":2400,"token_count_method":"heuristic","fragment_count":3,"hook_results":[{"hook":"pre_launch","command":"python scripts/pull.py","source":"team","exit_code":0,"duration_ms":1200}]}
```

A start record with no matching exit record is a crash indicator. `jig doctor` flags these and can write synthetic exit records to close the gap in history output.

`history.jsonl` is read tail-first for `jig history` to avoid loading the entire file. Retention enforcement runs on startup (after config load): prune entries older than `history_retention_days`, capped at 100ms; skip if file is locked.

---

## 18. Agent-Native Parity

jig is designed for agent-native use. Every action a user can perform in the TUI has a CLI equivalent for agents.

### 18.1 Parity Map

| User Action (TUI) | CLI Equivalent | Status |
|---|---|---|
| Select and launch template + persona | `jig -t T -p P` | Phase 1 |
| Relaunch most recent session | `jig --last` | Phase 1 |
| Relaunch specific history entry | `jig --session <UUID>` | Phase 1 |
| Preview assembled config | `jig --dry-run` | Phase 1 |
| Preview as machine-readable JSON | `jig --dry-run --json` | Phase 1 |
| List available templates | `jig template list --json` | Phase 1 |
| List available personas | `jig persona list --json` | Phase 1 |
| List installed skills | `jig skill list --json` | Phase 1 |
| Edit config scalar fields | `jig config set` | Phase 1 |
| Add/remove array config fields | `jig config add` / `jig config remove` | Phase 1 |
| View session history | `jig history --json` | Phase 1 |
| Query active session config from within | `jig serve --mcp` tools | Phase 5 |
| Modify next-session config from within | `jig serve --mcp` tools | Phase 5 |

### 18.2 MCP Tool Surface (`jig serve --mcp`, Phase 5)

jig exposes itself as an MCP server so Claude Code sessions can query and modify their own configuration:

| Tool | Description |
|---|---|
| `jig_get_active_config` | Return resolved config for the current session |
| `jig_list_templates` | List available templates with metadata |
| `jig_list_personas` | List available personas with metadata |
| `jig_list_skills` | List installed skills with source info |
| `jig_get_template` | Get full details for a named template |
| `jig_get_persona` | Get full details for a named persona |
| `jig_write_config_field` | Set a config field at a given scope |
| `jig_add_config_array_item` | Append to an array config field |
| `jig_remove_config_array_item` | Remove from an array config field |
| `jig_dry_run` | Return assembled session as JSON without launching |
| `jig_get_history` | Return joined session history objects |
| `jig_get_session` | Get details for a specific session by UUID |
| `jig_sync` | Trigger skill source sync |
| `jig_get_capabilities` | Return jig version, installed features, MCP tool list |

**Transport:** stdio (not TCP) — avoids port conflicts on shared machines.

**Write tool safety:** All write tools that modify `~/.claude.json` acquire the same `~/.claude.json.jig.lock` flock used by the launch path.

**Discovery:** When jig launches a session, it injects a `jig-capabilities.md` context fragment describing the available MCP tools and their schemas, enabling Claude Code within the session to discover and use them without out-of-band documentation.

---

## 19. Default Content

### 19.1 Templates (9)

| Template | For | Skills | Persona |
|----------|-----|--------|---------|
| `base` | Minimal defaults | — | default |
| `base-devops` | Infrastructure & CI/CD | docker, kubernetes, terraform | ops-focused |
| `base-frontend` | React/TS dev | typescript-strict, react-patterns, accessibility | frontend-focused |
| `data-science` | ML/data workflows | jupyter, pandas-helper, sql-analyst | analytical |
| `technical-writing` | Docs & content | markdown-linter, docs-style | editorial |
| `code-review` | PR review | — | code-review |
| `marketing-growth` | SEO, copywriting, campaigns | seo-analyst, copywriting, analytics-helper | growth-marketer |
| `sales-engineering` | Demos, POC, RFPs | demo-builder, rfp-assistant | sales-focused |
| `creative-writing` | Fiction, worldbuilding | narrative-craft, worldbuilding, continuity-checker | storyteller |

### 19.2 Personas (10)

| Persona | Key behavior |
|---------|-------------|
| `default` | Follow conventions, prefer explicit over implicit |
| `code-review` | Be critical, flag every issue, suggest improvements |
| `mentoring` | Explain everything, teach concepts, never shortcut |
| `debugging` | Root cause focus, add logging, trace execution |
| `greenfield` | Explore options, suggest architecture, prototype fast |
| `strict-security` | Audit inputs, enforce least-privilege, flag vulnerabilities |
| `editorial` | Clarity, consistency, audience focus |
| `growth-marketer` | Data-driven, conversion-focused, A/B test mindset |
| `sales-focused` | Customer-centric, solution-oriented, ROI framing |
| `storyteller` | Narrative voice, character consistency, show-don't-tell |

### 19.3 Default Sources (4)

| Name | Repository | Content |
|------|-----------|---------|
| composio | ComposioHQ/awesome-claude-skills | Automation-focused skills |
| voltagent | VoltAgent/awesome-agent-skills | Official + community curated skills |
| alirezarezvani | alirezarezvani/claude-skills | 192+ skills, multi-domain |
| jig-community | jig-icu/jig (community/) | jig's own curated content |

---

## 20. Repository Structure

### 20.1 Monorepo (just + Cargo workspace)

```
github.com/jig-icu/jig
├── Cargo.toml                         # Workspace root
├── Cargo.lock
├── LICENSE-MIT
├── LICENSE-APACHE
├── README.md
├── CONTRIBUTING.md
├── CHANGELOG.md
├── justfile
│
├── crates/
│   ├── jig-cli/                       # Binary crate (name = "jig")
│   │   ├── Cargo.toml                 # dep:jig-tui with optional = true, features = ["tui"]
│   │   └── src/
│   │       ├── main.rs                # Entry point, CLI parsing (clap)
│   │       └── cli.rs                 # Command routing
│   │
│   ├── jig-core/                      # Library crate (no TUI deps)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── config/
│   │       │   ├── mod.rs
│   │       │   ├── schema.rs          # Config schema (serde)
│   │       │   ├── resolve.rs         # Merge/inheritance resolution
│   │       │   ├── validate.rs        # Per-layer schema validation
│   │       │   └── migrate.rs         # Schema version migration
│   │       ├── assembly/
│   │       │   ├── mod.rs
│   │       │   ├── prompt.rs          # System prompt composition
│   │       │   ├── skills.rs          # Skill symlinking (path-jailed)
│   │       │   ├── plugins.rs         # Plugin discovery + --plugin-dir
│   │       │   ├── mcp.rs             # ~/.claude.json MCP management
│   │       │   ├── permissions.rs     # --allowedTools/--disallowedTools + rename rewriting
│   │       │   ├── preview.rs         # PreviewData struct (stdlib types only)
│   │       │   └── stage.rs           # Temp dir sequencer (thin, ≤ 200 lines)
│   │       ├── security/
│   │       │   ├── mod.rs
│   │       │   ├── approval.rs        # ApprovalUi trait + approval cache
│   │       │   └── ownership.rs       # Config file ownership checks
│   │       ├── executor.rs            # Fork+wait + signal handling (NOT launch.rs)
│   │       ├── sync/
│   │       │   ├── mod.rs
│   │       │   ├── git.rs             # Git CLI operations (no git2)
│   │       │   ├── index.rs           # Skill indexing from SKILL.md frontmatter
│   │       │   └── overrides.rs       # Override layer
│   │       ├── bootstrap/
│   │       │   ├── mod.rs
│   │       │   └── install.rs         # Dependency checking + install prompts
│   │       ├── import.rs              # Reverse-engineer from .claude/
│   │       ├── lock.rs                # Lock file generation/checking
│   │       ├── history.rs             # Session history (JSONL, tail-first reads)
│   │       └── doctor.rs              # Diagnostics
│   │
│   └── jig-tui/                       # TUI crate (feature-gated, ratatui 0.29.x)
│       ├── Cargo.toml
│       └── src/
│           ├── app.rs                 # App state + event loop
│           ├── launch.rs              # Launch transition screen
│           ├── editor.rs              # Editor Mode (Phase 2+)
│           ├── history.rs             # History view
│           ├── confirm.rs             # Dry-run confirmation
│           ├── widgets/
│           │   ├── filterable_list.rs  # nucleo-matcher fuzzy filter
│           │   ├── checkbox_list.rs
│           │   ├── key_value_editor.rs
│           │   └── markdown_viewer.rs  # pulldown-cmark → ratatui Line<'static>
│           └── theme.rs               # Semantic color scheme (16-color + truecolor)
│
├── community/                          # Non-Rust content
│   ├── skills/
│   │   ├── docker/
│   │   │   └── SKILL.md
│   │   ├── kubernetes/
│   │   ├── terraform/
│   │   ├── typescript-strict/
│   │   ├── react-patterns/
│   │   ├── accessibility/
│   │   ├── sql-analyst/
│   │   ├── seo-analyst/
│   │   ├── copywriting/
│   │   ├── narrative-craft/
│   │   ├── worldbuilding/
│   │   ├── demo-builder/
│   │   ├── rfp-assistant/
│   │   └── ...
│   ├── plugins/
│   │   ├── jig-config-helper/
│   │   └── persona-crafter/
│   ├── templates/
│   ├── personas/
│   └── fragments/
│
├── defaults/                           # Embedded in binary via include_str!
│   ├── templates/
│   ├── personas/
│   └── fragments/
│
├── schema/
│   └── jig-config.schema.json         # JSON Schema for .jig.yaml
│
├── tests/
│   ├── fixtures/                      # Test config files, mock .claude/ dirs
│   ├── bin/                           # Integration test binaries (mock_mcp_writer, etc.)
│   ├── unit/
│   ├── integration/
│   └── e2e/
│
├── .github/
│   ├── workflows/
│   │   ├── ci.yaml                    # Test + clippy + fmt + lint-community + size gate
│   │   ├── release.yaml               # Cross-compile + publish on tag
│   │   └── audit.yaml                 # Weekly cargo audit
│   └── ISSUE_TEMPLATE/
│
├── docs/
│   ├── CONFIGURATION.md
│   ├── TESTING.md
│   └── ARCHITECTURE.md
│
└── install.sh                          # curl installer script
```

**Key naming constraint:** `jig-core/src/assembly/executor.rs` (not `launch.rs`) — the TUI crate has its own `jig-tui/src/launch.rs` for the transition screen. Using `launch.rs` in both crates causes search confusion and naming collisions in documentation.

### 20.2 Cargo.toml Feature Flags

```toml
# crates/jig-cli/Cargo.toml
[dependencies]
jig-tui = { path = "../jig-tui", optional = true }

[features]
default = ["tui"]
tui = ["dep:jig-tui"]        # dep: prefix is REQUIRED for correct --no-default-features exclusion
```

**Release profiles:**
```toml
# workspace root Cargo.toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = "symbols"
panic = "abort"

[profile.release-headless]
inherits = "release"
opt-level = "z"              # minimize binary size for CI size gate
```

**Workspace-level lints:**
```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
unwrap_used = "warn"
```

### 20.3 Key Dependencies

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `clap` (derive) | 4.5.x | CLI argument parsing | Use `#[arg]`/`#[command]` attrs (clap 4 canonical form) |
| `serde` + `serde_json` | latest | Config serialization + claude.json handling | serde_json Value for claude.json roundtrip |
| `figment` | latest | Config file loading + 4-layer merge | **Replaces `serde_yaml`** (archived 2024, YAML 1.1 bugs) |
| `serde-yaml-ng` | latest | Alternative to figment if direct YAML API needed | Community fork of archived serde_yaml |
| `miette` + `thiserror` | 7.x + 2.x | Rich error diagnostics | Use `fancy` feature in binary only; thiserror 2.x for miette 7 compat |
| `tempfile` | 3.10.x | Temp directory management | — |
| `fd-lock` | latest | Cross-platform file locking | **Replaces `fs2`** (abandoned since 2016); guard MUST be dropped before exec |
| `ratatui` + `crossterm` | 0.29.x + 0.27.x | TUI framework (feature-gated) | Pin to 0.29.x — breaking API changes from 0.28; use `frame.area()` not `frame.size()` |
| `nucleo-matcher` | latest | TUI list fuzzy filtering | **Replaces `fuzzy-matcher`** (stagnant); `Matcher` is `!Send+!Sync`, must stay on TUI thread |
| `pulldown-cmark` | 0.12.x | Markdown rendering in preview | Use `simd` feature for large prompts |
| `directories` | latest | XDG directory resolution | **Replaces `dirs`** — `ProjectDirs` gives correct namespaced paths per platform |
| `sha2` | latest | SHA-256 for approval cache hashing | **Use `sha2` (RustCrypto) directly** — not the `sha256` wrapper |
| `getrandom` | latest | Session suffix generation | **Replaces `rand`** — only 4 random bytes needed |
| `signal-hook` | latest | POSIX signal handling (SIGINT/SIGTERM/SIGHUP) | Purpose-built; smaller than `nix` alone |
| `nix` | 0.29.x | `fork()`, `execvp()`, `setpgid()`, `waitpid()`, `killpg()` | POSIX primitives not in `signal-hook` |
| `ureq` | 3.x | HTTP client for `jig sync` | **Replaces `reqwest`** — synchronous, ~0.5MB vs ~3MB; no tokio |
| `scopeguard` | 1.2.x | `defer!` macro for cleanup | No stdlib equivalent |
| `tracing` + `tracing-subscriber` | latest | Structured logging | `tracing` in `jig-core`; subscriber init in `jig-cli` only |
| `rustc-hash` | 2.x | Fast non-crypto hash for TUI preview cache | `FxHasher` for content-change detection only; security hashes stay as `sha2` |
| `libc` | 0.2.x | `_exit(127)` after failed exec in child | Minimal |
| `unicode-width` | 0.1.x | TUI cell-width for CJK characters | Required in `jig-tui` for string truncation |
| `hex` | 0.4.x | Hex-encoding session suffix | Minimal |
| `proptest` | 1.5.x | Property-based testing | `[dev-dependencies]` only |
| `insta` | 1.39.x | Snapshot testing | `[dev-dependencies]` only; `INSTA_UPDATE=no` in CI |
| `criterion` | 0.5.x | Performance benchmarks | `[dev-dependencies]` only; use `.without_plots()` in CI |
| `serial_test` | latest | Serialize tests with `std::env::set_var` | `[dev-dependencies]` only |

**Removed from v0.6.0:**
- `fs2` → replaced by `fd-lock`
- `serde_yaml` → replaced by `figment` or `serde-yaml-ng`
- `dirs` → replaced by `directories`
- `fuzzy-matcher` → replaced by `nucleo-matcher`
- `tiktoken-rs` → feature-gated behind optional `tokens` feature; character heuristic (`text.len() / 4`) is default

**External runtime dependency:** `claude` binary must be installed and on `$PATH` (or at known fallback paths: `~/.claude/local/claude`, `/usr/local/bin/claude`). jig checks at Step 1 and guides installation. Minimum claude version checked at Step 1 via `claude --version`.

### 20.4 justfile

```makefile
default: build

build:
    cargo build

check:
    cargo clippy --all-targets -- -D warnings
    cargo fmt --check

test:
    cargo test --workspace

test-headless:
    cargo test --workspace --no-default-features

lint-community:
    @echo "Validating SKILL.md frontmatter..."
    @find community/skills -name SKILL.md -exec sh -c \
      'head -1 {} | grep -q "^---" || echo "MISSING FRONTMATTER: {}"' \;

bloat:
    cargo bloat --release --no-default-features

gate-phase1: check test test-headless lint-community
    cargo audit
    cargo build --release
    cargo build --profile release-headless --no-default-features
    @echo "=== Binary sizes ==="
    @ls -lh target/release/jig

bench:
    cargo bench --workspace

release version:
    cargo release {{version}} --execute
```

### 20.5 CI Matrix

```yaml
jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, macos-13]
    steps:
      - cargo test --workspace
      - cargo test --workspace --no-default-features
      - cargo clippy --all-targets -- -D warnings
      - cargo fmt --check

  size-gate:
    steps:
      - cargo build --profile release-headless --no-default-features
      - |
        SIZE=$(stat -f%z target/release/jig 2>/dev/null || stat -c%s target/release/jig)
        [ "$SIZE" -lt 5242880 ] || (echo "FAIL: headless binary exceeds 5MB ($SIZE bytes)" && exit 1)

  build:
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu,
                 x86_64-apple-darwin, aarch64-apple-darwin]

  audit:
    steps: [cargo audit]

  community:
    steps: [just lint-community]
```

---

## 21. Development Phases with Test Gates

### Phase 1 — CLI Core (MVP)

**Deliverables:**

*Config system:*
- Config schema v1 with serde/figment, validation, per-layer constraint checks
- Schema migration with confirmation + timestamped backup (chained: `v1→v2→v3`)
- Config resolution: merge global < project < local < CLI (parallel file reads)
- `extends` array support (DFS cycle detection, clear error messages)
- `persona.extends` in `.jig.local.yaml` only (hard error elsewhere)
- `from_source` shorthand expansion to full skill paths
- Env var expansion in MCP definitions (`${VAR}`, `${VAR:-default}`)
- All merge semantics implemented (union skills, last-wins persona, etc.)
- Plugin discovery from `~/.claude/plugins/installed_plugins.json`

*Assembly:*
- System prompt composition (persona rules + context fragments, ordered)
- Token estimation with configurable budget warnings (character heuristic `text.len() / 4`, labeled `~`)
- Skill symlinking via `--add-dir` to staged temp dir (path-jailed)
- Plugin path resolution via `--plugin-dir` (installed cache + local, stacked)
- MCP via `~/.claude.json` (atomic write with fd-lock on dedicated lock file, refcount inside flock, session-unique backup)
- MCP conflict namespacing (8-hex session suffix, permission rewriting including globs)
- Permissions via `--allowedTools` / `--disallowedTools` CLI flags
- Fork+wait with process group signal forwarding (setpgid + killpg; fd-lock guard dropped before exec)
- SessionGuard with Category A/B cleanup distinction; panic hook runs Category A

*Security:*
- Hook trust tiers (4 tiers, source-aware prompting, `ApprovalUi` trait)
- Hook execution model (`exec: []` default, `shell: true` opt-in, `command:` string without `shell: true` is error)
- Hook approval JSONL cache (append-only, last_used_at, TTL eviction)
- MCP first-run approval (same pattern as hooks)
- Global config ownership check (open-then-fstat, 0600/0640 required)
- Credential masking in history and output
- Worktree detection + concurrency warnings
- Skill symlink path jail

*CLI commands:*
- `jig -t T [-p P]`, `jig --go`, `jig --last [-p P]`, `jig --session <UUID>`, `jig --resume`, `jig --dry-run [--json]`
- `jig config set/add/remove/show` (dotted path, `--scope` selection)
- `jig init` (role-first → project detection → template suggestion, < 30s)
- `jig init --project` (generate .jig.yaml with guided setup)
- `jig import [--dry-run]` (full reverse-engineering with credential detection, hook approval)
- `jig doctor [--audit]`
- `jig template list|new|edit|show|delete` with `--json` support
- `jig persona list|new|edit|show` with `--json` support
- `jig history [--json] [--limit N]` (joined start+exit objects, tail-first read)
- `--json` as global flag; `--verbose / -v` as count flag

*Defaults & infrastructure:*
- 9 templates, 10 personas, 4 default sources, starter fragments (embedded via `include_str!`)
- Lock files: project `.jig.lock` + global `~/.config/jig/jig.lock`
- Error handling with miette (file path + line number + source snippet on YAML errors)
- Feature-gated TUI (`default = ["tui"]`, headless without)
- GitHub Releases CI (macOS x86/arm, Linux x86/arm)
- CI headless binary size gate (< 5MB with `release-headless` profile)
- Homebrew tap, curl installer, cargo install/binstall

**Phase 1 Test Gate — ALL must pass before Phase 2:**

*Unit tests:*
- YAML parsing: valid, invalid, missing fields, unknown fields, version mismatch
- Schema migration: v1 configs → auto-migrate → expected v2 (insta snapshots)
- Config merge: proptest property-based (higher wins scalars, union skills, last-wins persona, deterministic)
- `extends` array: single, chain, array of 3, circular detection (direct + indirect), missing base
- `persona.extends` in `.jig.yaml` → hard error; in `.jig.local.yaml` → merges correctly
- `from_source` expansion: correct resolution, missing source, empty list
- Env var expansion: `${VAR}`, `${VAR:-default}`, missing var → error
- Fragment ordering: priority numbers, explicit order, mixed mode, duplicates
- Token estimation: known test strings produce expected ranges (±15%)
- Permission rewriting: exact match, glob `mcp__*__*`, no-op for unrenamed servers
- MCP conflict detection: 2 sessions, 3 sessions, suffix uniqueness
- Ref count: increment, decrement, zero detection, stale detection — **using OS processes, not threads**
- Hook execution model: exec array (safe), shell: true (allowed), command without shell: true (error)
- Trust tier: correct tier assigned per source; same hash different source → re-prompt
- Approval cache TTL: External 90-day, Personal/Full 1-year; expired entries pruned
- Lock file: generation, verification, hash match, hash mismatch
- Plugin discovery: parse installed_plugins.json, missing file, malformed JSON, missing plugin
- Import mapping: each source type produces correct config (insta snapshots)
- Credential detection: connection strings, API keys, tokens → `${VAR}` placeholders

*Integration tests:*
- Assembly pipeline → staged dir layout matches spec (insta snapshot)
- `~/.claude.json` mutation cycle: add MCP → verify present → cleanup → verify removed
- `~/.claude.json` concurrent access: two OS processes flock + write → no corruption (barrier-based)
- `~/.claude.json` atomic write: simulate crash during write → backup is recoverable
- Refcount-inside-flock: two concurrent sessions, refcount never goes below 1 while both active
- Fork+wait with compiled mock claude binary: normal exit (0), error exit (1), SIGINT, SIGTERM
- Signal forwarding: SIGINT to parent → forwarded to child pgid → both exit
- fd-lock guard drop before exec: verify no lock held in child process
- `--allowedTools` with 20-pattern string → arrives intact at mock claude
- `--dry-run --json` → valid JSON matching resolved config, includes `resolution_trace`
- Hook execution: pre_launch runs before claude, post_exit runs after
- Hook approval: first run prompts, subsequent runs skip if hash unchanged, re-prompts on change
- Hook re-approval diff: changed hook shows diff vs previous command
- MCP approval: same caching pattern as hooks
- MCP suffix collision: 3 concurrent sessions → all get unique suffixes, all permission entries rewritten
- Wildcard permission entry `mcp__postgres__*` rewritten correctly after server rename
- Skill symlink path traversal: `../../etc/passwd` as skill path → abort before any symlink created
- `jig --last` with corrupted history.jsonl → scans upward, skips invalid lines, clear error
- Schema migration: load v1 config → confirm → timestamped backup created → migrated → verify
- Category A cleanup on panic: MCP entries removed before abort

*End-to-end tests:*
- Full launch cycle with mock claude binary (assemble → launch → cleanup)
- `jig init` (dev role): produces valid config with frontend template
- `jig init` (marketing role): produces valid config with marketing template
- `jig import` from test `.claude/` directory: produces correct `.jig.yaml` + `.jig.local.yaml`
- `jig import --dry-run`: prints output but writes no files
- `jig doctor`: detects missing claude binary, orphaned MCP entries, stale refcount
- Worktree warning: run outside git worktree → warning appears
- YAML fuzz: 1000 malformed inputs → clean miette errors, zero panics

*Quality gates:*
- `cargo clippy --all-targets -- -D warnings` passes
- `cargo fmt --check` passes
- `cargo audit` reports no known vulnerabilities
- `just lint-community` passes (all community content valid)
- CI green on: Linux x86_64, macOS ARM64, macOS x86_64
- Headless binary < 5MB (`release-headless` profile, stripped) — enforced by CI gate
- TUI binary < 10MB (`release` profile, stripped)
- `jig --help` renders in < 50ms
- `jig -t base --dry-run` completes in < 200ms

*Performance benchmarks (criterion):*
- Config resolution: < 10ms
- Assembly pipeline: < 50ms
- Full launch-to-exec: < 200ms (excluding Claude startup)
- Token estimation: < 20ms for 10k-token prompt

### Phase 2 — TUI + Health Checks

- Launch Mode: template/persona list + composed preview + token counts + debounced updates
- Two-pane layout with responsive breakpoints (100+ / 80-99 / <80 / <60 col modes)
- Dry-run confirmation with full command preview
- Editor Mode: section-based editing with undo stack
- Vim keybindings + which-key popup + optional mouse scroll
- Launch transition screen with per-step status and 500ms minimum display
- Theme support (16-color safe, true-color enhancement)
- Session history view with relaunch (`h` key, `L` for instant relaunch)
- MCP server health checks (ping before launch, show status)
- `extends` array composition fully exercised in TUI editor
- `jig diff <config>` for comparing resolved configs
- TUI ratatui-snapshot tests with `TestBackend`

### Phase 3 — Skill Registry + Sync

- `jig sync` from git sources (shell out to git CLI, not git2; `--no-recurse-submodules`)
- Skill indexing from SKILL.md frontmatter
- Full-copy override layer with runtime diff + staleness warnings
- `jig skill search/info/override/diff/reset`
- SHA256 integrity verification
- Lock file update on sync (`jig sync` updates, `--frozen` refuses, `--check` reports)

### Phase 4 — Bootstrapping + Team

- Dependency resolution + install prompts for missing skills/plugins
- Plugin marketplace integration (`claude plugin install`)
- `jig --resume` fully exercised (re-stage + `--resume`)
- `jig template export/import` (share via URL/gist)
- Shell completions (bash/zsh/fish; < 100ms, returns empty on error, CWD-aware)
- Context versioning in history (note fragment changes since last session)
- Structured audit events for SOC2 (config hash, MCP servers in history)

### Phase 5 — Ecosystem

- `jig serve --mcp` — 14-tool MCP server over stdio transport
- jig-config-helper plugin (craft `.jig.yaml` inside Claude)
- persona-crafter plugin (design custom personas interactively)
- Dynamic context injection (git branch, recent commits into fragments)
- CI/CD headless mode for `claude -p` pipelines
- `jig doctor --audit` full security review
- Nix, AUR packages
- jig.dev landing page + 15-second GIF demo
- JSON Schema published to SchemaStore
- `jig template share` (gist URL generation)
- Reserved schema fields for enterprise (signing, lockfile verification)

---

## 22. Distribution

| Channel | Command | Phase |
|---------|---------|-------|
| Homebrew | `brew install jig-icu/tap/jig-icu` | 1 |
| GitHub Releases | Download binary from releases page | 1 |
| curl installer | `curl -fsSL jig.dev/install.sh \| sh` | 1 |
| cargo-binstall | `cargo binstall jig-icu` | 1 |
| cargo install | `cargo install jig-icu` | 1 |
| Nix | `nix-shell -p jig-icu` | 5 |
| AUR | `yay -S jig-icu` | 5 |

---

## 23. Success Metrics

| Metric | Target |
|--------|--------|
| Launch-to-prompt | < 500ms |
| Bootstrap (fresh clone → running session) | < 60s |
| `jig init` completion | < 30s |
| Git cleanliness | Zero CWD files written, ever |
| Config correctness | 100% — staged config matches manual equivalent |
| Binary size (with TUI) | < 10MB |
| Binary size (headless) | < 5MB |
| Team adoption | Voluntary use within 1 week |
| Context savings | 40–60% token reduction for focused sessions |

---

## 24. Resolved Open Questions

All open questions from v0.6.0 are resolved. Key resolutions:

**Q1 — Session suffix scheme:** 8 hex characters from `getrandom`. Generated inside flock, collision-checked. Retry up to 32 times. Stored in ref count file alongside PID.

**Q2 — Pre-launch hook failure:** Abort the launch on non-zero exit. Show stderr via miette. No state written after step 7.

**Q3 — Non-TTY behavior:** `--non-interactive` and scoped `--yes` flags. In non-TTY without `--yes`, auto-deny new approvals and exit with error. `--yes` approves only cached items.

**Q4 — Persona `extends` cycle detection:** DFS visited-set cycle detection. Error includes full cycle path.

**Q5 — MCP approval cache hash:** SHA-256 of pre-expansion command string. Same rule for hook hashes. Masked current value shown at approval time.

**Q6 — Backup file rotation:** Session-unique `~/.claude.json.jig-backup-<pid>` names. Written atomically (tmp+rename within flock). Cleaned on successful exit; left for `jig doctor` on error.

**Q7 — Dependency check placement:** Before TUI opens; missing deps show pre-TUI install prompt.

**Q8 — Empty template list in TUI:** Show "No templates found. Run `jig init` to create one."

**Q9 — `e` key in Phase 1 TUI:** Opens relevant config file in `$EDITOR`. Replaced by Editor Mode in Phase 2.

**Q10 — Terminal height guard:** Check before entering raw mode; error if < 40×24 without entering raw mode.

**Q11 — Post-exit hooks on SIGINT:** Always run post-exit hooks unless jig receives SIGKILL.

**Q12 — `--dry-run` with active concurrent session:** Simulate suffix collision and show suffixed server names in output.

**Q13 — Template `extends` cycle detection:** DFS with grey/white/black visited sets. Same algorithm as persona extends. Missing base is a separate error.

**Q14 — `~/.claude.json` schema drift:** Use `serde_json::Value` roundtrip exclusively.

**Q15 — Concurrent `jig doctor --fix`:** Must acquire flock at start of cleanup and re-validate PID state while holding lock.

**Q16 — Cleanup/new-session race:** Cleanup re-reads refcount under flock before removing entries.

**Q17 — `--last` with corrupted history:** Scan from end, skip invalid lines, error after 50 lines with no valid entry.

**Q18 — Skill sync network errors:** 30s timeout; continue past partial failures; collect and report all errors.

**Q19 — `claude` binary not found:** Search `$PATH` + known fallback paths. Warn if found outside PATH. Check minimum version.

**Q20 — `claude` binary version:** Parse `claude --version` at Step 1 against `CLAUDE_MIN_VERSION` constant.

**Q21 — Multi-machine lock mismatch:** Wrong hash = differentiated error. `jig sync --frozen` fails if cache is behind lock.

**Q22 — Shell completion edge cases:** 100ms hard timeout; return empty on error; CWD-aware.

**Q23 — First-run without global config:** Default content embedded in binary, available without `jig init`.

**Q24 — Corrupted `hook-approvals.jsonl`:** Skip parse failures with a warning; re-prompt on next encounter; do not fail launch.

**Q25 — `--resume` semantics:** Uses most recent complete history entry for config; passes `--resume` to claude; error if combined with `-t`.

**Q26 — Env var expansion security:** Warn on shell metacharacters in expanded values. Never record expanded values. Show masked value at approval.

**Q27 — `jig diff <config>` semantics:** Diffs resolved config of `<other-config-file>` against current CWD resolved config. Structured diff of resolved fields. Supports `--json`.

**Q28 — `jig import <url>` hook trust:** External tier. Source URL stored in imported template for attribution.

**Q29 — `history.jsonl` retention:** Enforced on startup (after config load, before TUI). Capped at 100ms; skip if locked.

**Q30 — TUI filter zero matches:** Show "No results" placeholder. Backspace to empty restores full list immediately. Fuzzy algorithm (nucleo-matcher).

---

## 25. Out of Scope

- **Collaborative/team TUI** — session sharing, pairing features
- **Plugin marketplace browsing in TUI** — install plugins from within jig
- **Session recording/replay** — not a jig concern
- **Custom TUI themes** — 16-color safe + true-color enhancement is sufficient for Phase 1
- **Windows native** — Claude Code on Windows is WSL2-only; WSL2 behaves as Linux
- **Async runtime** — jig uses synchronous I/O throughout; no tokio or async-std
