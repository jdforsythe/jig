# jig Phase 1 — Manual Testing Playbook

## Setup: build and put `jig` on PATH

```bash
cd ~/dev/personal/jig   # or wherever your main checkout is
git fetch && git checkout phase-1-mvp
cargo build --release
export PATH="$PWD/target/release:$PATH"
# or: cp target/release/jig ~/.local/bin/jig
```

---

## 1. Smoke tests (no config needed)

```bash
jig --help
jig --version
jig template list
jig persona list
jig template show code-review
jig persona show strict-security
jig doctor
```

---

## 2. Config resolution

```bash
mkdir /tmp/jig-test && cd /tmp/jig-test
jig init          # creates .jig.yaml

# Edit it:
cat > .jig.yaml << 'EOF'
schema: 1
profile:
  settings:
    allowedTools: [Bash, Edit, Read, Glob, Grep]
persona:
  name: minimalist
  rules:
    - No unnecessary abstractions.
    - Show diffs, not explanations.
EOF

jig --dry-run     # should print: claude --allowedTools Bash,Edit,Read,Glob,Grep --append-system-prompt-file ...
jig config show   # should show persona + allowed tools
```

---

## 3. Actual launch

```bash
cd /tmp/jig-test
jig            # opens TUI → pick template+persona → Enter launches claude
# or bypass TUI:
jig --go       # launches with whatever is in .jig.yaml
```

---

## 4. Verify claude got the injected config

Once claude opens, ask at the prompt:

```
What system prompt are you running with? Show me the full contents.
```

Or:

```
List all your allowed tools. Are there any restrictions on what tools you can use?
```

**To inspect the system prompt file before launch**, add `-vv`:

```bash
jig --dry-run -vv 2>&1 | grep system-prompt
# shows the path: --append-system-prompt-file /tmp/jig-XXXXXX/system-prompt.md
cat /tmp/jig-XXXXXX/system-prompt.md
```

> Note: the temp dir is cleaned up on session exit — check it while the session is running.

---

## 5. MCP injection verification

Add an MCP server to `.jig.yaml`:

```yaml
profile:
  mcp:
    test-server:
      type: stdio
      command: npx
      args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
```

Then:

```bash
# Check ~/.claude.json was mutated mid-session:
cat ~/.claude.json | python3 -m json.tool | grep -A 20 '"mcpServers"'

# Inside claude, run:
# /mcp
# or ask: "what MCP servers are available?"
```

---

## 6. Testing with a lazyworktree project

Works as-is — `jig` picks up `.jig.yaml` from `current_dir()`, which is the worktree directory.

```bash
# In a feature branch worktree:
cd ~/dev/myproject-feature-branch
cat > .jig.yaml << 'EOF'
schema: 1
profile:
  settings:
    allowedTools: [Bash, Edit, Read, Write, Glob, Grep]
persona:
  name: pair-programmer
  rules:
    - This is the myproject repo. Main language is TypeScript.
    - Always run tests after changes.
EOF

jig --dry-run    # verify the assembled command
jig              # open TUI, pick persona, launch
```

---

## Known gaps (Phase 2 backlog)

| Gap | Workaround |
|-----|------------|
| `-t <name>` doesn't apply built-in template settings (allowedTools etc.) | Put settings directly in `.jig.yaml` |
| Pre-launch hooks are approved/cached but not run | None yet |
| `jig sync`, `jig import`, `jig diff` are stubs | N/A |
| TUI preview shows static "~0 tokens" | Use `--dry-run` to inspect assembled config |
| `--dry-run --json` outputs text, not JSON | Use `--dry-run` + read the temp file |
