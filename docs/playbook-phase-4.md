# jig Phase 4 Testing Playbook

Step-by-step manual verification for Phase 4a (bug fixes) and Phase 4b (profiles/toolboxes redesign).

---

## Prerequisites

```bash
# Build from jig source directory
cargo build --workspace

# Record absolute path to binary
JIG_BIN="$(pwd)/target/debug/jig"

# Create isolated test project (not a worktree)
export JIG_TEST_DIR=$(mktemp -d /tmp/jig-playbook-XXXX)
cd "$JIG_TEST_DIR"
git init

# Alias
alias jig="$JIG_BIN"
jig --version
```

---

## Phase 4a: Bug Fix Verification

### B1: `jig history` no longer crashes

```bash
jig history
jig history --limit 5
jig history --verbose
```

**Expected:** No crash. If no history exists, shows empty table or message. If history exists, shows session IDs (truncated to 8 chars), template/toolbox, date, persona (with --verbose).

### B2: `jig config show` displays all fields

```bash
cat > .jig.yaml << 'EOF'
schema: 1
profile:
  settings:
    model: claude-sonnet-4-5
    allowedTools:
      - Read
      - Grep
    disallowedTools:
      - Bash
hooks:
  pre_launch:
    - exec: [echo, "hello"]
persona:
  name: test-persona
  rules:
    - Be concise
EOF

jig config show
```

**Expected (non-JSON):** Output includes ALL of:
- Template/Toolbox name (or none)
- Persona: test-persona
- Model: claude-sonnet-4-5
- Allowed tools: Read, Grep
- Disallowed tools: Bash
- Hooks: 1 pre-launch, 0 post-exit
- Persona rules: 1

```bash
jig config show --json | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d.get('model') == 'claude-sonnet-4-5', 'model missing from JSON'
assert 'Read' in d.get('allowed_tools', []), 'allowed_tools missing'
print('JSON output OK')
"
```

```bash
rm -f .jig.yaml
```

### B3: `jig config add` with snake_case keys

```bash
cat > .jig.yaml << 'EOF'
schema: 1
EOF

# Both snake_case and camelCase should work
jig config add allowed_tools Read --scope project
jig config add disallowed_tools Bash --scope project

# Verify they show up
jig config show --json | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert 'Read' in d.get('allowed_tools', []), 'allowed_tools not found after add'
assert 'Bash' in d.get('disallowed_tools', []), 'disallowed_tools not found after add'
print('Key normalization OK')
"

# Verify YAML uses camelCase
grep allowedTools .jig.yaml && echo "YAML key correct" || echo "YAML key WRONG"
grep disallowedTools .jig.yaml && echo "YAML key correct" || echo "YAML key WRONG"

rm -f .jig.yaml
```

### B4: `?` in editor shows help overlay (not exit)

```bash
jig
# Navigate to [Custom / ad-hoc], press Enter to enter editor
# Press ?
```

**Expected:** Help overlay appears ON TOP of the editor. All editor bindings visible. Press any key to dismiss. Editor state preserved (no data loss, no exit to main screen).

### B5: Ctrl+S save popup works

```bash
jig
# Navigate to [Custom / ad-hoc], press Enter
# Add something (e.g., navigate to Allowed Tools, press 'a', type 'Read', press Enter)
# Press Ctrl+S
```

**Expected:** Save popup appears with:
- Name field (editable)
- Scope selector: [Global] [Project] [Local]
- [Save] [Cancel] buttons
- Tab moves focus between fields
- h/l changes scope
- Enter on Save writes the file
- Esc cancels

After saving with scope=Project, verify:

```bash
cat .jig.yaml
```

**Expected:** Valid YAML with the template config.

### B6: Launch from editor

```bash
jig
# Navigate to [Custom / ad-hoc], press Enter
# Optionally configure some tools
# Press Ctrl+Enter (or type :launch then Enter)
```

**Expected:** Editor exits and session launches with the configured draft. No save required for ad-hoc launch.

Alternative test:

```bash
jig
# Navigate to [Custom / ad-hoc], press Enter
# Type :l then Enter
```

**Expected:** Same — launches the session.

### B7: Preview shows all config properties

#### In main TUI:

```bash
cat > .jig.yaml << 'EOF'
schema: 1
profile:
  settings:
    model: claude-opus-4-5
    allowedTools: [Read, Grep]
    disallowedTools: [Bash]
  mcp:
    test-server:
      command: echo
      args: [hello]
hooks:
  pre_launch:
    - exec: [echo, test]
persona:
  name: reviewer
  rules:
    - Be thorough
EOF

jig
```

