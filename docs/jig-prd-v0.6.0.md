# jig: Intentional Context Utilization

## Product Requirements Document v0.6.0

**Date:** 2026-03-25
**License:** Dual MIT/Apache-2.0
**Package:** jig-icu | **Binary:** jig
**Monorepo:** github.com/jig-icu/jig (just + Cargo workspace)
**Platforms:** macOS (x86_64/ARM64), Linux (x86_64/ARM64). No Windows native — Claude Code on Windows is WSL2-only.
**Status:** Implementation Ready — All design decisions locked, all open questions resolved.

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

When a profile references a plugin, jig looks up the `installPath` and passes it as `--plugin-dir`. Multiple `--plugin-dir` flags are supported and additive:

```bash
claude --plugin-dir /path/to/plugin-a --plugin-dir /path/to/plugin-b
```

For plugins NOT yet installed, jig prompts to install via `claude plugin install`.

### 3.3 `~/.claude.json` Safety Protocol

```
1. flock(~/.claude.json.lock)              # fs2 crate, cross-platform
2. Backup: cp ~/.claude.json → ~/.claude.json.jig-backup
3. Read existing JSON
4. Merge jig MCP entries under projects."<abs_cwd>".mcpServers
5. Write to ~/.claude.json.tmp             # Temp file first
6. Atomic rename: .tmp → ~/.claude.json    # POSIX rename is atomic
7. Increment ref count: ~/.config/jig/state/<cwd-hash>.refcount
8. Release lock
```

On cleanup (session exit): decrement ref count. If zero, lock → read → remove jig-added entries → atomic write → unlock.

Recovery: `jig doctor` detects orphaned entries by checking ref count files against running processes. Can restore `~/.claude.json` from `.jig-backup`.

### 3.4 `--resume` Behavior

Claude Code reloads CLAUDE.md, skills, MCP, and extensions fresh on resume. This means `jig --resume` MUST re-stage the temp dir and re-apply all config (MCP to `~/.claude.json`, CLI flags) before passing `--resume` to claude. The resumed session picks up freshly staged config alongside the conversation history.

---

## 4. Concurrency

### 4.1 Recommended: Worktrees for Isolation

Claude Code itself has known concurrency issues with same-directory sessions (shell snapshot conflicts, shared project state). jig recommends git worktrees (via lazyworktree) for fully isolated concurrent sessions.

### 4.2 Same-Directory Sessions

When multiple jig sessions run from the same directory:

**Per-process (safe, no collision):** system prompt (`--append-system-prompt-file`), skills (`--add-dir`), plugins (`--plugin-dir`), permissions (`--allowedTools`/`--disallowedTools`), model (`--model`), env vars — all CLI flags with separate temp dirs per session.

**Shared (via `~/.claude.json`):** MCP servers. Written under the same project path key. Concurrent sessions see the union of all active sessions' servers. Ref counted — last session to exit cleans up.

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

  # Raw Claude CLI flag passthrough
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
    - command: "python scripts/pull-analytics.py > .jig/fragments/analytics.md"
      description: "Refresh analytics context"
  post_exit:
    - command: "python scripts/log-session.py"

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

jig expands `${VAR}` and `${VAR:-default}` at assembly time when writing to `~/.claude.json`. If a required var is unset and has no default, jig errors with a clear message.

### 5.3 Custom Content Storage

Users create personas and templates via `jig persona new` or `jig template new`. At creation time, they choose scope:

```bash
jig persona new security-hardened
# Where should this persona be stored?
# [1] Global (~/.config/jig/personas/security-hardened.md)
# [2] Project (.jig/personas/security-hardened.md)
```

Both locations use the same format. Global content is available everywhere; project content is scoped to the repo.

### 5.4 Merge Semantics

| Dimension | Merge strategy |
|-----------|---------------|
| Skills / Plugins | Union (additive). Higher specificity can only add. |
| MCP servers | `layer` unions with existing, `replace` substitutes entirely. |
| Settings (allowedTools, deny) | Union (additive). |
| Env vars | Higher specificity wins per key. |
| Persona | Last one wins entirely (not merged). |
| Context fragments | Union, ordered by priority number then appearance. |
| Hooks | Concatenated (all levels run in order). |
| `extends` array | Merged left to right, then project config merges on top. |

