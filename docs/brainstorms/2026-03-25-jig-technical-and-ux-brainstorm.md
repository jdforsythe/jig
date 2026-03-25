# jig: Technical Decisions & TUI UX Brainstorm

**Date:** 2026-03-25
**PRD Version:** v0.6.0
**Topics:** Unresolved implementation choices + TUI interaction design

---

## What We're Exploring

The jig PRD (v0.6.0) is highly detailed. This brainstorm resolves two categories of open questions not fully specified in the PRD:

1. **Technical edge cases** — MCP conflict handling, persona merge semantics, hook trust tiers
2. **TUI UX specifics** — When the TUI appears, navigation model, preview depth, launch transition

---

## Decisions Made

### Technical Decisions

#### 1. MCP Conflict Resolution (Concurrent Same-Directory Sessions)

**Decision:** Namespace by session when MCP server names conflict.

When two jig sessions launch in the same working directory and both define an MCP server with the same name, the second session renames its entry with a session suffix (e.g., `postgres` → `postgres__jig2`). Both sessions get their full MCP config; neither silently overwrites the other.

**Why this approach:**
- "Last writer wins" could silently break session 1 mid-flight
- "First writer wins" requires a blocking UX (error/refuse to launch)
- Namespacing gives both sessions full functionality with zero breakage
- Fits naturally with the existing ref-counting model (each session tracks its own named entries)

**Implementation note:** The session suffix can be derived from the PID or a short UUID stored in the ref count file. Cleanup on exit only removes the suffixed entries owned by that session.

**Critical: permission rewriting required.** Claude Code names MCP tools as `mcp__<server-name>__<tool-name>`. When a server is namespaced (e.g., `postgres` → `postgres__jig2`), the assembly pipeline must also rewrite any `allowedTools`/`disallowedTools` entries that reference that server:

- `mcp__postgres__query` → `mcp__postgres__jig2__query`
- Wildcard entries like `mcp__postgres__*` must also be rewritten

This rewrite happens after the conflict-detection step, before building the `--allowedTools`/`--disallowedTools` flags. The pipeline must:
1. Record which server names received a suffix (the "rename map")
2. Scan all resolved permission entries for `mcp__<original>__` prefixes
3. Rewrite matched entries using the rename map

Glob/wildcard patterns in permissions (`mcp__postgres__*`) need pattern-aware matching, not naive string replacement.

---

#### 2. Persona Merge Semantics Across Config Layers

**Decision:** Support explicit `extends` syntax for persona inheritance in `.jig.local.yaml`.

The base behavior remains "last wins entirely" for persona declarations. But a persona can declare `extends: <name>` to inherit a named persona's rules and append additional ones. Conflicts (same rule key) use the extending persona's value.

```yaml
# .jig.local.yaml
persona:
  extends: project     # inherits .jig.yaml persona
  rules:
    - "Always use metric units in measurements"  # appended
```

**Why this approach:**
- Pure "last wins" forces users to copy-paste team rules into their personal override
- Full deep-merge everywhere is complex and hard to reason about
- Explicit `extends` is opt-in, keeps the simple case simple, and makes inheritance legible
- Mirrors how template `extends` arrays already work in the PRD

**Scope:** Only applicable in `.jig.local.yaml` and CLI persona overrides. Project `.jig.yaml` personas cannot `extends` global personas (to prevent team configs from depending on personal global state).

---

#### 3. Hook Trust Tiers

**Decision:** Auto-approve hooks in global config; always prompt for hooks from team/skill configs with source-aware messaging.

| Source | Trust | Behavior |
|--------|-------|----------|
| `~/.config/jig/config.yaml` | Full | Auto-approved (user wrote it) |
| `.jig.yaml` (committed) | Team | Prompt with "from team config (committed to git)" |
| Synced skills | External | Prompt with "from skill: `<skill-name>` (source: `<url>`)" |
| `.jig.local.yaml` | Personal | Prompt once, cache by hash |

**Why this approach:**
- Global config hooks are the user's own shell commands — no approval friction needed
- Team config hooks deserve visibility (someone else committed them)
- Skill hooks are the highest-risk category (external code) — always show provenance
- Reduces approval fatigue for power users who set up their own global hooks

**Implementation note:** The approval cache stores `{ command_hash, source, approved_at }`. If the source changes (same command now comes from a skill rather than team config), prompt again.

