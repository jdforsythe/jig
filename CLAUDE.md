# jig — Claude Code Development Guide

## Testing Requirements

**Run tests after every change. Do not commit without green tests.**

```bash
cargo test -p jig-core   # fast — run after any jig-core change
cargo test               # full workspace — run before every commit
```

Each bug fix must include a regression test that would have caught the bug.
Each new feature must include tests covering the happy path and key edge cases.

---

## Known Gotchas

### MCP / `~/.claude.json`

**Always use direct map navigation — never JSON Pointer — for cwd key lookups.**
`serde_json::Value::pointer()` treats `/` as a path separator per RFC 6901.
Absolute paths like `/private/tmp/jig-test` silently mis-navigate. Use chained
`.get("projects").and_then(|p| p.get(&cwd_key))` everywhere in `mcp.rs`.
_See: `fix(mcp): use direct map navigation instead of JSON Pointer for cwd key lookup`_

**All new MCP entries must get the session suffix — not just conflicting ones.**
`cleanup_entries` identifies jig-written entries by `name.ends_with(&suffix_marker)`.
If any entry is written without the suffix, cleanup can't find it and it leaks
permanently into `~/.claude.json`. The `rename_map` in `write_atomic` must cover
every key in `new_servers`, unconditionally.
_See: `fix(mcp): always suffix written entries so cleanup reliably removes them`_

**Every `Option<T>` field on `McpServer` must have `#[serde(skip_serializing_if = "Option::is_none")]`.**
Claude Code's MCP schema validator rejects entries with explicit `null` fields
(e.g. `"env": null`). Absent optional fields must be omitted, not nulled.
_See: `fix(mcp): skip serializing None fields on McpServer`_

**Never deserialize `~/.claude.json` into a typed struct.**
Always round-trip through `serde_json::Value`. A typed struct silently drops
unknown fields on write-back, corrupting user config when Claude Code adds new
fields to its schema.

### Process Launch / Executor

**Do not call `setpgid` + `tcsetpgrp` in the child after fork.**
After `fork + setpgid(0, 0)`, the child is in a new process group that does not
control the terminal. Any `read()` on stdin causes the kernel to send `SIGTTIN`,
stopping the child silently. `waitpid` then blocks forever — a hang that requires
force-kill to escape. Keep the child in the parent's process group (no `setpgid`
call); it inherits terminal access without any job-control handshake.
Signal forwarding must use `kill(child_pid, sig)` not `killpg` to avoid
hitting jig itself since they share a process group.
_See: `fix(executor): remove setpgid to fix hang on claude launch`_

### Persona Resolution

**`name: <builtin>` in `.jig.yaml` is not just a display label.**
When `persona.name` or `persona.ref` matches a key in `defaults::builtin_personas()`,
the built-in rules are looked up and prepended before any user-provided rules.
`resolve_persona()` in `config/resolve.rs` implements this. Adding a new built-in
persona requires a test that verifies name-matching merges the built-in rules.
_See: fix in `config/resolve.rs::resolve_persona()`_

---

## Architecture Constraints

- `jig-core` must compile without TUI deps — never import ratatui types here
- `~/.claude.json` is always `serde_json::Value` — never a typed struct
- `process::exit()` is forbidden after `SessionGuard` is live — only `exec` or
  normal return are valid exits (exec doesn't run Drop; use the panic hook for cleanup)
- The fd-lock guard on `~/.claude.json.jig.lock` **must** be dropped before
  `fork_and_exec` — if the child inherits the exclusive flock, it blocks all
  concurrent jig instances for the entire claude session duration