### 5.5 Schema Migration

When jig updates and the schema changes:

```
$ jig --template devops

  This config uses schema v1. jig now uses schema v2.
  Changes: [brief description of what changed]
  
  Upgrade ~/.config/jig/config.yaml to v2? [Y/n]
  
  (Original backed up to config.yaml.v1-backup)
```

Migration functions are chained: `v1→v2`, `v2→v3`, etc. Each is a pure function tested with snapshot tests.

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
│   │   ├── docker/
│   │   ├── kubernetes/
│   │   └── terraform/
│   ├── voltagent/
│   ├── alirezarezvani/
│   └── jig-community/
├── overrides/                     # User customizations over synced skills
│   └── composio/docker/
│       └── SKILL.md
├── jig.lock                       # Global state tracker (installed versions/hashes)
├── history.jsonl                  # Session launch history
└── state/
    ├── locks/                     # Session lock files
    ├── hook-approvals.json        # Cached hook approvals by command hash
    ├── mcp-approvals.json         # Cached MCP approvals
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
 │     ├─ Is claude binary available? (if not → error with install instructions)
 │     ├─ Is this a git worktree? (if not → warning)
 │     └─ Is another jig session active in this CWD? (if so → warning)
 │
 ├─ 2. RESOLVE CONFIG
 │     ├─ Load ~/.config/jig/config.yaml (global defaults)
 │     ├─ Load .jig.yaml (project, if exists)
 │     ├─ Load .jig.local.yaml (personal overrides, if exists)
 │     ├─ Apply CLI overrides (--template, --persona, flags)
 │     └─ Resolve `extends` array (merge templates left to right, detect cycles)
 │
 ├─ 3. EXPAND
 │     ├─ Expand `from_source` shorthand → full skill paths
 │     └─ Expand ${ENV_VAR} in MCP definitions (error on missing without default)
 │
 ├─ 4. CHECK SCHEMA VERSION
 │     └─ If config schema < current → prompt to auto-migrate with backup
 │
 ├─ 5. CHECK DEPENDENCIES
 │     ├─ Skills: verify cached in ~/.config/jig/skills/ (if not → offer `jig sync`)
 │     ├─ Plugins: verify in ~/.claude/plugins/installed_plugins.json
 │     └─ If missing → prompt: "Install? [Y/n]"
 │
 ├─ 6. SECURITY APPROVALS
 │     ├─ Hooks: first-run approval for project hooks (cached by command hash)
 │     └─ MCP: first-run approval for project MCP definitions
 │
 ├─ 7. RUN PRE-LAUNCH HOOKS
 │
 ├─ 8. STAGE TEMP DIR (/tmp/jig-XXXXXX/, permissions 0700)
 │     ├─ composed-prompt.md        (persona rules + context fragments, ordered)
 │     ├─ skills/                    (symlinks to resolved skills)
 │     └─ plugins/                   (symlinks to resolved local plugin dirs)
 │
 ├─ 9. WRITE MCP TO ~/.claude.json
 │     └─ (Atomic write protocol — see Section 3.3)
 │
 ├─ 10. BUILD CLAUDE COMMAND
 │      claude \
 │        --append-system-prompt-file /tmp/jig-XXXXXX/composed-prompt.md \
 │        --add-dir /tmp/jig-XXXXXX/skills \
 │        --plugin-dir ~/.claude/plugins/cache/.../formatter/1.2.0 \
 │        --plugin-dir /tmp/jig-XXXXXX/plugins/local-plugin \
 │        --allowedTools "Bash(pnpm *),Bash(turbo *)" \
 │        --disallowedTools "Read(./.env)" \
 │        --model claude-sonnet-4-20250514 \
 │        [--resume]
 │
 ├─ 11. EXPORT ENV VARS
 ├─ 12. RECORD SESSION START (history.jsonl)
 │
 ├─ 13. FORK
 │      └─ child: setpgid(0, 0), exec claude
 │
 ├─ 14. PARENT: SIGNAL HANDLERS
 │      ├─ SIGINT  → kill(-child_pgid, SIGINT)
 │      ├─ SIGTERM → kill(-child_pgid, SIGTERM)
 │      └─ SIGHUP  → kill(-child_pgid, SIGHUP)
 │
 ├─ 15. PARENT: waitpid(child)
 │
 └─ 16. CLEANUP (always runs, even on signal):
        ├─ Decrement ref count
        ├─ If ref count == 0: remove MCP entries from ~/.claude.json (atomic)
        ├─ Run post_exit hooks
        ├─ Clean staged temp dir
        ├─ Update history (duration, exit code)
        └─ Exit with child's exit code