---

### TUI Design Decisions

#### 4. TUI Trigger Behavior

**Decision:** Always show the TUI when running `jig` with no arguments. Skip TUI with `jig --go`, `jig -t <template>`, or `jig --last`.

```bash
jig                         # always opens TUI
jig -t base-devops          # skips TUI, launches directly
jig --last                  # skips TUI, repeats last session
jig --go                    # skips TUI, uses .jig.yaml defaults
jig --dry-run               # skips TUI, shows resolved command
```

**Why this approach:**
- Consistent mental model: `jig` = TUI, flags = headless
- Scripting and CI naturally use flags anyway
- Avoids the "sometimes TUI, sometimes not" confusion of conditional triggering
- Power users who always want headless just alias `jig --go`

---

#### 5. TUI Navigation Model

**Decision:** Two-pane split layout with live preview.

```
┌──────────────────┬────────────────────────────┐
│ Templates        │ Preview: base-devops        │
│ ──────────────── │ ─────────────────────────── │
│ > base-devops    │ Skills: docker, k8s, git    │
│   base           │ Persona: strict-security    │
│   base-frontend  │ Context fragments: 3        │
│   data-science   │ Est. tokens: ~2,400         │
│                  │                             │
│ Personas         │ System prompt (scrollable): │
│ ──────────────── │ <!-- jig persona: strict-   │
│ > strict-secu... │ security -->                │
│   default        │ You are a security-focused  │
│                  │ ...                         │
│                  │ [Enter] Launch  [d] Dry-run │
└──────────────────┴────────────────────────────┘
```

**Key bindings:**
- `j/k` — navigate lists
- `Tab` — switch focus between Templates and Personas lists
- `Ctrl+D/U` or `Shift+j/k` — scroll preview pane independently
- `/` — fuzzy filter current list
- `Enter` — launch with selected template + persona
- `d` — dry-run (show resolved command without launching)
- `?` — which-key popup

**Responsive behavior (matches PRD §TUI Design):**
- ≥100 cols: Full two-pane with scrollable preview
- 80–99 cols: Narrower preview, abbreviated labels
- <80 cols: Single-pane, preview on toggle (`p`)
- <60 cols: Minimal list-only mode

---

#### 6. Preview Pane Depth

**Decision:** Always show the full composed system prompt in the right pane, scrollable independently.

The preview pane renders the complete assembled output:
1. Token count estimate (top, prominent)
2. Skills list
3. Permissions summary
4. Full system prompt (persona rules + context fragments, in assembly order)

No mode switching required. Users who want a summary can glance at the top; users who want detail scroll down.

**Why:** Mode switching (tabbed, expand/collapse) adds cognitive overhead. The right pane has space — use it. The token count at the top serves as the natural "summary" anchor.

---

#### 7. Launch Transition

**Decision:** Brief assembly status screen (≈1–2 seconds) before handing off to Claude Code.

```
Launching jig session...

  ✓ Config resolved
  ✓ Skills staged (3 symlinks)
  ⟳ Writing MCP to ~/.claude.json...
  ✓ MCP written (2 servers)
  ✓ Env vars exported

Forking claude...
```

**Why:** The assembly pipeline has meaningful steps (skill symlinking, atomic MCP write, temp dir creation). A brief status display:
- Confirms something is happening (not frozen)
- Makes errors immediately attributable to a specific step
- Disappears as soon as `claude` takes over the terminal (no leftover output)

If assembly completes in <200ms, show for a minimum of 500ms so it doesn't flicker. If it exceeds 2s, keep showing until done.

---

## Open Questions

*None — all questions resolved during brainstorm.*

---

## What's Out of Scope (YAGNI)

- **Collaborative/team TUI** — sharing sessions, pairing features
- **Plugin marketplace browsing in TUI** — install plugins from within jig
- **Session recording/replay** — not a jig concern
- **Custom TUI themes** — 16-color safe + true-color enhancement is sufficient for Phase 1

---

## Next Steps

These decisions are additive to or clarify the existing PRD. They should be incorporated into:

1. The Phase 1 MVP implementation plan (MCP namespacing, persona extends, trust tiers)
2. The Phase 2 TUI implementation plan (layout, keybindings, launch transition)

Ready for `/ss:plan` when implementation planning begins.