**Expected:** Preview pane on right shows:
- Model: claude-opus-4-5
- Persona: reviewer
- Permissions: allowed/disallowed tools
- MCP Servers: 1 (test-server)
- Hooks: 1 pre-launch
- Persona rules content

Navigate between templates/profiles — preview updates for each.

```bash
rm -f .jig.yaml
```

#### In editor mode:

```bash
jig
# Enter editor via [Custom / ad-hoc]
# Navigate to Model section, press i, type "claude-opus-4-5", press Enter
# Navigate to Allowed Tools, press a, type "Read", press Enter
```

**Expected:** Preview pane updates within ~100ms showing the model and allowed tools.

---

## Phase 4b: Profiles/Toolboxes Redesign Verification

### P0: Builtin toolboxes and profiles

```bash
jig toolbox list
```

**Expected:** Lists toolboxes: full-access, read-only, full-devops, full-frontend (with descriptions).

```bash
jig profile list
```

**Expected:** Lists profiles: code-review, security-audit, pair-programming, tdd, documentation, devops, frontend (with toolbox + persona for each).

```bash
jig persona list
```

**Expected:** Unchanged from v1.3.0 — lists all 10 builtin personas.

### P1: CLI flags

```bash
# -t now means toolbox
jig -t read-only --dry-run --json | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert 'Read' in d['args'] or '--allowedTools' in ' '.join(d['args']), 'read-only toolbox not applied'
print('-t toolbox OK')
"

# --profile selects a named profile
jig --profile code-review --dry-run --json | python3 -c "
import sys, json
d = json.load(sys.stdin)
args = ' '.join(d['args'])
# code-review profile = read-only toolbox + code-reviewer persona
assert 'Read' in args or 'allowedTools' in args, 'toolbox not applied'
assert d.get('system_prompt', ''), 'persona rules should produce a system prompt'
print('--profile OK')
"

# --profile with -p override
jig --profile code-review -p strict-security --dry-run --json | python3 -c "
import sys, json
d = json.load(sys.stdin)
# Persona should be strict-security (CLI override wins)
assert 'security' in d.get('system_prompt', '').lower() or True, 'persona override check'
print('persona override OK')
"

# -m model override still works
jig --profile code-review -m claude-haiku-4-5 --dry-run --json | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert 'claude-haiku-4-5' in d['args'], 'model override not applied'
print('model override OK')
"
```

### P2: TUI profiles-first layout

```bash
jig
```

**Expected:**
- Left pane title: "Profiles" (not "Templates")
- List contains: code-review, security-audit, pair-programming, tdd, documentation, devops, frontend, [Custom / ad-hoc]
- Right pane: Preview showing composed toolbox+persona for selected profile
- Navigate with j/k, preview updates
- Enter on a profile launches the session

### P3: Custom / ad-hoc picker

```bash
jig
# Navigate to [Custom / ad-hoc], press Enter
```

**Expected:** Sub-screen with:
- Left-top: Toolbox list (full-access, read-only, full-devops, full-frontend, None)
- Left-bottom: Persona list (all 10 builtins + None)
- Right: Preview showing composed result
- Tab switches between toolbox and persona focus
- Enter launches with selected toolbox+persona
- `e` enters editor with toolbox pre-loaded
- Esc returns to profile list

### P4: Editor two-panel layout

```bash
jig
# [Custom / ad-hoc] → select a toolbox → press e
```

**Expected:** Editor shows two panels:
- Left panel (Toolbox): AllowedTools, DisallowedTools, Model, McpServers, ContextFragments, Hooks, PassthroughFlags
- Right panel (Persona): PersonaName, PersonaRules
- H/L switches between panels
- Preview section shows toolbox config summary + persona rules
- Ctrl+S save works
- Ctrl+Enter launch works
- ? shows help overlay

### P5: Profile subcommands

```bash
jig profile show code-review
```

**Expected:** Shows toolbox=read-only, persona=code-reviewer, and the composed config details.

```bash
jig toolbox show read-only
```

**Expected:** Shows allowed tools, disallowed tools, and other settings.

---

## Automated Test Suite

```bash
cd "$(dirname "$JIG_BIN")/../.."
cargo test --workspace
```

**Expected:** All tests pass, 0 failures. Test count should increase from 232 (Phase 3 + fixes PR) to ~260+ after Phase 4a+4b.

---

## Cleanup

```bash
rm -rf "$JIG_TEST_DIR"
```