```

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

<!-- jig context: .jig/fragments/project.md -->
This project manages hardware-integrated analytics devices.
The API layer uses Express with Prisma. Tests use Vitest.
```

Injected via `--append-system-prompt-file` — the highest precedence position in Claude's system prompt.

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

### 9.3 Overrides — Full Copy + Runtime Diff

When overriding a synced skill, jig copies the entire directory to the override layer:

```bash
jig skill override composio/docker
# Copies to ~/.config/jig/overrides/composio/docker/
# Opens SKILL.md in $EDITOR
```

The override is a complete copy of the SKILL.md that the user edits as a normal markdown file. No patches, no merge tooling.

```bash
jig skill diff composio/docker     # Compare override vs upstream (runtime diff)
jig skill reset composio/docker    # Discard override, revert to upstream
```

After `jig sync` updates upstream: "⚠ Upstream skill 'composio/docker' changed. Your override may be outdated. Run `jig skill diff` to review."

---

## 10. Lock Files

### 10.1 Project Lock: `.jig.lock` (committed)

Purpose: **Reproducibility.** Ensures teammates get the same skill/plugin versions.

```yaml
schema: 1
locked_at: 2026-03-25T10:00:00Z
locked_by: jig 0.1.0

skills:
  composio/docker:
    source: https://github.com/ComposioHQ/awesome-claude-skills
    commit: abc123def456
    sha256: a1b2c3d4e5f6...
  composio/kubernetes:
    source: https://github.com/ComposioHQ/awesome-claude-skills
    commit: abc123def456
    sha256: f6e5d4c3b2a1...

plugins:
  formatter:
    marketplace: claude-plugins-official
    version: 1.2.0
```

`jig sync` updates the lock. `jig sync --frozen` refuses to update (for CI).

### 10.2 Global Lock: `~/.config/jig/jig.lock`

Purpose: **State tracking.** Records what's installed globally.

Same format. Enables:
- `jig sync --check` — "3 skills have updates available"
- `jig doctor` — verify installed skills match recorded hashes
- Update notifications on startup
- Rollback reference if a sync breaks something

---

## 11. Security

### 11.1 Hook Approval

Project `.jig.yaml` files can contain `hooks.pre_launch` with arbitrary commands. On first run:

```
This project config contains pre-launch hooks:
  1. python scripts/pull-analytics.py > .jig/fragments/analytics.md
     "Refresh analytics context"

Run these hooks? [Y/n]
```

Approval cached in `~/.config/jig/state/hook-approvals.json` keyed on command hash. Re-prompted if commands change.

### 11.2 MCP Approval

On first run of a project config with MCP server definitions:

```
This project config defines MCP servers:
  project-db: npx @modelcontextprotocol/server-postgres ${DATABASE_URL}

Approve these MCP servers? [Y/n]
```

### 11.3 Integrity Pinning

`sha256` field on skill declarations. `.jig.lock` records hashes. `jig sync --verify` checks all hashes.

### 11.4 `jig doctor --audit`

Flags:
- Hooks in project configs (shows actual commands)
- Env vars overriding PATH or credential variables
- MCP servers connecting to non-localhost endpoints
- Skills from non-pinned sources in team configs
- Orphaned MCP entries in `~/.claude.json`
- Stale ref count files
- `~/.claude.json` corruption (offers restore from backup)

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

For non-dev users selecting "Marketing & growth":
```
  Suggested template: marketing-growth
  Suggested persona: growth-marketer
  
  This template includes skills for SEO analysis, copywriting,
  campaign planning, and conversion optimization.
```

**Requirements:** Complete in < 30 seconds. Auto-detect project type. Role selection drives template suggestion. Never require YAML editing to get started.

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
| `.claude/settings.json` hooks | `hooks` section |
| `.claude/settings.json` model | `profile.settings.model` |
| `.claude/settings.json` env | `profile.env` |

**Credential detection:** MCP args are scanned for connection strings, API keys, and tokens. These are replaced with `${DESCRIPTIVE_VAR_NAME}` and a `.jig.local.yaml` is generated with placeholders.

**`jig import --dry-run`** shows what would be generated without writing.

**Never silently drops config.** If something can't be mapped: "Could not import: 'spinnerTipsEnabled' (not a jig concept). Skipping."

---

## 14. Error Handling

Use `miette` for rich diagnostics. Every error includes: what failed, why, and what to do. YAML parse errors include file path and line number.

```
Error: Failed to resolve skill 'docker' from source 'composio'

  × Directory not found: ~/.config/jig/skills/composio/docker
  
  help: Run `jig sync composio` to fetch skills.
        Verify source URL in ~/.config/jig/config.yaml
  
  ─── config.yaml:12 ───
  composio:
    url: https://github.com/ComposioHQ/awesome-claude-skills
```

---

## 15. TUI Design

### 15.1 Technology & Aesthetic

- **Framework:** ratatui + crossterm (feature-gated: `default = ["tui"]`)
- **Headless build:** `cargo install jig-icu --no-default-features`
- **Palette:** 16-color base (must be usable), true-color enhancement. Semantic color slots.
- **Responsive:** Min 80x24. ≥100 cols: two panels. 80-99: narrower preview. <80: single panel with Tab. <60: minimal list.
- **Input:** Vim keybindings. Optional mouse (click, scroll). Which-key popup for discoverability.

### 15.2 Launch Mode

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

Composition indicator always visible. Templates marked [G]lobal or [P]roject. `L` for instant relaunch. Token budget warning at threshold. Worktree/concurrency warnings displayed.

### 15.3 Dry-Run Confirmation

Shows full staged directory layout, composed command, token breakdown, CWD mutation status (should always show "None"), and MCP servers with status.

### 15.4 Editor Mode

Section-based editing (skills, plugins, MCP, permissions, persona, context, hooks, flags). Undo stack. Scope selection (global/project) when creating new content. Live preview of composed output.

### 15.5 Session History

Shows recent launches with template, persona, directory, duration. Relaunch from history. Yank config. Details view showing full resolved config.

---

## 16. CLI Interface

```bash
# ─── Launch ─────────────────────────────────────────────
jig                                    # TUI
jig -t T [-p P]                        # Direct (shorthand flags)
jig --template T [--persona P]         # Direct (full flags)
jig --last [-p P]                      # Relaunch (optionally swap persona)
jig --resume                           # Re-stage + claude --resume
jig --dry-run [--json]                 # Preview assembly

# ─── Templates ──────────────────────────────────────────
jig template list
jig template new <name>                # Interactive, scope selection
jig template edit <name>               # Opens in $EDITOR
jig template show <name>               # Print resolved config
jig template delete <name>
jig template export <name>             # YAML to stdout
jig template import <file|url>

# ─── Personas ───────────────────────────────────────────
jig persona list
jig persona new <name>                 # Interactive, scope selection
jig persona edit <name>
jig persona show <name>

# ─── Skills ─────────────────────────────────────────────
jig sync [source] [--check|--frozen|--verify]
jig skill list
jig skill search <query>
jig skill info <name>
jig skill override <name>              # Copy to override layer, open in $EDITOR
jig skill diff <name>                  # Show delta vs upstream
jig skill reset <name>                 # Discard override

# ─── Utilities ──────────────────────────────────────────
jig init [--project]                   # Role-first guided setup
jig import [--dry-run]                 # Reverse-engineer from .claude/
jig doctor [--audit]                   # Diagnostics + security review
jig history                            # Session history
jig diff <config-file>                 # Compare resolved configs
jig completions <shell>                # bash/zsh/fish
```

---

## 17. Default Content

### 17.1 Templates (9)

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

### 17.2 Personas (10)

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

### 17.3 Default Sources (4)

| Name | Repository | Content |
|------|-----------|---------|
| composio | ComposioHQ/awesome-claude-skills | Automation-focused skills |
| voltagent | VoltAgent/awesome-agent-skills | Official + community curated skills |
| alirezarezvani | alirezarezvani/claude-skills | 192+ skills, multi-domain |
| jig-community | jig-icu/jig (community/) | jig's own curated content |

---

## 18. Repository Structure

### 18.1 Monorepo (just + Cargo workspace)

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
│   ├── jig-cli/                       # Binary crate (name = "jig-icu")
│   │   ├── Cargo.toml
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
│   │       │   ├── validate.rs        # Schema validation
│   │       │   └── migrate.rs         # Schema version migration
│   │       ├── assembly/
│   │       │   ├── mod.rs
│   │       │   ├── prompt.rs          # System prompt composition
│   │       │   ├── skills.rs          # Skill symlinking
│   │       │   ├── plugins.rs         # Plugin discovery + --plugin-dir
│   │       │   ├── mcp.rs             # ~/.claude.json MCP management
│   │       │   ├── permissions.rs     # --allowedTools/--disallowedTools
│   │       │   └── stage.rs           # Temp dir orchestrator
│   │       ├── sync/
│   │       │   ├── mod.rs
│   │       │   ├── git.rs             # Git CLI operations
│   │       │   ├── index.rs           # Skill indexing
│   │       │   └── overrides.rs       # Override layer
│   │       ├── bootstrap/
│   │       │   ├── mod.rs
│   │       │   └── install.rs         # Dependency checking + install prompts
│   │       ├── import.rs              # Reverse-engineer from .claude/
│   │       ├── lock.rs                # Lock file generation/checking
│   │       ├── history.rs             # Session history (JSONL)
│   │       ├── doctor.rs              # Diagnostics
│   │       ├── tokens.rs              # Token estimation
│   │       └── launch.rs              # Fork+wait + signal handling
│   │
│   └── jig-tui/                       # TUI crate (feature-gated)
│       ├── Cargo.toml
│       └── src/
│           ├── app.rs                 # App state + event loop
│           ├── launch.rs              # Launch Mode
│           ├── editor.rs              # Editor Mode
│           ├── history.rs             # History view
│           ├── confirm.rs             # Dry-run confirmation
│           ├── widgets/
│           │   ├── filterable_list.rs
│           │   ├── checkbox_list.rs
│           │   ├── key_value_editor.rs
│           │   └── markdown_viewer.rs
│           └── theme.rs               # Semantic color scheme
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
│   │   ├── jig-config-helper/         # Plugin: craft .jig.yaml inside Claude
│   │   │   ├── .claude-plugin/
│   │   │   │   └── plugin.json
│   │   │   ├── skills/
│   │   │   │   └── SKILL.md
│   │   │   └── commands/
│   │   │       └── jig-init.md
│   │   └── persona-crafter/           # Plugin: design custom personas
│   │       ├── .claude-plugin/
│   │       │   └── plugin.json
│   │       └── skills/
│   │           └── SKILL.md
│   ├── templates/                     # Shareable templates
│   ├── personas/                      # Shareable personas
│   └── fragments/                     # Shareable fragments
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
│   ├── unit/
│   ├── integration/
│   └── e2e/
│
├── .github/
│   ├── workflows/
│   │   ├── ci.yaml                    # Test + clippy + fmt + lint-community
│   │   ├── release.yaml               # Cross-compile + publish on tag
│   │   └── audit.yaml                 # Weekly cargo audit
│   └── ISSUE_TEMPLATE/
│       ├── bug_report.md
│       ├── feature_request.md
│       └── template_request.md
│
├── docs/
│   ├── CONFIGURATION.md
│   ├── TESTING.md
│   └── ARCHITECTURE.md
│
└── install.sh                          # curl installer script
```

### 18.2 Key Dependencies

| Crate | Purpose |
|-------|---------|
| ratatui + crossterm | TUI (feature-gated) |
| clap (derive) | CLI argument parsing |
| serde + serde_yaml + serde_json | Config serialization |
| miette + thiserror | Error handling with rich diagnostics |
| tempfile | Temp directory management |
| fs2 | Cross-platform file locking (flock) |
| fuzzy-matcher | Fuzzy search in skill browser |
| dirs | XDG directory resolution |
| pulldown-cmark | Markdown preview rendering |
| tiktoken-rs | Token count estimation |
| proptest | Property-based testing |
| insta | Snapshot testing |
| criterion | Performance benchmarks |

### 18.3 justfile

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
    @echo "Validating template YAML..."
    @find community/templates -name "*.yaml" -exec sh -c \
      'python3 -c "import yaml; yaml.safe_load(open(\"{}\"))" 2>&1 || echo "INVALID: {}"' \;
    @echo "Validating personas are non-empty..."
    @find community/personas -name "*.md" -empty -exec echo "EMPTY: {}" \;

gate-phase1: check test test-headless lint-community
    cargo audit
    cargo build --release
    cargo build --release --no-default-features
    @echo "=== Binary sizes ==="
    @ls -lh target/release/jig-icu

bench:
    cargo bench --workspace

release version:
    cargo release {{version}} --execute
```

### 18.4 CI Matrix

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

## 19. Development Phases with Test Gates

### Phase 1 — CLI Core (MVP)

**Deliverables:**

*Config system:*
- Config schema v1 with serde, validation, `schema: 1` field
- Schema migration with confirmation + backup (chained: `v1→v2→v3`)
- Config resolution: merge global < project < local < CLI
- `extends` array support (merge left to right, cycle detection)
- `from_source` shorthand expansion to full skill paths
- Env var expansion in MCP definitions (`${VAR}`, `${VAR:-default}`)
- All merge semantics implemented (union skills, last-wins persona, etc.)
- Plugin discovery from `~/.claude/plugins/installed_plugins.json`

*Assembly:*
- System prompt composition (persona rules + context fragments, ordered)
- Token estimation with configurable budget warnings (tiktoken-rs)
- Skill symlinking via `--add-dir` to staged temp dir
- Plugin path resolution via `--plugin-dir` (installed cache + local, stacked)
- MCP via `~/.claude.json` (atomic write via fs2 flock, ref count, backup)
- Permissions via `--allowedTools` / `--disallowedTools` CLI flags
- Fork+wait with process group signal forwarding (setpgid + kill to pgid)

*Security:*
- Hook first-run approval (cached by command hash, re-prompt on change)
- MCP first-run approval (cached, re-prompt on change)
- Worktree detection + concurrency warnings

*CLI commands:*
- `jig -t T [-p P]`, `jig --last [-p P]`, `jig --resume`, `jig --dry-run [--json]`
- `jig init` (role-first → project detection → template suggestion, < 30s)
- `jig init --project` (generate .jig.yaml with guided setup)
- `jig import [--dry-run]` (full reverse-engineering with credential detection)
- `jig doctor [--audit]`
- `jig template list|new|edit|show|delete`
- `jig persona list|new|edit|show` (with scope selection: global/project)
- Session history (JSONL)

*Defaults & infrastructure:*
- 9 templates, 10 personas, 4 default sources, starter fragments
- Lock files: project `.jig.lock` + global `~/.config/jig/jig.lock`
- Error handling with miette (file path + line number on YAML errors)
- Feature-gated TUI (`default = ["tui"]`, headless without)
- GitHub Releases CI (macOS x86/arm, Linux x86/arm)
- Homebrew tap, curl installer, cargo install/binstall

**Phase 1 Test Gate — ALL must pass before Phase 2:**

*Unit tests:*
- YAML parsing: valid, invalid, missing fields, unknown fields, version mismatch
- Schema migration: v1 configs → auto-migrate → expected v2 (insta snapshots)
- Config merge: PICT pairwise (4 factors × present/absent, ~16 combinations)
- Config merge: proptest property-based (higher wins scalars, union skills, last-wins persona, deterministic)
- `extends` array: single, chain, array of 3, circular detection, missing base
- `from_source` expansion: correct resolution, missing source, empty list
- Env var expansion: `${VAR}`, `${VAR:-default}`, missing var → error, escaped `$$`
- Fragment ordering: priority numbers, explicit order, mixed mode, duplicates
- Token estimation: known test strings produce expected ranges (±10%)
- Persona resolution: inline rules, file ref, global ref, missing ref
- Ref count: increment, decrement, zero detection, stale detection, concurrent access
- Lock file: generation, verification, hash match, hash mismatch
- Plugin discovery: parse installed_plugins.json, missing file, malformed JSON, missing plugin
- Import mapping: each source type produces correct config (insta snapshots)
- Credential detection: connection strings, API keys, tokens → `${VAR}` placeholders

*Integration tests:*
- Assembly pipeline → staged dir layout matches spec (insta snapshot)
- `~/.claude.json` mutation cycle: add MCP → verify present → cleanup → verify removed
- `~/.claude.json` concurrent access: two processes flock + write → no corruption
- `~/.claude.json` atomic write: simulate crash during write → backup is recoverable
- Fork+wait with mock claude: normal exit (0), error exit (1), SIGINT, SIGTERM, child crash
- Signal forwarding: SIGINT to parent → forwarded to child pgid → both exit
- `--plugin-dir` stacking: 3 dirs to mock claude → all received
- `--allowedTools` with 20-pattern string → arrives intact at mock claude
- `--dry-run --json` → valid JSON matching resolved config
- Hook execution: pre_launch runs before claude, post_exit runs after
- Hook approval: first run prompts, subsequent runs skip if hash unchanged, re-prompts on change
- MCP approval: same caching pattern as hooks
- Lock file `--frozen`: refuse update with clear error
- `jig --resume`: re-stages config AND passes `--resume` flag to claude
- Schema migration: load v1 config → confirm prompt → backup created → migrated → verify

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
- Binary < 10MB with TUI, < 5MB headless (stripped)
- `jig --help` renders in < 50ms
- `jig -t base --dry-run` completes in < 200ms

*Performance benchmarks (criterion):*
- Config resolution: < 10ms
- Assembly pipeline: < 50ms
- Full launch-to-exec: < 200ms (excluding Claude startup)

### Phase 2 — TUI + Health Checks

- Launch Mode: template/persona list + composed preview + token counts
- Dry-run confirmation with full command preview
- Editor Mode: section-based editing with undo stack
- Responsive layouts (100+ / 80-99 / <80 col breakpoints)
- Vim keybindings + which-key popup + optional mouse
- Theme support (16-color safe, true-color enhancement)
- Session history view with relaunch (`L`)
- MCP server health checks (ping before launch, show status)
- `extends` array composition in TUI editor
- `jig diff <config>` for comparing resolved configs
- Template composition `extends: [a, b]` fully exercised in UI

### Phase 3 — Skill Registry + Sync

- `jig sync` from git sources (shell out to git CLI, not git2)
- Skill indexing from SKILL.md frontmatter
- Full-copy override layer with runtime diff + staleness warnings
- `jig skill search/info/override/diff/reset`
- SHA256 integrity verification
- Lock file update on sync (`jig sync` updates, `--frozen` refuses, `--check` reports)
- Managed settings integration (respect Claude Code deny rules)

### Phase 4 — Bootstrapping + Team

- Dependency resolution + install prompts for missing skills/plugins
- Plugin marketplace integration (`claude plugin install`)
- `jig --resume` fully exercised (re-stage + `--resume`)
- `jig template export/import` (share via URL/gist)
- Shell completions (bash/zsh/fish)
- Context versioning in history (note fragment changes since last session)
- Structured audit events in history for SOC2 (who, config hash, MCP servers)

### Phase 5 — Ecosystem

- jig-config-helper plugin (craft `.jig.yaml` inside Claude)
- persona-crafter plugin (design custom personas interactively)
- Dynamic context injection (git branch, recent commits into fragments)
- CI/CD headless mode for `claude -p` pipelines
- `jig doctor --audit` full security review
- Nix, AUR packages
- jig.dev landing page + 15-second GIF demo
- JSON Schema published to SchemaStore
- CONTRIBUTING.md, issue templates, `good first issue` labels
- `jig template share` (gist URL generation)
- Reserved schema fields for enterprise (signing, lockfile verification)

---

## 20. Distribution

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

## 21. Success Metrics

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
| Context savings | 40-60% token reduction for focused sessions |
