---
title: "feat: Implement jig CLI — Phase 1 Core + Phase 2 TUI"
type: feat
status: phase-1-complete
date: 2026-03-25
deepened: 2026-03-25 (Round 1 + Round 2)
origin: docs/brainstorms/2026-03-25-jig-technical-and-ux-brainstorm.md
phase_1_merged: 2026-03-25 (PR #1)
phase_1_post_merge_fixes:
  - "fix(executor): remove setpgid to fix hang on claude launch — child must stay in parent process group"
  - "fix(mcp): always suffix written entries so cleanup reliably removes them"
  - "fix(mcp): skip serializing None fields on McpServer — Claude Code rejects null fields"
  - "fix(mcp): use direct map navigation instead of JSON Pointer — '/' in cwd paths breaks pointer()"
  - "fix(resolve): merge builtin persona rules when persona name matches a builtin"
---

# feat: Implement jig CLI — Phase 1 Core + Phase 2 TUI

---

## Enhancement Summary

**Deepened on:** 2026-03-25 (Round 1 + Round 2)
**Research agents run:** 24 total — Round 1: architecture, security, performance, data-integrity, observability, spec-flow, testing, dependency-analysis, agent-native, ratatui/crossterm, Rust best-practices, code-simplicity; Round 2: fd-lock/O_CLOEXEC, figment, nucleo-matcher, signal handling, ratatui 0.29, miette 7, security-sentinel, testing patterns, architecture strategist, agent-native parity, observability reviewer, performance oracle

### Top Blocking Issues (resolve before writing Phase 1 code)

1. **`dep:jig-tui` syntax** — `jig-cli/Cargo.toml` must use `dep:jig-tui` (not bare `jig-tui`) for `--no-default-features` to actually exclude the TUI crate.
2. **Replace `serde_yaml`** — dtolnay archived it in early 2024. Replace with `figment` (recommended) or `serde-yaml-ng`. Use raw `serde_json::Value` for `~/.claude.json` roundtrip (never a typed struct) to prevent silent field deletion on schema drift.
3. **Replace `fs2`** — unmaintained since 2016. Replace with `fd-lock`. Also: use a dedicated lock file (`~/.claude.json.jig.lock`) not a lock on the target file itself.
4. **Refcount write inside flock** — the refcount increment MUST happen while the `~/.claude.json` flock is held. If done outside, two concurrent sessions can both read `count=0` and one's cleanup will delete the other's MCP entries.
5. **Rename `jig-core/src/launch.rs` → `executor.rs`** — naming collision with `jig-tui/src/launch.rs` (launch transition screen). Different concerns, confusing name.
6. **`--yes` flag scope** — auto-approving external skill hooks in CI is a supply-chain attack vector. Scope `--yes` to cache-only or previously-approved items. External hooks still require explicit manual approval.
7. **tiktoken-rs binary size** — embeds ~4–6 MB of BPE vocabulary data. Will breach the `< 5 MB` headless binary target without feature-gating.
8. **O_CLOEXEC on lock file fd** — if the `fd-lock` guard is still open when `execv("claude")` is called, the child inherits the exclusive flock for the entire session duration, blocking all other concurrent jig instances. Drop the lock guard **before** the `exec` call. This is the most dangerous silent correctness bug in the implementation.
9. **`ApprovalUi` trait boundary unspecified** — hook approval prompts happen inside `jig-core` but require terminal interaction belonging in `jig-cli`/`jig-tui`. Without an explicit trait, the core crate either depends on TUI types (breaks headless build) or uses `println!` directly (breaks TUI mode). Define `pub trait ApprovalUi: Send { fn prompt_approval(&self, req: &ApprovalRequest) -> ApprovalDecision; }` in `jig-core`, implemented by `TerminalApprovalUi`, `TuiApprovalUi`, and `MockApprovalUi`.
10. **Hook execution model unspecified** — the plan does not specify whether hook `command:` strings run via shell (`sh -c ...`) or direct exec. Use `exec: []` array as the default (direct exec, no injection risk); add `shell: true` as an explicit opt-in for shell scripting capability.
11. **`PreviewData.system_prompt_lines` type must be `Vec<String>`** — the current plan shows `Vec<Line<'static>>` (a ratatui type). `jig-core` must not import ratatui. Convert to `Vec<Line<'static>>` inside `jig-tui` only.

### Key Improvements Added (Round 1)

- **Dependency substitutions** — 6 critical replacements (serde_yaml, fs2, dirs, fuzzy-matcher, sha256, rand/getrandom) with security and maintenance rationale
- **18 new spec gaps** (Q13–Q30) — template extends cycles, claude.json schema drift, concurrent doctor races, history corruption, skill URL validation, env var injection, etc.
- **Observability plan** — `tracing` crate, `--verbose/-v` flag, hook stderr capture, `--dry-run --json` schema, per-step timing, history.jsonl full schema
- **Agent-native parity** — `jig config set`, `--json` global flag, `jig serve --mcp`, `--last-id`, stable `--dry-run --json` schema
- **Security hardenings** — global config ownership check, approval cache JSONL format, credential redaction in history, path jail for skill symlinks, hash scope documentation, diff display on re-approval
- **Performance optimizations** — parallel YAML reads, `OnceLock` for BPE encoder, debounced TUI preview, release profile with LTO+strip+panic=abort, CI size gate
- **ratatui implementation patterns** — panic hook, event loop, StatefulWidget, Paragraph::scroll, popup with Clear, markdown converter, mouse scroll, restore before exec

### Key Improvements Added (Round 2)

- **O_CLOEXEC / fd inheritance** — drop lock guard before exec; prevents claude child inheriting exclusive flock and blocking all concurrent jig instances
- **`ApprovalUi` trait** — explicit core↔CLI boundary for hook approval prompts; enables headless/TUI/mock implementations without crate coupling
- **Hook execution model** — `exec: []` array for direct exec; `shell: true` explicit opt-in; prevents injection while preserving shell scripting capability
- **`PreviewData` type fix** — `system_prompt_lines: Vec<String>`, not `Vec<Line<'static>>`; ratatui types must not cross into `jig-core`
- **`--session <UUID>` replaces `--last-id <N>`** — UUID already in history.jsonl schema; positional index is fragile across concurrent writes
- **MCP tool surface expanded to 14** — `jig_write_config_field` replaces `jig_set_template`; full set covers read, write, list, history, sync
- **`resolution_trace` field** — per-field provenance in `--dry-run --json`; feeds `jig config show --explain`
- **Credential masking expanded** — add `PGPASSWORD`, `MYSQL_PWD`, `DOCKER_AUTH_CONFIG` to the masked env var set
- **Approval cache TTL** — 90 days inactivity for External tier, 1 year for Full/Personal; `last_used_at` field in JSONL records
- **`[profile.release-headless]`** — `opt-level = "z"` for headless CI gate; estimated ~1.5–1.8MB (well under 5MB)
- **`FxHasher`** for preview content cache (security hashes stay SHA-256)
- **`ureq` not `reqwest`** for skill sync HTTP; ~0.5MB vs ~3MB compiled
- **nucleo-matcher lifecycle** — `Matcher` not Send/Sync; scratch `Vec<u32>` must outlive `Utf32Str`; pre-populate at TUI init; empty pattern shows all items
- **git clone security** — `--no-recurse-submodules`; clone-to-temp-then-rename for atomic `jig sync` updates
- **TOCTOU ownership fix** — ownership check must use open-then-fstat, not stat-then-open

### New Considerations Discovered (Round 1)

- `hook-approvals.json` is unsafe for concurrent writes as a JSON file → switch to JSONL append
- history.jsonl exit record should be a separate appended line, not a mutation of the start record
- CWD hash for refcount files must use `canonicalize()` to handle symlinks and mount points
- The Drop guard needs two cleanup categories: always-run vs. clean-exit-only
- `jig --resume` semantics are undefined (which session's config?)
- env var expansion into MCP args creates credential leakage in history.jsonl
- `jig-core/src/tokens.rs` with tiktoken-rs will inflate the headless binary above the 5 MB limit

### New Considerations Discovered (Round 2)

- `fd-lock` guard dropped before `exec` — silent fd inheritance deadlocks all concurrent sessions for the claude session duration
- `process::exit()` after Step 10 is forbidden — `SessionGuard::Drop` will not run; only `exec` or panic-then-abort are valid exits after state is written
- `panic = "abort"` means `Drop` is NOT called on panic — the installed panic hook must run Category A cleanup before aborting
- ratatui 0.29 API breaks: `frame.size()` → `frame.area()`; `Frame<B>` → `Frame`; `Layout::areas()` for compile-time destructuring
- figment `.admerge()` (not `.merge()`) required for array fields to accumulate across layers
- figment `Serialized::globals()` inserts explicit `null` for absent fields — use a manual `Provider` impl for CLI flags
- nucleo-matcher `Matcher` is `!Send + !Sync` — must stay on the TUI thread; cannot be shared
- history.jsonl exit record should include `jig_version`, `token_count_estimate`, `token_count_method`, `fragment_count`
- synthetic exit records needed for crash detection in `jig doctor`
- `jig history --json` must emit joined session objects (start+exit correlated), not raw JSONL lines

---

## Overview

Implement `jig` v1.0.0 — a Rust CLI tool that assembles Claude Code launch configurations from layered YAML configs, skills, personas, and MCP servers, then forks Claude Code with the fully assembled context. This plan covers Phase 1 (core assembly pipeline, config resolution, security model) and Phase 2 (interactive TUI), incorporating all technical decisions resolved in the origin brainstorm.

All technical decisions are finalized. Seven open questions from the PRD were resolved in the brainstorm and are carried forward here with full rationale (see brainstorm: `docs/brainstorms/2026-03-25-jig-technical-and-ux-brainstorm.md`).

---

## Problem Statement

Claude Code users working in large codebases accumulate complex, session-specific launch configurations: MCP servers, tool permissions, system prompt personas, context fragments, and skill-injected files. Today there is no standard way to compose, version, or share these configurations. Each session is configured manually, team patterns cannot be shared declaratively, and personal overrides conflict with project defaults.

`jig` solves this by providing a layered config system — global → project → local → UI template → CLI — that assembles the full Claude Code invocation automatically, with a TUI for interactive selection and a headless path for scripting. TUI/CLI template and persona selections override all file-based layers.

---

## Proposed Solution

`jig` is a thin orchestrator that reads YAML config files, merges them according to explicit precedence rules, stages skills as symlinks, writes MCP server entries into `~/.claude.json`, builds the `claude` CLI invocation, and forks the subprocess. The parent process forwards signals and waits. On exit, it cleans up atomically.

The tool is feature-gated: the headless binary ships without TUI deps (`cargo install jig-icu`) and the default binary includes the TUI.

### High-Level Architecture

```
jig (binary)
├── CLI parsing (clap derive)                     [crates/jig-cli/]
│   ├── bare `jig` → TUI                          [crates/jig-tui/]
│   └── `jig -t T` / `--go` / `--last` → headless
│
├── Config resolution (layered merge)             [crates/jig-core/config/]
│   ├── ~/.config/jig/config.yaml  (global, lowest)
│   ├── .jig.yaml                  (project/team)
│   ├── .jig.local.yaml            (personal)
│   ├── UI template config         (overrides all file layers)
│   └── CLI flags / persona        (highest)
│
├── Assembly pipeline (16 steps)                  [crates/jig-core/assembly/]
│   ├── MCP write → ~/.claude.json (atomic, flock)
│   ├── Skills → /tmp/jig-XXXXXX/ (symlinks)
│   ├── System prompt composition
│   └── Permissions build
│
└── Fork + exec `claude` → parent waitpid
```

---

## Technical Approach

### Pre-Phase: Workspace Bootstrap

Before any feature work, establish the Rust workspace structure exactly as specified in PRD §18.1:

```
jig/
├── Cargo.toml            # workspace root: members = ["crates/jig-cli", "crates/jig-core", "crates/jig-tui"]
├── Cargo.lock
├── justfile              # build, test, lint, check, package, release recipes
├── crates/
│   ├── jig-cli/          # binary crate: entry point, clap CLI routing
│   │   └── src/
│   │       ├── main.rs
│   │       └── cli.rs
│   ├── jig-core/         # library crate: no TUI deps
│   │   └── src/
│   │       ├── config/
│   │       │   ├── schema.rs
│   │       │   ├── resolve.rs
│   │       │   ├── validate.rs
│   │       │   └── migrate.rs
│   │       └── assembly/
│   │           ├── mcp.rs
│   │           ├── permissions.rs
│   │           ├── stage.rs
│   │           ├── prompt.rs
│   │           └── skills.rs
│   └── jig-tui/          # TUI crate (feature-gated)
│       └── src/
│           ├── app.rs
│           ├── launch.rs
│           ├── theme.rs
│           └── widgets/
│               ├── filterable_list.rs
│               ├── markdown_viewer.rs
│               └── key_value_editor.rs
├── docs/
│   ├── jig-prd-v0.6.0.md
│   └── brainstorms/
└── tests/                # integration tests
```

**Key crate constraints:**
- `jig-core` must compile without TUI deps (used in CI headless builds and `cargo install --no-default-features`)
- `jig-tui` depends on `jig-core` and `ratatui`+`crossterm`
- `jig-cli` depends on both; feature flag `default = ["tui"]` controls TUI inclusion
- Binary target in `jig-cli`: `[[bin]] name = "jig" path = "src/main.rs"`

### Research Insights — Workspace Bootstrap

**BLOCKING: `dep:` prefix is required for optional crate exclusion.**
In `jig-cli/Cargo.toml`, the TUI crate must be declared as:
```toml
[dependencies]
jig-tui = { path = "../jig-tui", optional = true }

[features]
default = ["tui"]
tui = ["dep:jig-tui"]
```
Without `dep:jig-tui`, Cargo may not exclude the crate when `--no-default-features` is passed, causing the headless binary to pull in ratatui/crossterm and breach the 5 MB size limit.

**Release profile (add to workspace root `Cargo.toml` before writing any code):**
```toml
[profile.release]
opt-level = 3
lto = "thin"           # thin LTO: good dead-code elimination, faster than "fat"
codegen-units = 1      # required for effective LTO
strip = "symbols"      # saves 1–3 MB per binary
panic = "abort"        # removes unwinding machinery, saves ~100–200 KB
```
Use `lto = "fat"` only for published artifacts (a separate `[profile.release-fat]`).

**CI binary size gate (add to `.github/workflows/ci.yml`):**
```yaml
- name: Check headless binary size
  run: |
    cargo build --release --no-default-features
    SIZE=$(stat -f%z target/release/jig 2>/dev/null || stat -c%s target/release/jig)
    [ "$SIZE" -lt 5242880 ] || (echo "FAIL: headless binary exceeds 5MB ($SIZE bytes)" && exit 1)
```
Gate this from day one so size regressions are caught at the PR that introduces them.

**Rename `jig-core/src/assembly/launch.rs` → `executor.rs` (or `fork.rs`)** before writing any code. The TUI crate has its own `launch.rs` for the transition screen. Two files named `launch.rs` in the same workspace cause search confusion and naming collisions in documentation. Module paths: `jig_core::assembly::executor` and `jig_tui::launch`.

**Workspace-level lints (root `Cargo.toml`):**
```toml
[workspace.lints.rust]
unsafe_code = "forbid"
unused_must_use = "deny"

[workspace.lints.clippy]
all = "warn"
pedantic = "warn"
unwrap_used = "warn"
```

**tiktoken-rs binary size:** This crate embeds ~1.8–4 MB of BPE vocabulary tables. Put it behind an optional `tokens` feature on `jig-core`, or move it to `jig-tui` (where it is consumed). Do not make it an unconditional `jig-core` dependency — it will breach the headless binary size limit. For the default build, use a character-based approximation (`text.len() / 4`) labeled as `~` in the UI.

### Research Insights (Round 2) — Workspace Bootstrap

**Add `[profile.release-headless]` alongside `[profile.release]`.** The standard release profile (LTO+strip) is correct for the TUI binary. For the headless CI size gate, a separate profile with `opt-level = "z"` (minimize binary size, not speed) shrinks the headless binary to ~1.5–1.8MB — well under the 5MB gate. Reference it explicitly in the CI gate command:
```toml
[profile.release-headless]
inherits = "release"
opt-level = "z"
```
```yaml
# CI gate
cargo build --profile release-headless --no-default-features
```
Do NOT use UPX compression — it conflicts with macOS Gatekeeper codesigning and breaks notarization.

**`ureq` instead of `reqwest` for `jig sync` HTTP.** `reqwest` with `tokio` adds ~3MB to the binary and pulls in an async runtime jig does not need elsewhere. `ureq` is a synchronous HTTP client that compiles to ~0.5MB. For skill sync (blocking network I/O run once at user request), synchronous HTTP is correct. Add to `jig-core` only behind the optional `sync` feature flag.

**Run `cargo bloat --release --no-default-features` before committing any new dependency.** Add this as a `just bloat` recipe. The headless binary size budget is 5MB; the TUI binary has no hard constraint but should stay under 10MB. Knowing which crate contributes what before the CI gate fails saves iteration time.

**`#[from]` no longer implies `#[source]` in thiserror 2.x.** If upgrading from thiserror 1.x, all `#[from]` attributes must be paired with explicit `#[source]` if source-chain display is needed. miette 7 requires thiserror 2. Verify all error enums in `jig-core` compile against the 2.x API before writing the first phase of code.

---

### Phase 1: Core CLI (MVP)

#### 1.1 Config Schema + Parsing (`crates/jig-core/config/schema.rs`)

Define the full serde schema matching PRD §4 (lines 214–322). One struct hierarchy covering all four scope layers:

```rust
// crates/jig-core/config/schema.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct JigConfig {
    pub schema: u32,
    pub profile: Option<Profile>,
    pub persona: Option<Persona>,
    pub context: Option<Context>,
    pub hooks: Option<Hooks>,
    pub extends: Option<Vec<String>>,
    pub token_budget: Option<TokenBudget>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Persona {
    pub name: Option<String>,
    pub rules: Option<Vec<String>>,
    pub file: Option<PathBuf>,
    #[serde(rename = "ref")]
    pub ref_name: Option<String>,
    /// SCOPE: Only valid in `.jig.local.yaml`. Rejected (hard error) in `.jig.yaml` or
    /// global config. Enforced by `validate_layer_constraints()` before merge.
    pub extends: Option<String>,
}
```

**Validation rules:**
- `persona.extends` is rejected (error) when found in `.jig.yaml` or global config — only `.jig.local.yaml` may use it (see brainstorm: `docs/brainstorms/2026-03-25-jig-technical-and-ux-brainstorm.md`, §2)
- `schema` version mismatch triggers migration prompt (with backup) before proceeding
- `extends` array cycle detection: circular references produce a clear error, not infinite loop

### Research Insights — Config Schema

**Per-layer validation, not post-merge validation.** The `persona.extends` scope check must run on each layer *before* it joins the merge stack. Once layers are merged, field provenance is lost. Call `validate_layer_constraints(&layer, ConfigSource::TeamProject)` immediately after deserializing each config file. `ConfigSource` is an enum:
```
ConfigSource::GlobalUser | TeamProject | PersonalLocal | CliFlag
```
The validator checks: `persona.extends` is `None` in all sources except `PersonalLocal`. If present elsewhere, return a hard error with the source file path.

**YAML 1.1 foot-guns from `serde_yaml`.** The `serde_yaml` crate was archived by dtolnay in early 2024. It implements YAML 1.1, where `yes`/`no`/`on`/`off` parse as booleans and Norway (`NO`) is a boolean. Replace with:
- **`figment`** (recommended): handles config layering natively, integrates with serde-derived structs, maintained by the Rocket team. Eliminates the need to write merge logic by hand.
- **`serde-yaml-ng`**: drop-in replacement, community-maintained fork of `serde_yaml`, more actively maintained.

**Template `extends` cycle detection algorithm.** Use DFS with grey/white/black visited sets applied to the full resolution graph (not just the immediate parent list). Error message format: `Circular extends: template-a → base → template-a`. A missing base template is a separate error: `Template 'nonexistent' referenced in extends not found (available: base, base-devops, base-frontend)`. Add test fixtures for both direct and indirect cycles.

### Research Insights (Round 2) — Config Schema

**`HookTrustTier` must be a separate enum from `ConfigSource`.** Both enums have a `PersonalLocal` / `GlobalUser` concept, but they serve different concerns. `ConfigSource` tracks where a config value was read from (for error messages and `resolution_trace`). `HookTrustTier` determines the approval UX. The `ExternalSkill` variant of `HookTrustTier` must carry the source URL for display in approval prompts — this data is not in `ConfigSource`. Define them separately:
```rust
pub enum ConfigSource { GlobalUser, TeamProject, PersonalLocal, CliFlag }

pub enum HookTrustTier {
    Full,                           // GlobalUser hooks
    Team,                           // TeamProject hooks
    ExternalSkill { url: String },  // synced skill hooks — URL shown in approval prompt
    Personal,                       // PersonalLocal hooks
}
```

**miette 7 + thiserror 2 integration requires both transparent attributes.** For error variants that wrap another `miette`-enabled error, you need both:
```rust
#[derive(Debug, Error, Diagnostic)]
pub enum ConfigError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolution(#[from] ResolutionError),
}
```
`#[error(transparent)]` alone delegates the `Display` message; `#[diagnostic(transparent)]` delegates the `Diagnostic` trait (spans, help, code). Without both, the inner diagnostic metadata (source spans, help text) is silently dropped.

**Embed `NamedSource<Arc<String>>` in config parse errors.** When `figment` (or manual YAML parsing) fails, include the source file content in the error:
```rust
pub struct ConfigParseError {
    pub source_code: NamedSource<Arc<String>>,
    #[label("here")]
    pub span: SourceSpan,
    pub message: String,
}
```
This enables miette's fancy renderer to display the YAML file content with a caret pointing at the offending line — far more actionable than "parse error at line 7".

---

#### 1.2 Config Resolution (`crates/jig-core/config/resolve.rs`)

Implements the layered merge (PRD §5.4). Priority order: global → project → local → **UI template config** → CLI persona/model flags. Merge semantics:

| Field | Rule |
|---|---|
| `profile.skills` | Union (additive) |
| `profile.mcp` | `layer` = union, `replace` = substitute |
| `profile.settings` (allowedTools, deny) | Union (additive). Template `disallowedTools` applied at CLI priority, winning over file-layer `allowedTools`. |
| `profile.env` | Higher specificity wins per key |
| `persona` | Last wins entirely — **unless** `extends` declared. UI/CLI persona is always last → always wins over `.jig.yaml` persona. |
| Template config | Applied after all file layers (CLI priority). UI template selection overrides `.jig.yaml` scalar fields. |
| `context.fragments` | Union, ordered by `priority` number |
| `hooks` | Concatenated (all levels run) |

**Persona `extends` resolution** (brainstorm §2):

```rust
// crates/jig-core/config/resolve.rs
fn resolve_persona(layers: &[JigConfig]) -> ResolvedPersona {
    let winning_persona = layers.iter().rev()
        .find_map(|l| l.persona.as_ref());

    match winning_persona {
        Some(p) if p.extends.is_some() => {
            // find the named persona in earlier layers, inherit its rules, append p.rules
            let base = resolve_named_persona(layers, p.extends.as_ref().unwrap());
            merge_persona_extends(base, p)
        }
        Some(p) => p.into(),
        None => ResolvedPersona::default(),
    }
}
```

### Research Insights — Config Resolution

**Use `serde_json::Value` (raw JSON value) for `~/.claude.json` roundtrip — never a typed struct.** Claude Code's `~/.claude.json` format is not under jig's control. If jig deserializes the whole file through a typed struct with `#[serde(deny_unknown_fields)]`, any new Claude-added key will cause a parse failure. With default settings, unknown keys are silently dropped — which would delete them. Instead: parse `~/.claude.json` into a raw `serde_json::Value`, extract/inject only `projects."<cwd>".mcpServers` by path, and write the whole Value back. Every key jig does not touch is preserved by construction.

**Parallel YAML file reads for `< 10ms` criterion benchmark target.** The four config files are independent reads. Issue them concurrently using `std::thread::scope`:
```
global_config   ──┐
project_config  ──┤ parallel reads ──→ merge sequentially
local_config    ──┘
```
Single-threaded sequential YAML parsing of 4 files takes ~8–15ms. Parallel reads bring this to ~2–5ms.

**`figment` as the config layering alternative.** If adopting figment, the 4-layer merge becomes declarative:
```rust
let config = Figment::new()
    .merge(Yaml::file(global_path))
    .merge(Yaml::file(project_path))
    .merge(Yaml::file(local_path))
    .merge(Cli::args())
    .extract::<JigConfig>()?;
```
figment handles missing files gracefully (treats them as empty layers) and provides structured error metadata including which layer caused a parse failure.

**Persona `extends` cycle detection must also apply to template `extends` arrays.** Templates use `extends: [base, code-review]` (array, potentially multi-parent). Use the same DFS visited-set algorithm. The test fixtures should include:
- Direct cycle: `a extends [b]`, `b extends [a]`
- Indirect cycle: `a extends [base]`, `base extends [a, other]`
- Missing base: `a extends [nonexistent]`

### Research Insights (Round 2) — Config Resolution

**figment `.admerge()` for array accumulation, `.merge()` for scalar override.** This is the most common figment mistake. `.merge()` replaces the entire value at a key, including arrays. For `profile.skills` (union semantics), `context.fragments` (union + ordered), and `allowedTools` / `disallowedTools` (union), use `.admerge()`. For `persona` (last wins), `env` keys (per-key override), and `schema` version (scalar), use `.merge()`. Mixing these up silently discards earlier layers' array entries.

**`figment::Yaml::file()` silently skips missing files — verify this is intentional.** This is the correct behavior for optional config files (`.jig.local.yaml` need not exist). But log a `TRACE` event for each skipped file so `-vvv` output shows exactly which layers were loaded and which were absent. Without this, debugging a "why isn't my local config being picked up?" issue is painful.

**`~` is not auto-expanded by figment.** If a config value contains `~/some/path`, figment passes it as-is — it does not call `dirs::home_dir()`. Add a post-deserialization pass that expands leading `~/` in all `PathBuf` fields. Apply this before the path-jail check so the canonical path is correct.

**Manual `Provider` impl for CLI flags.** `figment::providers::Serialized::globals()` serializes all struct fields, inserting explicit `null` for `None` values. These nulls override lower-priority layers' real values. Implement the `figment::Provider` trait manually for the CLI flags struct: only insert fields that were explicitly set by the user (i.e., `Option::is_some()` after clap parsing).

**`resolution_trace` as a first-class output field.** Track which `ConfigSource` set each resolved field during the merge. Store as `HashMap<String, ConfigSource>` (dotted path → source). Expose in `--dry-run --json` as `"resolution_trace": {"persona.name": "PersonalLocal", "mcp.postgres": "TeamProject"}`. Also drives `jig config show --explain` human-readable output.

---

#### 1.3 MCP Conflict Resolution (`crates/jig-core/assembly/mcp.rs`)

**Decision (brainstorm §1):** When two sessions in the same working directory define an MCP server with the same name, the second session namespaces its entry with a session suffix derived from the PID.

```rust
// crates/jig-core/assembly/mcp.rs

/// Detects naming conflicts against currently-registered servers in ~/.claude.json.
/// Returns a RenameMap: { original_name -> suffixed_name } for conflicting entries.
pub fn detect_conflicts(
    existing_servers: &HashMap<String, McpServer>,
    new_servers: &HashMap<String, McpServer>,
    session_suffix: &str,           // e.g., "jig_a3f1b2c9" derived from random bytes
) -> RenameMap {
    new_servers.keys()
        .filter(|name| existing_servers.contains_key(*name))
        .map(|name| (name.clone(), format!("{}__{}", name, session_suffix)))
        .collect()
}
```

**Atomic write protocol** (PRD §4, lines 158–178):
1. `flock(~/.claude.json.jig.lock, LOCK_EX)` ← use dedicated lock file, not target
2. Backup `~/.claude.json` → `~/.claude.json.jig-backup-<pid>` (atomic: tmp+rename)
3. Read current contents into `serde_json::Value`
4. Run conflict detection → build rename map
5. Apply renames to new server entries
6. Merge into `projects."<abs_cwd>".mcpServers`
7. Write to `~/.claude.json.jig-<pid>.tmp`
8. POSIX rename `.tmp` → `~/.claude.json` (atomic)
9. Increment ref count in `~/.config/jig/state/<cwd-hash>.refcount` ← MUST be inside flock
10. Release lock

**Cleanup on exit:**
- Re-acquire flock
- Re-read refcount under lock (do not trust in-memory value)
- If count > 0 after decrement: release lock, skip MCP removal (another session still running)
- If count == 0: remove jig-owned entries → atomic write → release
- `jig doctor` cross-checks refcount files against running PIDs, offers orphan cleanup

### Research Insights — MCP Conflict Resolution

**BLOCKING: Refcount increment/decrement MUST occur while the flock is held.** If it happens after lock release, two sessions can both read `count=0`, both write `count=1`, and the first session to exit will decrement to `count=0` and delete the other session's MCP entries mid-flight. The critical section must cover: read → merge → write-rename → refcount-write → unlock. Do not release the lock between the atomic rename and the refcount update.

**Use a dedicated lock file, not the target file.** `flock(~/.claude.json, LOCK_EX)` locks the inode. A POSIX rename replaces the inode — any other process that opened the old inode retains a lock on the old inode, not the new one. Use a stable lock file: `~/.claude.json.jig.lock` (never renamed, never deleted), and flock that. All processes contend on the same inode unconditionally.

**Session suffix: use 8 hex characters (not 4).** 4 hex chars = 65,536 values. In a shared dev server scenario with many concurrent sessions, collision probability grows toward certainty. 8 hex chars = ~4 billion values, negligible collision risk. Use `getrandom` (not `rand`) to fill 4 bytes, then hex-encode them. Generate within the flock, checking existing `mcpServers` keys for collisions; retry up to 32 times with a hard error if exhausted.

**Backup file must be written atomically.** Step 2 (`cp ~/.claude.json → backup`) is a read-then-write, not atomic. If jig crashes mid-copy, the backup is partial JSON. Instead: within the flock, write the backup content to `~/.claude.json.jig-backup-<pid>.tmp`, then rename it to `~/.claude.json.jig-backup-<pid>`. This ensures the backup is always either the previous complete version or the current complete version.

**Use session-unique backup names.** A single shared `~/.claude.json.jig-backup` is overwritten on every launch. Two concurrent sessions would clobber each other's backup. Use `~/.claude.json.jig-backup-<pid>` (session-scoped). Clean up own backup on successful exit; leave on error for `jig doctor` recovery.

**Cleanup race with new session starting.** On exit: acquire lock → read refcount (do not trust in-memory count) → decrement → if zero: remove entries → write → release. This prevents a race where: session A decrements to 0 without the lock, session B starts and reads no conflicts, session A acquires lock and removes entries that B just wrote. Always re-read refcount under the lock before acting on it.

**CWD hash must use `std::fs::canonicalize()`.** Raw `$PWD` differs between symlinked and real paths. Two sessions with different CWD strings pointing to the same physical directory get separate refcount files. Canonicalize the path before hashing. Store the canonical path as a human-readable field inside the refcount file (so `jig doctor` can display it without reversing the hash). Catch `canonicalize()` failure at startup (returns error if a path component does not exist) rather than silently at cleanup.

**Never record expanded MCP args in `history.jsonl`.** MCP server args expanded from `${DATABASE_URL}` may contain plaintext credentials. Store the pre-expansion template string in history, not the expanded value. Set `history.jsonl` permissions to `0600` at creation. Also: when showing `--dry-run` output, mask expanded credential values (show `***` where env vars containing `_TOKEN`, `_KEY`, `_SECRET`, `_URL`, `_PASSWORD` were substituted).

### Research Insights (Round 2) — MCP Conflict Resolution

**CRITICAL: Drop the `fd-lock` guard BEFORE calling `execv`.** `fd-lock` holds an `flock(LOCK_EX)` on `~/.claude.json.jig.lock`. If the `RwLockWriteGuard` (or equivalent) is still alive when `execv("claude", args)` is called, the file descriptor is inherited by the child process. The child holds the exclusive lock for the entire Claude Code session duration — potentially hours. All other `jig` instances trying to launch in parallel will block indefinitely at the lock acquire step. The fix:
```rust
let _guard = lock_file.write()?;     // acquire
// ... perform atomic write, refcount update ...
drop(_guard);                         // MUST drop before exec
executor::exec_claude(args)?;         // exec replaces this process
```
There is no other signal that this is happening — concurrent jig instances just silently hang.

**fd-lock on NFS: detect `EOPNOTSUPP` and fall back.** Some Linux NFS mounts return `EOPNOTSUPP` for `flock()`. Detect this at lock acquisition and fall back to a PID-file advisory lock (`~/.claude.json.jig-<pid>.lock`). Log a `WARN` trace event. The fallback is not race-free, but it is better than a hard failure that blocks all network-mounted users.

**Atomic tmp file must be a sibling of the target.** `rename(tmp, target)` only works atomically if both paths are on the same filesystem. Place `~/.claude.json.jig-<pid>.tmp` in the same directory as `~/.claude.json` (`~/.`). A tmp file in `/tmp` would cross filesystem boundaries on most systems, making the rename non-atomic.

**`sync_data()` before rename.** After writing the tmp file, call `file.sync_data()` (not `sync_all()` — metadata sync is unnecessary) before the rename. Without this, a kernel crash between write and rename can leave `~/.claude.json` as a zero-byte file.

**Expanded credential masking set.** Beyond `_TOKEN`, `_KEY`, `_SECRET`, `_URL`, `_PASSWORD`, also mask: `PGPASSWORD`, `MYSQL_PWD`, `MYSQL_PASSWORD`, `DOCKER_AUTH_CONFIG`, `AWS_SECRET_ACCESS_KEY`, `GCP_CREDENTIALS`, `ANTHROPIC_API_KEY`. Store as a compile-time `phf::Set` or a `const` slice for O(1) lookup.

---

#### 1.4 Permission Rewriting (`crates/jig-core/assembly/permissions.rs`)

**Critical (brainstorm §1):** After conflict detection produces a rename map, all `allowedTools`/`disallowedTools` entries referencing renamed servers must be rewritten before building the `claude` flags.

```rust
// crates/jig-core/assembly/permissions.rs

/// Rewrites permission entries to use suffixed server names.
/// Handles exact matches and glob patterns.
///
/// "mcp__postgres__query"   → "mcp__postgres__jig_a3f1b2c9__query"
/// "mcp__postgres__*"       → "mcp__postgres__jig_a3f1b2c9__*"
pub fn rewrite_mcp_permissions(
    permissions: &[String],
    rename_map: &RenameMap,
) -> Vec<String> {
    permissions.iter()
        .map(|perm| apply_rename_to_permission(perm, rename_map))
        .collect()
}

fn apply_rename_to_permission(perm: &str, rename_map: &RenameMap) -> String {
    // pattern: mcp__<server>__<rest>
    // parse server name, check rename_map, reconstruct
}
```

The pipeline records which servers were renamed **before** building `--allowedTools`/`--disallowedTools` flags. The rewrite step runs immediately after conflict detection, before any permission flags are assembled.

### Research Insights — Permission Rewriting

**Hook command hash must use pre-expansion strings.** The plan specifies pre-expansion for MCP approval hashes (Q5). Apply the same rule to hook approval hashes. Document this explicitly in code: "SHA-256 of the UTF-8 bytes of the command string exactly as it appears in the config file, before `${ENV_VAR}` substitution." This prevents per-machine cache misses for shared team hooks and prevents credential values from entering the approval cache.

**`hook-approvals.json` → switch to JSONL format for concurrent-write safety.** The JSON file with an `"approvals"` array requires a read-modify-write cycle, which races under concurrent launches. Switch to one JSON object per line (JSONL), appended with `O_APPEND`. Single-line appends are atomic for writes smaller than `PIPE_BUF` (4096 bytes on Linux/macOS). Reads reconstruct the approved set by parsing all lines and deduplicating by `(command_hash, source)`:
```
{"command_hash":"sha256:abc...","source":"skill:docker-tools","approved_at":"2026-03-25T10:00:00Z"}
{"command_hash":"sha256:def...","source":"team:.jig.yaml","approved_at":"2026-03-25T10:01:00Z"}
```

**Show diff on re-approval prompts.** When a hook's hash changes and jig re-prompts, show a diff against the previously-approved version (store the full command string in the approval record, not just the hash):
```
Hook from skill 'docker-tools' has changed since last approval:
  - docker compose up -d
  + docker compose up -d && curl -s https://example.com/$(whoami)
Approve? [Y/n]
```

**Skill symlink path jail.** Before creating any symlink in `/tmp/jig-XXXXXX/`, verify the target is a canonical subdirectory of the expected root:
- Synced skills: must be under `~/.config/jig/skills/<source-name>/`
- Local skills: must be under the directory containing `.jig.yaml`
- Fragments: same prefix check

Use `std::fs::canonicalize()` on each path and check the prefix. Abort with a clear error if escape is detected. Apply the same check to `context.fragments`, `profile.plugins.path`, `persona.file`, and any other user-supplied path fields.

---

#### 1.5 Hook and MCP Trust Tiers (`crates/jig-core/config/validate.rs` + security module)

**Decision (brainstorm §3):** Four-tier trust model with source-aware prompting.

| Source | Trust | Behavior |
|---|---|---|
| `~/.config/jig/config.yaml` | Full | Prompt once, cache by SHA-256 hash (not auto-approved unconditionally) |
| `.jig.yaml` (committed to git) | Team | Prompt: "Hook from team config (committed to git): `<cmd>`" |
| Synced skills | External | Prompt: "Hook from skill `<name>` (source: `<url>`): `<cmd>`" |
| `.jig.local.yaml` | Personal | Prompt once, cache by SHA-256 hash |

**Approval cache** at `~/.config/jig/state/hook-approvals.jsonl` (JSONL format):

```
{"command_hash":"sha256:abc...","command":"python scripts/pull.py","source":"skill:docker-tools","approved_at":"2026-03-25T10:00:00Z"}
```

Cache key is `(command_hash, source)`. If the same command hash now appears under a different source, prompt again — the source change could indicate a supply-chain substitution.

**MCP first-run approval** (PRD §14, security model): MCP servers from skills and team configs require the same approval-cache pattern as hooks. Cache keyed on `(server_name, command_hash, source)`. Re-prompts if the server definition changes or if its source changes. Hash is computed on the **pre-expansion** server definition (command + unexpanded arg strings).

**Worktree detection:** On launch, check if `$PWD` is inside a git worktree. Emit a warning if concurrent sessions are detected in the same worktree. Record `worktree: bool` and `concurrent_sessions_at_launch: [pid, ...]` in the session start history entry.

### Research Insights — Hook and MCP Trust Tiers

**BLOCKING: `--yes` flag scope must be restricted.** Auto-approving external skill hooks in CI is a direct supply-chain attack vector. A compromised skill repository with an injected hook + a CI pipeline using `jig --yes --go` = silent code execution on the build server. Proposed scoped model:
- `--yes` (as currently described): only auto-approves items already in the approval cache (previously manually approved)
- `--yes-team`: auto-approves global + team config hooks (no external skills)
- `--yes-all` (explicit opt-in, printed warning): auto-approves everything including external skills

Items that have never been approved must still fail with an error in `--yes` mode, forcing a first-run manual approval before automated use. Document this in the CLI spec and in the `jig --help` text for the flag.

**BLOCKING: Global config must have first-run approval, not unconditional auto-approval.** The threat model of "the user wrote it" breaks down because: (a) `jig import` can write to global config scope, (b) a compromised process running as the same user can append to the file, (c) the file may have weak permissions on a multiuser system. Instead: first time jig encounters a hook in global config, prompt once. Cache by hash (same as `.jig.local.yaml`). Add a startup ownership check: `~/.config/jig/config.yaml` must be owned by `getuid()` and have permissions `0600` or `0640`. If not, error with a specific `chmod 600` instruction before reading it.

**`jig import` must prompt before writing hooks to global scope.** The import flow should show each hook verbatim and require explicit confirmation before writing to global config, regardless of which storage scope the user selected.

**Env var expansion security.** When an MCP arg contains `${DATABASE_URL}`, display the current (masked) value at approval time: "Note: `${DATABASE_URL}` resolves to `postgresql://user:***@localhost/db` on this machine." Use `jig doctor --audit` to warn when a pre-expansion string contains a substitution that currently resolves to a non-localhost host.

**`claude_flags` passthrough needs an allowlist.** The schema includes `profile.settings.claude_flags` for raw Claude CLI flag passthrough. Any flag that jig manages itself (e.g., `--append-system-prompt-file`, `--add-dir`, `--allowedTools`) must be blocked — passing them via `claude_flags` would conflict with jig's own assembled flags. Implement an explicit allowlist of permitted passthrough flags, not a denylist.

### Research Insights (Round 2) — Hook and MCP Trust Tiers

**Hook execution model: `exec:` array vs `shell: true`.** Specify in the config schema:
```yaml
hooks:
  pre_launch:
    - exec: ["python", "scripts/pull.py"]          # direct exec (default, no injection)
    - exec: ["./scripts/setup.sh"]
    - command: "echo hello && date"                 # legacy string — requires shell: true
      shell: true                                   # explicit opt-in for shell semantics
```
For the `exec: []` array form, use `std::process::Command::new(args[0]).args(&args[1..])` directly — no shell invocation. For `command: string` without `shell: true`, error with a clear message: "Hook command string requires `shell: true`. To avoid shell injection, use `exec: [\"cmd\", \"arg\"]` instead." This forces a conscious decision and documents the risk at the config level.

**`ApprovalUi` trait — full specification:**
```rust
// crates/jig-core/src/security/approval.rs

pub struct ApprovalRequest {
    pub tier: HookTrustTier,
    pub command: String,          // full command as it appears in config
    pub source_file: PathBuf,     // file where hook was defined
    pub previous_command: Option<String>,  // for diff display on re-approval
}

pub enum ApprovalDecision { Approved, Denied, ApproveSession }

pub trait ApprovalUi: Send {
    fn prompt_approval(&self, req: &ApprovalRequest) -> ApprovalDecision;
}
```
Wire `ApprovalUi` into `assembly::stage::run()` as a `&dyn ApprovalUi` parameter. The binary (jig-cli) passes `TerminalApprovalUi` for headless mode and `TuiApprovalUi` during TUI launch. Tests pass `MockApprovalUi`. This is the correct boundary: business logic in core, interaction in the binary.

**Approval cache `last_used_at` and TTL eviction.**  Add `last_used_at` to each approval record:
```json
{"command_hash":"sha256:abc...","command":"python scripts/pull.py","source":"skill:docker-tools","approved_at":"2026-03-25T10:00:00Z","last_used_at":"2026-03-25T11:30:00Z"}
```
On each cache hit, update `last_used_at` (via a new appended line with the same hash — deduplication on read takes the most recent `last_used_at`). TTL: External tier entries expire after 90 days of inactivity; Full/Personal entries expire after 1 year. `jig doctor --audit` reports and prunes expired entries.

**TOCTOU in global config ownership check.** The current plan suggests checking file ownership with `stat()` before reading. This is a classic TOCTOU: a symlink swap between `stat()` and `open()` passes the check but reads a different file. Use open-then-fstat:
```rust
let file = File::open(&config_path)?;
let metadata = file.metadata()?;  // fstat on the already-open fd
assert_eq!(metadata.uid(), unsafe { libc::getuid() });
```
This eliminates the race window.

---

#### 1.6 Assembly Pipeline (`crates/jig-core/assembly/stage.rs`)

The full 16-step pipeline (PRD §8.1, lines 512–583):

```
Step  1: Detect environment (claude binary, git worktree, concurrent sessions → session suffix)
Step  2: Resolve config (global → project → local → UI template → CLI flags)
Step  3: Expand from_source shorthands + ${ENV_VAR} substitutions in MCP URLs
Step  4: Check schema version (prompt migration if needed)
Step  5: Check skill/plugin dependencies (prompt install if missing)
Step  6: Security approvals (hook trust tier evaluation + approval cache check)
Step  7: Run pre_launch hooks (with source-tagged error attribution)
Step  8: Stage temp dir at /tmp/jig-XXXXXX/ (mode 0700)
Step  9: Symlink skills into temp dir (with path jail validation)
Step 10: Write MCP to ~/.claude.json (atomic, flock, conflict-detect, rename-map, refcount — all inside flock)
Step 11: Build claude invocation flags
Step 12: Export env vars
Step 13: Record session start in ~/.config/jig/state/history.jsonl
Step 14: Fork: child executes setpgid(0,0) then exec("claude", flags)
Step 15: Parent installs SIGINT/SIGTERM/SIGHUP handlers → forward to child pgid
Step 16: Parent waitpid(child)

Cleanup (always runs, even on error after Step 10):
  Category A (always, including on error/panic):
    - Re-acquire flock; re-read refcount under lock; if zero → remove MCP entries → atomic write
    - Remove /tmp/jig-XXXXXX/ temp dir
    - Release lock
  Category B (clean exit only — only if waitpid returned normally):
    - Run post_exit hooks
    - Update history.jsonl with exit record (separate appended line, not mutation)
```

**Error handling guarantee:** Steps 10+ write state. If any step fails after Step 10, the Category A cleanup sequence must run before propagating the error. Use a `SessionGuard` struct that implements `Drop`.

```rust
// Pseudocode: SessionGuard holds all written state references
pub struct SessionGuard {
    temp_dir: TempDir,
    mcp_written: bool,
    session_suffix: String,
    cwd: PathBuf,
    exit_outcome: Option<ExitOutcome>,  // set after waitpid returns normally
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        // Category A: always run
        if self.mcp_written {
            let _ = mcp::cleanup_entries(&self.cwd, &self.session_suffix);
        }
        // temp_dir auto-cleans via TempDir::Drop

        // Category B: clean exit only
        if let Some(outcome) = &self.exit_outcome {
            let _ = hooks::run_post_exit();
            let _ = history::record_exit(outcome);
        }
        // Never panic inside Drop — use let _ = for all fallible operations
    }
}
```

### Research Insights — Assembly Pipeline

**`stage.rs` must be a thin sequencer — no step logic inline.** Maximum 200 lines in `stage.rs`. All step-level logic lives in the module it belongs to:
- Steps 1 (environment detection) → `context.rs` or `env_detect.rs`
- Steps 8–9 (temp dir + symlinks) → `stage.rs` owns the `SessionGuard`
- Steps 10–12 (MCP write, flag building, env export) → respective sub-modules
- Steps 13–16 (history, fork, signal handling, waitpid) → `executor.rs` (not `launch.rs`)

**Signal handler ordering constraint (document in `executor.rs`):**
```
Signal forwarding is active from fork until waitpid returns. The signal handler
only calls kill(-pgid, sig) — no Rust allocations, no locks. Cleanup via Drop
runs strictly after waitpid returns, at which point no signals are being forwarded.
Do not move any cleanup logic before the waitpid call.
```
Register signal handlers BEFORE `fork()`. After fork, the child inherits handler registrations but `signal-hook` masks them in the child — handlers only fire in the parent. Call `setpgid(0, 0)` in the child before `exec`. Forward signals to `-child_pgid` (negative = entire process group) to reach all grandchildren.

**Dispatch `--help`, `--version`, and `completions` before any config I/O.** These commands must complete in < 50ms. If `main.rs` resolves config before dispatching to help, the full YAML parse runs unconditionally. Route these to their handlers before calling `config::resolve()`.

**Hook stderr capture.** In Step 7, use `std::process::Command::output()` (not `status()`). On non-zero exit, include captured stderr in the `miette` diagnostic as a secondary label. On success with non-empty stderr from a "silent" hook, emit a `warn!` trace event. Truncate captured output at 4KB with a "... truncated" indicator.

**`jig --dry-run` shows hooks that would run, without executing them.** The dry-run output should list each hook with its source tier label:
```
Hooks that would run:
  [team] pre_launch: python scripts/pull-analytics.py
  [personal] pre_launch: echo "session starting"
```
This is both a security-review aid and a debugging tool.

**Drop guard panic safety.** Never call `.unwrap()` or `.expect()` inside `Drop`. Use `let _ = ...` for all fallible cleanup operations. Stack multiple `scopeguard::defer!` blocks for independent resources so each is independently cleaned up if a previous one panics.

### Research Insights (Round 2) — Assembly Pipeline

**Signal handling sequence (complete specification for `executor.rs`):**
```rust
// 1. Register signal handlers BEFORE fork() — child inherits the handler
//    registrations but signal-hook masks them in the child.
let signals = signal_hook::iterator::Signals::new(&[SIGINT, SIGTERM, SIGHUP])?;

// 2. fork()
let child_pid = unsafe { nix::unistd::fork() }?;

match child_pid {
    ForkResult::Child => {
        // 3. In child: create new process group so killpg reaches grandchildren
        nix::unistd::setpgid(Pid::from_raw(0), Pid::from_raw(0))?;
        // 4. exec — replaces child process image
        let err = nix::unistd::execvp(&prog, &args)?;
        // 5. exec failed — use _exit (not exit!) to avoid running atexit handlers
        unsafe { libc::_exit(127) };
    }
    ForkResult::Parent { child } => {
        let child_pgid = child;  // pgid == child pid right after setpgid(0,0)
        // 6. Forward signals to entire process group
        for sig in signals.forever() {
            let _ = nix::sys::signal::killpg(child_pgid, sig);
        }
    }
}

// 7. waitpid loop with EINTR retry (macOS more likely to surface EINTR)
loop {
    match nix::sys::wait::waitpid(child, Some(WaitPidFlag::WNOHANG)) {
        Ok(WaitStatus::Exited(_, code)) => break code,
        Ok(WaitStatus::Signaled(_, sig, _)) => {
            // Re-raise so jig's own exit code reflects the signal (e.g., $? = 130 for SIGINT)
            unsafe { libc::raise(sig as libc::c_int) };
            break 128 + sig as i32;
        }
        Err(Errno::EINTR) => continue,  // retry on EINTR
        _ => continue,
    }
}
```

**`process::exit()` is forbidden after Step 10.** Once `SessionGuard` is constructed (after MCP write), the only valid exits are:
- Normal return from the function (runs `Drop`)
- `exec` (replaces process image — Drop does NOT run, so drop the guard FIRST)
- Panic (runs Drop only if `panic = "unwind"`; with `panic = "abort"`, Drop is skipped — the panic hook must do Category A cleanup before calling `process::abort()`)

Add a lint check (or doc comment) to `executor.rs`: "Never call `process::exit()` from this module."

**`_exit(127)` after failed exec in child.** After a failed `execvp`, use `libc::_exit(127)` (not `std::process::exit(127)`). `process::exit()` runs Rust `atexit` handlers and flushes stdio buffers — in the child process after a failed exec, this would double-flush the parent's stdio. `_exit` exits immediately without cleanup.

**Dispatch order for fast commands.** The dispatch order in `main.rs` must be:
1. `--help` / `--version` / `completions` — return before any I/O
2. Initialize `tracing-subscriber` based on `--verbose` count
3. Config I/O (YAML reads)
4. Everything else

Tracing init before config I/O enables `TRACE`-level logs for the YAML parse phase, which is essential for diagnosing "why isn't my config being picked up" issues.

---

#### 1.7 CLI Argument Parsing (`crates/jig-cli/src/cli.rs`)

Using `clap` derive macros. Primary commands:

```rust
// crates/jig-cli/src/cli.rs
#[derive(Parser)]
#[command(name = "jig", version, about)]
pub struct Cli {
    /// Open TUI (default when no flags given)
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short = 't', long)]
    pub template: Option<String>,

    #[arg(short = 'p', long)]
    pub persona: Option<String>,

    #[arg(long)]
    pub last: bool,

    #[arg(long)]
    pub go: bool,

    #[arg(long)]
    pub resume: bool,

    #[arg(long)]
    pub dry_run: bool,

    #[arg(long)]
    pub json: bool,      // with --dry-run: JSON output

    #[arg(long, global = true)]
    pub yes: bool,       // auto-approve cache-only (see security section for scope)

    #[arg(long, global = true)]
    pub non_interactive: bool,

    /// Verbosity: -v (info), -vv (debug), -vvv (trace)
    #[arg(short = 'v', long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand)]
pub enum Commands {
    Template(TemplateArgs),
    Persona(PersonaArgs),
    Skill(SkillArgs),
    Sync(SyncArgs),
    Init(InitArgs),
    Import(ImportArgs),
    Doctor,
    History(HistoryArgs),
    Diff(DiffArgs),
    Completions(CompletionsArgs),
    Config(ConfigArgs),     // new: jig config set <key> <value>
}
```

**Routing logic:**
- No flags + no subcommand → TUI (Phase 2)
- `--template` / `-t` → headless launch
- `--go` → headless, use `.jig.yaml` defaults
- `--last` → headless, repeat last history entry
- `--dry-run` → assemble but don't fork; print resolved command (+ JSON if `--json`)
- Any subcommand → route to subcommand handler

### Research Insights — CLI Argument Parsing

**Add `--verbose / -v` to the CLI spec.** The assembly pipeline has 16 steps and a 4-layer config merge. Without structured logging, assembly failures in headless/CI use are undiagnosable. Add:
- `-v` / `--verbose` (count flag): `-v` = info, `-vv` = debug, `-vvv` = trace
- Initialize `tracing-subscriber` in `main.rs` based on this count (or `RUST_LOG` env var)
- `jig-core` uses `tracing` instrumentation; the subscriber is initialized only in `jig-cli`

**Add `jig config set <key> <value> [--scope local|project|global]`.** This is the most important agent-native parity gap. The TUI editor lets users modify config fields interactively; agents have no equivalent — they must parse and edit raw YAML. A CLI mutation command closes this gap. Use dotted path notation: `jig config set profile.settings.model claude-opus-4 --scope local`.

**Add `--json` as a global flag for all list/show commands.** Every command that produces output should support `--json` for machine-readable output. This applies to: `jig template list`, `jig persona list`, `jig skill list`, `jig history`, `jig template show <name>`. With `--json`, emit newline-delimited JSON or a JSON object; no human-readable text mixed in.

**`--resume` semantics must be specified.** When `jig --resume` is run, it should use the most recent entry from `history.jsonl` for re-staging (same as `--last`), and additionally pass `--resume` to claude. Combining `--resume` with `-t T` is an error: "Cannot specify --resume and --template together."

**clap 4.x note:** Use `#[arg(...)]` (not `#[clap(...)]`) and `#[command(...)]` — the canonical attribute form since clap 4.0. Use `env = "JIG_CONFIG"` on the config path arg for automatic env-var fallback.

### Research Insights (Round 2) — CLI Argument Parsing

**`--session <UUID>` replaces `--last-id <N>`.** The plan currently specifies `--last-id <N>` for relaunching arbitrary history entries. Positional indices are fragile: concurrent sessions writing to `history.jsonl` can change the effective index of an entry. The UUID is already present in every `history.jsonl` record. Use `--session <UUID>` instead:
```
jig --session 4a2f1b9c-...   # relaunch a specific session by UUID
jig --last                    # relaunch most recent (shorthand, UUID resolved at runtime)
```
`jig history --json` must emit joined session objects with the UUID in each record so agents can reference them.

**`jig config add` / `jig config remove` for array fields.** Dotted path `set` handles scalar fields. Array fields (`profile.skills`, `context.fragments`) need dedicated operations:
```
jig config add profile.skills docker --scope local
jig config remove profile.skills docker --scope local
```
Without these, an agent must read the YAML, parse it, modify the array, and write it back — error-prone and not atomic.

**`jig history --json` must emit joined objects, not raw JSONL.** Raw JSONL has start records and exit records as separate lines. An agent querying `jig history --json` should receive pre-correlated objects:
```json
[{"session_id":"uuid","started_at":"...","ended_at":"...","exit_code":0,"template":"base-devops","duration_ms":14523}]
```
Joining is done by `session_id` in the history reader. Records with no matching exit get `"ended_at":null,"exit_code":null,"status":"crash_or_running"`.

---

### Phase 2: TUI Implementation

**Decision (brainstorm §4):** TUI always shows on bare `jig`. Bypassed by `--go`, `-t`, `--last`, `--dry-run`.

#### 2.1 TUI App Structure (`crates/jig-tui/src/app.rs`)

Framework: `ratatui` + `crossterm`. Event loop: `crossterm::event::poll(timeout)` in a dedicated thread, send to main loop via `mpsc::channel`.

```rust
// crates/jig-tui/src/app.rs
pub struct App {
    templates: FilterableList<Template>,
    personas: FilterableList<Persona>,
    focus: PaneFocus,       // Templates | Personas
    preview: PreviewPane,
    mode: AppMode,          // Normal | Filter | Confirm | WhichKey
    terminal_cols: u16,
    layout: LayoutMode,     // FullTwoPane | NarrowTwoPane | SinglePane | Minimal
}

enum LayoutMode {
    FullTwoPane,    // ≥100 cols
    NarrowTwoPane,  // 80–99 cols
    SinglePane,     // <80 cols — preview toggled with `p`
    Minimal,        // <60 cols — list-only
}
```

### Research Insights — TUI App Structure

**Install panic hook before `init_terminal()`.** A panic in raw mode leaves the terminal broken. The correct startup order:
1. Parse CLI args
2. Load and validate config
3. Install signal handlers (`signal-hook::Signals`)
4. Call `install_panic_hook()` — restores terminal state before printing panic backtrace
5. Call `init_terminal()` — enters raw mode + alternate screen + mouse capture
6. Use `scopeguard::defer! { restore_terminal(); }` as belt-and-suspenders

```rust
pub fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::event::DisableMouseCapture,
        );
        original(info);
    }));
}
```

**Event loop: `poll(timeout)` in thread + `mpsc` channel.** Use `event::poll(Duration::from_millis(100))` in the event thread — this allows `Tick` events to be emitted even when no input arrives (needed for launch transition spinner animation). Direct `event::read()` without `poll` blocks indefinitely.

**Define a `PreviewData` struct as the boundary between `jig-core` and `jig-tui`.** The TUI preview pane should receive a `PreviewData` struct from `jig-core`, not a raw `ResolvedConfig`. This keeps `jig-tui` from depending on internal config types and makes preview rendering independently testable and benchmarkable against the < 50ms update requirement:
```
PreviewData {
    token_count: usize,
    skills: Vec<String>,
    permissions_summary: String,
    system_prompt_lines: Vec<Line<'static>>,
    worktree_warning: bool,
    active_concurrent_sessions: usize,
}
```

**Terminal minimum size enforcement.** In the render function (not at startup — enforce on every draw to handle resize-down):
```rust
if area.width < 40 || area.height < 24 {
    // render error message; do not render normal TUI
    return;
}
```
Before entering raw mode at startup, check `crossterm::terminal::size()` and print a clear error and exit (without entering raw mode) if too small.

**TUI testing.** ratatui ships `TestBackend` for rendering tests:
```rust
let backend = ratatui::backend::TestBackend::new(80, 24);
let mut terminal = Terminal::new(backend).unwrap();
terminal.draw(|frame| render_session_list(frame, &state)).unwrap();
let buffer = terminal.backend().buffer().clone();
insta::assert_snapshot!(format!("{}", buffer));
```
Keep event handling pure: `fn handle_key_event(state: &mut AppState, event: KeyEvent) -> Action` — decouples rendering from state transitions, makes both independently testable.

### Research Insights (Round 2) — TUI App Structure

**ratatui 0.29 API changes — verify before writing TUI code:**
- `frame.size()` is deprecated → use `frame.area()` (returns `Rect`)
- `Frame<B: Backend>` generic parameter removed → `Frame` is now concrete
- `Layout::areas::<N>(area)` returns `[Rect; N]` for compile-time destructuring (cleaner than `.split()` + indexing)
- `Constraint::Fill(n)` replaces `Constraint::Min(0)` hacks for stretch behavior
- `WidgetRef` trait added — implement for stateful widgets stored in structs (avoids `clone()` on render)

Terminal restore order matters: `DisableMouseCapture` → `LeaveAlternateScreen` → `disable_raw_mode()` → `cursor::Show`. Reversing any of these leaves a broken terminal state.

**`PreviewData.system_prompt_lines` must be `Vec<String>`, not `Vec<Line<'static>>`.** The `Line<'static>` type is from ratatui. If `jig-core` uses it, `jig-core` gains a ratatui dependency, breaking the headless build. Define `PreviewData` in `jig-core` with stdlib-only types:
```rust
// crates/jig-core/src/assembly/preview.rs
pub struct PreviewData {
    pub token_count: usize,
    pub token_count_method: TokenCountMethod,  // Heuristic | Tiktoken
    pub skills: Vec<String>,
    pub permissions_summary: String,
    pub system_prompt_lines: Vec<String>,      // plain text lines — NOT Vec<Line<'static>>
    pub worktree_warning: bool,
    pub active_concurrent_sessions: usize,
}
```
In `jig-tui`, convert `Vec<String>` → `Vec<Line<'static>>` via the markdown converter before passing to `Paragraph`.

**`preview::compute()` runs only steps 1–3 of the assembly pipeline.** The preview must NOT write any state (no temp dir, no MCP write). It only reads config, resolves, and generates the system prompt text. This is a strict subset of the full pipeline. Mark it clearly in code: `// preview only: steps 1-3. No state mutations.` Add a compile-time check: `preview.rs` must not import `mcp`, `stage`, or `executor`.

**Filter key events by `KeyEventKind::Press`.** On Windows, ratatui/crossterm emits `KeyEventKind::Press`, `KeyEventKind::Repeat`, and `KeyEventKind::Release`. Filter to `Press` only (or `Press | Repeat` for held keys) to avoid double-processing:
```rust
if event.kind != KeyEventKind::Press { return; }
```

**Mouse coordinates are terminal-absolute, not widget-relative.** `MouseEvent` column/row values are absolute terminal coordinates. Convert to widget-relative before checking if a click is inside a widget: `if event.column >= widget_area.x && event.column < widget_area.x + widget_area.width { ... }`.

---

#### 2.2 Two-Pane Layout (`crates/jig-tui/src/app.rs`)

**Decision (brainstorm §5):** Left pane = Templates + Personas lists; right pane = live scrollable preview.

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

| Key | Action |
|---|---|
| `j` / `k` | Navigate current list |
| `Tab` | Switch focus: Templates ↔ Personas |
| `Ctrl+D` / `Ctrl+U` | Scroll preview pane independently |
| `/` | Enter filter mode for current list |
| `Enter` | Launch with selected template + persona |
| `d` | Dry-run (show resolved command) |
| `p` | Toggle preview pane (single-pane mode only) |
| `L` | Relaunch last session |
| `e` | Edit selected template/persona (opens `$EDITOR` in Phase 1) |
| `s` | Trigger `jig sync` |
| `h` | Open session history view |
| `?` | Which-key popup |
| `q` / `Esc` | Quit |

### Research Insights — Two-Pane Layout

**`Layout::new` pattern (ratatui 0.28+):**
```rust
let chunks = Layout::new(
    Direction::Horizontal,
    [Constraint::Percentage(35), Constraint::Percentage(65)],
).split(frame.area());
```
Use `frame.area()` (not `frame.size()` — deprecated in ratatui 0.29). Use `Constraint::Fill(1)` for the stretch constraint (replaces `Min(0)` hacks). Use `Layout::vertical()` / `Layout::horizontal()` shorthand constructors.

**Mouse scroll support for preview pane.** Enable `crossterm::event::EnableMouseCapture` at terminal init. Handle `MouseEventKind::ScrollDown` / `ScrollUp` in the event loop: `preview.scroll_down(3)` / `scroll_up(3)`. This is how users scroll the preview pane without touching the keyboard.

**Which-key popup pattern.** Compute a centered `Rect`, render `Clear` widget first (prevents background bleed-through), then render the keybindings `Paragraph` on top:
```rust
frame.render_widget(Clear, popup_area);
frame.render_widget(keybindings_widget, popup_area);
```
Check `app.mode == AppMode::WhichKey` after rendering the main layout, not before.

**Filter mode behavior (Q30 answer).** When filter produces zero matches: show "No results" placeholder in list area. When user backspaces to empty string: restore full list immediately (no Esc needed). Filter applies to both template name and description. Algorithm: fuzzy (nucleo-matcher), not substring.

### Research Insights (Round 2) — Two-Pane Layout

**nucleo-matcher lifecycle — complete `FilterableListState` implementation:**
```rust
// crates/jig-tui/src/widgets/filterable_list.rs
use nucleo_matcher::{Config, Matcher, Utf32Str, pattern::{Atom, AtomKind, CaseMatching}};

pub struct FilterableListState {
    items: Vec<String>,           // full list, pre-loaded at TUI init
    filtered: Vec<(u32, usize)>,  // (score, original_index) — pre-populated at init
    query: String,
    // Matcher is !Send + !Sync — must stay on TUI thread
    matcher: Matcher,
}

impl FilterableListState {
    pub fn new(items: Vec<String>) -> Self {
        let mut state = Self {
            items,
            filtered: Vec::new(),
            query: String::new(),
            matcher: Matcher::new(Config::DEFAULT),
        };
        state.update_filter();  // pre-populate on init, not on first keypress
        state
    }

    pub fn update_filter(&mut self) {
        if self.query.is_empty() {
            // empty query shows all items (do not hide everything)
            self.filtered = self.items.iter().enumerate()
                .map(|(i, _)| (0u32, i))
                .collect();
            return;
        }
        let atom = Atom::new(&self.query, CaseMatching::Smart, AtomKind::Fuzzy, false);
        let mut indices = Vec::new();  // scratch buffer — must outlive Utf32Str
        self.filtered = self.items.iter().enumerate()
            .filter_map(|(i, item)| {
                let haystack = Utf32Str::new(item, &mut indices);
                let mut score_indices = Vec::new();
                atom.score(haystack, &mut self.matcher, &mut score_indices)
                    .map(|score| (score, i))
            })
            .collect();
        self.filtered.sort_unstable_by(|a, b| b.0.cmp(&a.0));  // high score first
        // Note: nucleo returns indices unsorted — sort_unstable is required
    }
}
```

Key constraints:
- `Matcher` is `!Send + !Sync` — do not move it across threads or put it in an `Arc`
- The scratch `Vec<u32>` (`indices`) must outlive the `Utf32Str` borrow — keep it alive for the entire `score()` call
- Pre-populate `filtered` at `new()`, not on first keypress (avoids a flash of empty list)
- Empty query shows all items, not zero items

**`ratatui::widgets::StatefulWidget` for `FilterableListState`:**
```rust
impl StatefulWidget for FilterableListWidget<'_> {
    type State = FilterableListState;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // render visible window of state.filtered
    }
}
```
Store the state in `App`, pass a reference to the widget on each render. This is the ratatui-idiomatic pattern for scrollable lists.

---

#### 2.3 Preview Pane Depth (`crates/jig-tui/src/app.rs`)

**Decision (brainstorm §6):** Always render the full composed system prompt, scrollable independently. No mode switching required.

Preview renders in order:
1. Token count estimate (prominent, top — fast approximation, labeled `~`)
2. Skills list (inline, comma-separated)
3. Permissions summary (tools count)
4. Full system prompt (persona rules + context fragments in assembly order, rendered from Markdown via `pulldown-cmark`)

The preview updates on every selection change. Token estimation must be fast enough for interactive use (< 50ms for typical configs).

### Research Insights — Preview Pane Depth

**Debounce preview updates (50ms).** In filter mode, every keypress changes the selected item and triggers a preview update. For large system prompts (10k tokens, ~40KB Markdown), pulldown-cmark rendering + token estimation takes 10–15ms. At 10 keystrokes/second, this causes dropped frames. Debounce: track `last_selection_change: Instant`; only compute preview after 50ms of quiet. Show "updating..." in the token count line while debouncing.

**Cache rendered preview behind content hash.** Call `markdown_to_lines()` once on selection change, store in `PreviewPane::content: Vec<Line<'static>>`. Do not call it during render. Use a fast hash (FxHasher or DefaultHasher) over the prompt string to detect content changes vs. scroll-only events:
```rust
struct PreviewCache {
    content_hash: u64,
    rendered: Vec<Line<'static>>,
    token_count: usize,
}
```
Scroll operations only change `scroll_offset`; they do not re-render.

**`Paragraph::scroll` for independent preview scrolling:**
```rust
let paragraph = Paragraph::new(Text::from(preview.content.clone()))
    .scroll((preview.scroll_offset, 0))
    .wrap(Wrap { trim: false });
```
Do not use `Wrap { trim: true }` — it breaks indented code blocks. Clamp `scroll_offset` to `max_scroll()` = `total_lines.saturating_sub(viewport_height)`.

**pulldown-cmark → ratatui `Line<'static>` converter.** Write a thin event-to-spans converter in `crates/jig-tui/src/widgets/markdown_viewer.rs`. Map headings → bold+cyan, inline code → yellow, code blocks → dark background, strong → bold, emphasis → italic. Use `text.to_string()` on all `CowStr` values from pulldown-cmark to satisfy `'static` lifetime.

**Token count breakdown by component.** When token budget warn threshold is triggered, the display should show per-component breakdown:
```
~4,200 tokens [WARN]
  Persona: strict-security  1,800
  Fragment: code-standards.md  1,400
  Fragment: team-context.md  1,000
```
This gives users actionable information about what to cut.

**Fast token approximation for interactive use.** Use `text.len() / 4` (chars-per-token heuristic, ~15% error rate for English prose) for the live preview display. Label it `~` to indicate approximation. Reserve tiktoken-rs (if feature-enabled) for `--dry-run --json` output and token budget threshold checks.

### Research Insights (Round 2) — Preview Pane Depth

**`FxHasher` for preview content cache.** The preview cache uses a hash to detect whether content changed (scroll vs. selection change). Use `rustc-hash::FxHasher` (or `std::collections::hash_map::DefaultHasher`) — not SHA-256, which is for security hashes. `FxHasher` is ~3× faster than `DefaultHasher` for short strings and compiles to near-zero overhead:
```rust
use std::hash::{Hash, Hasher};
use rustc_hash::FxHasher;

fn content_hash(s: &str) -> u64 {
    let mut h = FxHasher::default();
    s.hash(&mut h);
    h.finish()
}
```
Add `rustc-hash = "2"` to `jig-tui` dev and runtime deps. Do NOT use `FxHasher` for security-sensitive hashes (approval cache, ownership check) — those stay as `sha2`.

**`Vec::with_capacity(256)` for `Line` construction in the markdown converter.** The markdown converter iterates `pulldown-cmark` events and builds `Vec<Line<'static>>`. Pre-allocating with `with_capacity(256)` avoids repeated reallocations for typical-sized system prompts (50–500 lines). The actual number doesn't matter much — 256 is a good lower bound.

**`Paragraph::scroll` offset clamping.** The scroll offset must be clamped on every resize event, not just on scroll input:
```rust
let max_scroll = (preview.line_count as u16).saturating_sub(area.height);
preview.scroll_offset = preview.scroll_offset.min(max_scroll);
```
Terminal resize can shrink `area.height`, making a previously-valid `scroll_offset` exceed the new maximum. Without clamping on resize, the paragraph renders blank (offset past the end).

**`token_count_estimate` + `token_count_method` in the exit record.** The preview computes a token count. Record this in the history exit record so `jig history --json` can show token counts retroactively:
```json
{"type":"exit","session_id":"...","token_count_estimate":2400,"token_count_method":"heuristic","fragment_count":3}
```
`token_count_method` is `"heuristic"` (char/4 approximation) or `"tiktoken"` (exact, if feature-enabled).

---

#### 2.4 Launch Transition Screen (`crates/jig-tui/src/launch.rs`)

**Decision (brainstorm §7):** Brief assembly status screen before handing off to Claude Code.

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
- Each step transitions from `⟳` (in-progress) to `✓` (done) or `✗` (failed)
- On failure: step shows `✗` with error detail and elapsed time; TUI stays up for user to read before exit

The transition screen runs as a separate `ratatui` render loop. After `exec("claude")`, the terminal is fully handed off — no leftover TUI output.

### Research Insights — Launch Transition Screen

**Call `restore_terminal()` BEFORE `execv("claude", args)`.** If alternate screen mode is active when `exec` is called, Claude Code inherits it. Call `restore_terminal()` (disable raw mode + leave alternate screen + show cursor) immediately before the `exec` syscall. This ensures Claude Code starts with a clean terminal.

**Per-step timing on failure.** When a step fails (`✗`), show elapsed time: `✗ Writing MCP to ~/.claude.json... (1.2s) — No space left on device`. This turns the transition screen from a progress display into an actionable failure diagnostic.

**Minimum display time without `async`.** Use `std::time::Instant::now()` at transition start. After all steps complete, `sleep(MIN_DISPLAY - elapsed)` if elapsed < 500ms. Do not add a separate async runtime for this.

### Research Insights (Round 2) — Launch Transition Screen

**Complete terminal restore sequence before exec:**
```rust
pub fn restore_terminal() {
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::event::DisableMouseCapture,   // 1st
        crossterm::terminal::LeaveAlternateScreen,  // 2nd
        crossterm::cursor::Show,                  // 3rd
    );
    let _ = crossterm::terminal::disable_raw_mode();  // 4th
}
```
This order is critical. Reversing `LeaveAlternateScreen` and `DisableMouseCapture` may leave mouse capture active in the restored terminal. Calling `disable_raw_mode()` before `LeaveAlternateScreen` can cause rendering artifacts on some terminal emulators.

**Spinner animation requires the `Tick` event.** The `⟳` spinner needs to animate even when no keyboard input arrives. Use `event::poll(Duration::from_millis(100))` in the event thread and emit a `Tick` event when it returns `Ok(false)` (timeout). The transition render loop redraws the spinner on each `Tick`. Without `poll`-based ticks, `event::read()` blocks until user input — the spinner freezes.

---

#### 2.5 TUI Theme (`crates/jig-tui/src/theme.rs`)

- Base palette: 16-color safe (works in any terminal)
- True-color enhancement: activated when `COLORTERM=truecolor` or `COLORTERM=24bit`
- Selected items: bold + accent color
- Focused pane border: bright accent; unfocused: dim
- Token count: yellow when above warn threshold, red when critical

### Research Insights — TUI Theme

**`COLORTERM` detection at runtime:**
```rust
pub fn detect_truecolor() -> bool {
    matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor") | Ok("24bit"))
}
```

**True-color palette recommendation (if truecolor active):**
```
border_focused:   Color::Rgb(86, 182, 194)   // cyan
border_unfocused: Color::Rgb(80, 80, 80)
highlight_bg:     Color::Rgb(86, 182, 194)
token_warn:       Color::Rgb(229, 192, 123)  // warm yellow
token_critical:   Color::Rgb(224, 108, 117)  // soft red
success:          Color::Rgb(152, 195, 121)  // green
failure:          Color::Rgb(224, 108, 117)
```

**`unicode-width` dependency.** Add `unicode-width = "0.1"` to `jig-tui` deps. Required for correct cell-width calculations if template/persona names contain CJK characters. ratatui uses it internally; you need it directly if doing manual string truncation (e.g., truncating long template names to fit pane width).

---

## Alternative Approaches Considered

### MCP Conflict Resolution

| Approach | Rejected Because |
|---|---|
| Last writer wins | Silently breaks session 1 mid-flight |
| First writer wins | Requires blocking UX (error/refuse to launch) |
| **Namespace by session (chosen)** | Both sessions get full functionality, zero breakage |

### Persona Merge

| Approach | Rejected Because |
|---|---|
| Full deep-merge everywhere | Complex, hard to reason about, implicit |
| Copy-paste team rules into personal override | Tedious, diverges over time |
| **Explicit `extends` (chosen)** | Opt-in, simple case stays simple, inheritance is legible |

### TUI Trigger

| Approach | Rejected Because |
|---|---|
| Conditional (TUI only if config is ambiguous) | "Sometimes TUI, sometimes not" is confusing |
| **Always TUI on bare `jig` (chosen)** | Consistent mental model; flags = headless |

### Preview Pane Depth

| Approach | Rejected Because |
|---|---|
| Tabbed (summary / full) | Adds cognitive overhead; mode switching friction |
| Expand/collapse sections | Same problem |
| **Always full system prompt (chosen)** | Right pane has space; token count anchors the summary |

### Research Insights — Alternatives

**Simplicity considerations.** A code simplicity review identified the following as candidates for Phase 2 deferral (not blocking Phase 1 delivery, but worth noting):

| Feature | Phase 1 scope | Simplification option |
|---|---|---|
| 4 layout modes | Include | Could ship as 1 responsive layout, name modes later |
| FilterableList<T> generic | Include | Could start concrete, generalize on 2nd list |
| tiktoken-rs | Feature-gate | Character heuristic is sufficient for Phase 1 |
| Session history TUI view | Keep JSONL write; defer view | TUI view is a separate feature |
| MCP session suffix + rewriting | Include (concurrent sessions are a real scenario) | Could document "one session at a time" limitation for v0 |
| Launch transition screen 500ms minimum | Include | Simplest to add; min display prevents flicker |

These are trade-offs, not mandates. The plan as written is the right target. The simplicity notes are a reminder to resist scope creep within each section.

---

## System-Wide Impact

### Interaction Graph

Launching `jig` triggers this chain:

```
jig main()
  → [--help/--version/completions: return before config I/O]
  → CLI parse (clap)
  → [TUI] user selects template+persona
  → config::resolve() — reads 4 YAML files (parallel), merges, validates per-layer
  → assembly::stage::run() — 16-step pipeline (thin sequencer in stage.rs)
      → assembly::mcp::write_atomic()
          → flock(~/.claude.json.jig.lock, LOCK_EX)
          → backup ~/.claude.json atomically (tmp+rename within flock)
          → read into serde_json::Value (not typed struct)
          → detect_conflicts() → RenameMap
          → permissions::rewrite_mcp_permissions()
          → atomic rename
          → refcount::increment() ← inside flock
          → release lock
      → skills::symlink_all() → /tmp/jig-XXXXXX/ (path jail validated)
      → env::export_all() (no credentials in history)
      → history::record_start() (pre-expansion strings only)
  → executor::fork_claude()
      → child: setpgid(0,0) → execv("claude", args)
      → parent: signal_handler_install() → waitpid(child)
  → [on exit] cleanup::run() (SessionGuard::Drop)
      → Category A (always): refcount::decrement_under_lock() → if zero: mcp::remove_entries()
      → Category A: fs::remove_dir_all(/tmp/jig-XXXXXX/)
      → Category B (clean exit): hooks::run_post_exit()
      → Category B: history::record_exit() (separate JSONL append line)
```

### Error & Failure Propagation

- **Config parse error:** `miette` rich diagnostics with source span; exits before any state is written
- **Hook approval denied:** Exits cleanly; no state written
- **Hook stderr output:** Captured and shown in miette diagnostic on failure
- **MCP write failure (Step 10):** `SessionGuard` Drop runs Category A cleanup; reports which sub-step failed and the OS error; exits non-zero
- **`claude` not found:** `bootstrap::check()` searches `$PATH` + known paths (`~/.claude/local/claude`); prompts to install; exits with specific instruction
- **`claude` exits non-zero:** jig exits with same code (transparent pass-through)
- **Signal to jig (SIGINT):** Forwarded to child pgid; jig waits for child to exit, then runs Category A cleanup
- **SIGHUP:** Same as SIGINT — forward and wait

Errors from `assembly::mcp` after the lock is taken must always release the lock, even on panic. Use `fd-lock` (replacing `fs2`) with a `Drop` impl on the lock handle.

### State Lifecycle Risks

| State | Written | Cleaned Up | Risk |
|---|---|---|---|
| `~/.claude.json` MCP entries | Step 10 (inside flock) | Cleanup: re-read refcount under flock | Orphaned if jig crashes before cleanup; `jig doctor` recovers |
| `/tmp/jig-XXXXXX/` symlinks | Step 8–9 | Category A cleanup | Orphaned temp dirs if crash; low risk (OS cleans /tmp on reboot) |
| `history.jsonl` entry | Step 13 (start); Category B (exit) | Never removed | Start without matching exit = crash indicator for `jig doctor` |
| `hook-approvals.jsonl` | On approval | Never removed (by design) | JSONL append is concurrent-safe; `jig doctor` can prune old entries |
| Refcount file | Inside flock | Inside flock | Must be canonical-path-keyed; stale count if crash; `jig doctor` validates against running PIDs |
| `~/.claude.json.jig-backup-<pid>` | Within flock (atomic) | On successful exit | Session-unique filename prevents concurrent overwrites |

### API Surface Parity

- `jig -t T` (headless) and TUI launch path must invoke the identical `assembly::stage::run()` function — no divergence
- `jig --dry-run` uses the same pipeline up through Step 11, then prints instead of forking; also shows hooks that would run (without running them)
- `jig doctor` reads the same state files (`~/.claude.json`, refcount, history) as the main pipeline
- All list/show commands support `--json` for machine-readable output

### Integration Test Scenarios

1. **Concurrent conflict:** Two test processes both call `mcp::write_atomic()` with a conflicting server name; verify one gets suffixed and both permission lists are correctly rewritten
2. **Crash recovery:** Write MCP entries, kill process before cleanup; run `jig doctor`; verify orphaned entries are detected and removed
3. **Persona extends chain:** `.jig.local.yaml` extends `.jig.yaml` persona; verify rules are merged in the right order
4. **Hook approval cache:** Same hook command appears in both `.jig.local.yaml` and a skill; verify it prompts twice (different `source`) even with identical hash
5. **Signal forwarding:** Fork mock claude that sleeps; send SIGINT to jig; verify mock claude receives SIGINT, jig waits for it, cleanup runs
6. **Refcount inside flock:** Concurrent launches; verify refcount is consistent and no session's MCP entries are prematurely removed
7. **Symlink path jail:** Config referencing `../../etc/passwd` as skill path; verify abort with clear error before any symlink is created
8. **`--dry-run` with active concurrent session:** Output shows suffixed MCP server names

### Testing Patterns (Round 2)

**flock correctness tests must use OS processes, not threads.** `flock()` is per-process. Two `std::thread` instances in the same process share the same flock state — the test would pass even if the locking code is wrong. Use `std::process::Command::new(env!("CARGO_BIN_EXE_jig"))` to spawn real child processes for concurrency tests:
```rust
let p1 = Command::new(env!("CARGO_BIN_EXE_mock_mcp_writer")).spawn()?;
let p2 = Command::new(env!("CARGO_BIN_EXE_mock_mcp_writer")).spawn()?;
// join both, verify state
```
Add a `mock_mcp_writer` integration test binary in `tests/bin/`.

**`INSTA_UPDATE=no` in CI.** Snapshot tests with `insta` should fail (not auto-update) when output changes unexpectedly. Set `INSTA_UPDATE=no` in `.github/workflows/ci.yml`. Developers run `INSTA_UPDATE=unseen cargo test` locally to approve new snapshots. Commit the `snapshots/` directory.

**`serial_test` for `std::env::set_var` tests.** Environment variable mutations are process-global and not thread-safe. Any test that calls `std::env::set_var` or `remove_var` must be annotated `#[serial]` (from the `serial_test` crate) to prevent data races with other tests in the same process.

**`proptest` regressions committed to git.** When `proptest` finds a failing case, it writes a regression file to `proptest-regressions/`. Commit this directory. Without it, CI may not replay the specific failing case that was found locally.

**`Criterion::default().without_plots()` in CI.** Criterion generates HTML plots that require gnuplot. In CI, gnuplot is rarely available. Add `.without_plots()` to avoid spurious warnings or failures. The numeric results are unaffected.

**`CARGO_BIN_EXE_*` env var for integration test binaries.** Cargo sets `CARGO_BIN_EXE_<name>` for each `[[bin]]` target at test compile time. Use this instead of hardcoding `target/debug/jig` paths:
```rust
let jig_bin = env!("CARGO_BIN_EXE_jig");
let output = Command::new(jig_bin).arg("--version").output()?;
```

---

## Acceptance Criteria

### Functional Requirements

**Phase 1 — Core CLI:**
- [ ] `jig -t base-devops` assembles and launches Claude Code with correct MCP, skills, system prompt, and permissions
- [ ] `jig --dry-run` prints the resolved `claude` command without forking or writing state
- [ ] `jig --dry-run` lists hooks that would run (with source tier labels) without executing them
- [ ] `jig --dry-run --json` outputs machine-readable JSON representation of the assembled session (stable versioned schema)
- [ ] `--json` is a global flag accepted by `jig template list`, `jig persona list`, `jig skill list`, `jig history`
- [ ] Concurrent sessions in the same directory get namespaced MCP server names (`postgres` → `postgres__jig_a3f1b2c9`)
- [ ] `allowedTools`/`disallowedTools` entries for renamed servers are correctly rewritten
- [ ] Glob-style permission entries (`mcp__postgres__*`) are also rewritten correctly
- [ ] `.jig.local.yaml` persona with `extends: project` inherits project persona rules and appends its own
- [ ] `persona.extends` in `.jig.yaml` (non-local) is rejected with a clear error
- [ ] Global config hooks prompt once on first encounter; cache approval by SHA-256 hash; re-prompt if hash changes
- [ ] Team config hooks prompt with "from team config (committed to git)" attribution
- [ ] Skill hooks prompt with skill name + source URL
- [ ] Personal hooks prompt once and cache approval
- [ ] Same hook command under a different source re-prompts even if previously approved
- [ ] `--yes` only auto-approves hooks already in the approval cache (not new external hooks)
- [ ] `~/.claude.json` write is atomic (POSIX rename) and uses `fd-lock` for mutual exclusion on a dedicated lock file
- [ ] Refcount increment is performed within the same flock as the MCP write
- [ ] MCP entries are removed on clean exit when refcount reaches zero (re-read under lock)
- [ ] `jig doctor` detects orphaned MCP entries and offers cleanup
- [ ] `jig --last` repeats the most recent complete session from `history.jsonl`
- [ ] `jig --resume` re-stages the most recent session config and passes `--resume` to claude
- [ ] SIGINT sent to jig is forwarded to the claude child process group
- [ ] Running outside a git worktree emits a visible warning
- [ ] Path traversal in skill symlinks is detected and aborted with a clear error
- [ ] `history.jsonl` never records expanded MCP args or env var values containing credentials
- [ ] `jig config set <key> <value>` modifies config at the specified scope without opening `$EDITOR`
- [ ] Default content ships: 9 templates, 10 personas, 4 default skill sources (PRD §19, Phase 1 defaults)

**Phase 2 — TUI:**
- [ ] `jig` (no args) opens TUI; `jig --go`, `jig -t T`, `jig --last`, `jig --dry-run` bypass it
- [ ] Two-pane layout: left = template+persona lists, right = live preview
- [ ] Preview shows `~` token count, skills, permissions summary, full scrollable system prompt
- [ ] Preview updates on selection change after 50ms debounce (< 50ms total for typical configs)
- [ ] `j/k` navigate, `Tab` switches pane focus, `/` enters filter mode, `Enter` launches, `d` dry-run, `?` which-key
- [ ] At <80 columns: single-pane mode with `p` to toggle preview
- [ ] At <60 columns: minimal list-only mode
- [ ] Below 40×24 terminal: error message shown instead of TUI (no raw mode entered)
- [ ] Mouse scroll wheel scrolls preview pane
- [ ] Launch transition screen shows assembly steps live; min 500ms display; per-step timing on failure
- [ ] `restore_terminal()` called before `execv("claude")`
- [ ] `h` opens session history view with relaunch capability

### Non-Functional Requirements

- [ ] `jig --help` completes in < 50ms (dispatched before any config I/O)
- [ ] `jig -t base --dry-run` completes in < 200ms
- [ ] Headless binary (`--no-default-features`) is < 5MB (CI size gate enforced)
- [ ] TUI binary is < 10MB (CI size gate enforced)
- [ ] No `clippy -D warnings` violations
- [ ] `cargo fmt --check` passes
- [ ] `cargo audit` clean (no known CVEs in dependency tree)
- [ ] All 1000 YAML fuzz inputs handled gracefully (no panics; clean `miette` errors)

### Quality Gates

- [ ] Unit tests: YAML parsing, schema migration (insta snapshots), config merge (proptest property-based), `extends` chain + cycle detection (template and persona), env var expansion, token estimation, ref counting, lock file, permission rewriting
- [ ] Integration tests: full assembly pipeline snapshot, `~/.claude.json` mutation cycle, concurrent flock (barrier-based thread test), atomic write crash recovery, fork+wait with compiled mock `claude` binary (normal/error/signal exit), signal forwarding, `--dry-run --json` output, hook approval caching, refcount-inside-flock correctness, symlink path jail
- [ ] E2E tests: `jig init`, `jig import`, `jig doctor`, YAML fuzz (proptest 10k cases)
- [ ] Performance benchmarks (`criterion` in `crates/jig-core/benches/`): config resolution < 10ms; assembly pipeline < 50ms; full launch-to-exec < 200ms (excluding Claude startup); token estimation < 20ms for 10k-token prompt

---

## Gaps & Open Questions (SpecFlow Analysis)

The following gaps were identified during SpecFlow analysis and must be resolved before or during Phase 1 implementation. Critical items block specific modules; important items affect UX and test coverage.

### Critical — Blocks Implementation

**Q1: Session suffix scheme for MCP namespace collision (blocks `assembly/mcp.rs`)**

Use 8-hex-character suffix (e.g., `jig_a3f1b2c9`) generated from `getrandom`. Generate within the flock, checking existing keys for collisions. Retry up to 32 times; hard error if exhausted. Store alongside PID in refcount file.

**Q2: Pre-launch hook failure semantics (blocks assembly pipeline Step 7)**

Abort the launch on non-zero exit. Show hook stderr with a miette diagnostic indicating which hook failed, its exit code, and its source tier. Exit jig non-zero. Do not write any state after Step 7.

**Q3: Non-TTY behavior for all interactive prompts (blocks headless + CI use)**

Add `--non-interactive` / `--yes` CLI flags. `--yes` auto-approves only cached items. In non-TTY contexts without `--yes`, auto-deny new approvals and exit with a descriptive error. In non-TTY with `--yes`, approve cached items only; new external hooks still error.

**Q4: Persona `extends` cycle detection (Phase 1 deliverable gap)**

Apply DFS visited-set cycle detection to persona `extends` (single-parent). Error includes the full cycle path. Add unit tests with fixtures for direct cycles.

**Q5: MCP approval cache hash input (blocks `assembly/mcp.rs` + approval cache)**

Hash the **pre-expansion** server definition (command + unexpanded arg strings). Same rule applies to hook approval hashes. Document in code: "SHA-256 of the UTF-8 bytes of the command string as it appears in the config file, before `${ENV_VAR}` substitution." Masked current value shown at approval time.

**Q6: `~/.claude.json.jig-backup` — single file or rotation?**

Use session-unique backup names (`~/.claude.json.jig-backup-<pid>`). Clean up own backup on successful exit; leave on error for `jig doctor` recovery. `jig doctor --restore-backup` shows backup metadata and requires confirmation. Add HMAC or SHA-256 digest alongside backup for integrity verification.

### Important — UX and Coverage

**Q7: Dependency check placement in TUI flow**

Run the check before the TUI opens. Missing dependencies show a pre-TUI install prompt. Templates with missing deps show a badge in the TUI list (Phase 2+ enhancement).

**Q8: Empty template list in TUI**

Show a placeholder: "No templates found. Run `jig init` to create one." `Enter` navigates to `jig init`.

**Q9: `e` (edit) key in Phase 1 TUI**

In Phase 1, `e` opens the relevant config file in `$EDITOR`. If `$EDITOR` is unset, show tooltip. Replaced by Editor Mode in Phase 2.

**Q10: Terminal height guard for TUI**

Detect terminal size before entering raw mode. If height < 24 or width < 40, display: "Terminal too small (minimum 40×24). Resize and try again." and exit cleanly without entering raw mode.

**Q11: Post-exit hooks on interrupted (SIGINT) exit**

Always run post-exit hooks unless jig itself receives SIGKILL. Document this behavior. Category B cleanup runs after waitpid regardless of whether child exited via signal.

**Q12: `jig --dry-run` with active concurrent session**

Simulate the suffix collision and show the suffixed server names in the output. Add test case.

### Additional Open Questions (Q13–Q30) — New from Spec Flow Analysis

**Q13: Template `extends` cycle detection algorithm**

Use DFS with grey/white/black visited sets on the full resolution graph (array-based multi-parent support). Error format: `Circular extends: template-a → base → template-a`. Missing base is a separate error. Fixtures needed for direct, indirect, and missing-base cases.

**Q14: `~/.claude.json` structural drift when Claude Code updates schema**

Use `serde_json::Value` for the entire file — never deserialize through a typed struct. Extract/inject only `projects."<cwd>".mcpServers` by JSON path. All other keys preserved by construction. If the `projects` key is absent or its value is not an object, warn and initialize it, do not fail.

**Q15: Concurrent `jig doctor` runs — TOCTOU on orphan cleanup**

`jig doctor --fix` must acquire the flock at the start of any cleanup operation and re-validate PID state while holding the lock before removing any entry. Two concurrent doctor instances: second one re-checks under the lock; entries already removed are skipped.

**Q16: Race between cleanup and new session starting in the same directory**

Cleanup must re-read refcount under the flock, not trust the in-memory decrement. If re-read count > 0, skip MCP entry removal. This prevents the race: session A exits (refcount 1→0 in-memory), session B starts and merges before A acquires the lock, A's cleanup removes B's entries.

**Q17: `jig --last` with corrupted or truncated `history.jsonl`**

Scan from end of file upward. Skip lines that fail JSON parse or are missing `template`/`assembled_flags` fields. If no valid complete entry found within last 50 lines, error: "No valid previous session found. History may be corrupted — run `jig doctor` to inspect."

**Q18: Skill source URL validation and network error handling during `jig sync`**

Validate URL format at config parse time (must match `https://`, `git://`, or `git@...`). Set 30-second timeout on git operations. On partial sync failure (3 sources succeed, 1 fails): continue past failures, collect errors, report all at end. On launch with missing skill cache: hard error with "run jig sync" suggestion.

**Q19: `claude` binary not found — PATH resolution and fallbacks**

Search `$PATH` first, then check `~/.claude/local/claude` and `/usr/local/bin/claude` as fallbacks. If found outside PATH, warn and use found path but suggest adding to PATH. Run `claude --version` at Step 1 to check version compatibility against a minimum required version in jig.

**Q20: `claude` binary updated mid-session — CLI flag interface changes**

Run `claude --version` at Step 1 and parse it. Maintain a `CLAUDE_MIN_VERSION` constant in jig. If installed version is below minimum, error with a specific upgrade instruction before assembling any flags. If `claude` exits non-zero with "unknown flag", re-surface the error with context about which jig-assembled flag was rejected.

**Q21: Multi-machine scenario — `.jig.lock` committed, different machines with different cached skills**

When local skill cache hash differs from `.jig.lock` hash, treat as missing skill with a differentiated error message: "Skill 'composio/docker' is cached but at wrong commit. Run `jig sync` to update." Do not auto-run sync. `jig sync --frozen` on CI should fail if any source needs updating (cache is behind lock).

**Q22: Shell completion edge cases**

Completions have a hard 100ms timeout. Return empty on any error (no stderr output in completion context). Source from global + current-directory config. CWD-aware: complete from `.jig.yaml` templates when in a project directory.

**Q23: Schema version migration on first-run (no existing global config)**

Default templates and personas are embedded via `include_str!` and available without `jig init`. `jig init` is enhancement, not prerequisite. Schema migration backup uses timestamped name: `config.yaml.v1-backup-20260325T100000Z`.

**Q24: Corrupted `hook-approvals.jsonl`**

A line that fails JSON parse is skipped (logged as warning). Treat as "approval lost" for that entry — re-prompt on next encounter. `jig doctor` flags corrupted lines and offers to remove them. Do not fail launch on a corrupted approvals file.

**Q25: `--resume` semantics**

`--resume` uses the most recent complete entry from `history.jsonl` for config (same as `--last`) and additionally passes `--resume` to claude. Combining `--resume` with `-t T` is an error.

**Q26: Env var expansion security in MCP args**

At minimum, warn when an expanded value contains shell metacharacters (`;`, `&&`, `||`, backtick, `$(...)`). Never record expanded values in history. Show masked current value at approval time. `jig doctor --audit` warns when a pre-expansion string contains a substitution that resolves to a non-localhost host.

**Q27: `jig diff <config>` — semantics**

`jig diff <other-config-file>` diffs the resolved config of `<other-config-file>` against the current working directory's resolved config. Output is a structured diff of resolved fields, not raw YAML. Supports `--json`.

**Q28: `jig import <url>` — hook trust for imported templates**

Imported templates from a URL are subject to the same hook approval flow as synced skills (External tier). The source URL is stored in the imported template for attribution. Prompts before writing hooks to global config scope regardless of target scope selected.

**Q29: `history.jsonl` retention enforcement**

Retention enforcement runs on jig startup (after config load, before TUI opens). If `history_retention_days` is set and the file has entries older than the threshold, prune them on startup. Cap at 100ms for the prune operation; skip if file is locked.

**Q30: TUI filter mode — no-match and clear-input behavior**

Zero matches: show "No results" placeholder in list area. Clear-input (backspace to empty): restore full list immediately (no Esc needed). Filter applies to both template name and description field. Algorithm: fuzzy (nucleo-matcher), not substring.

### Additional Test Cases (from SpecFlow analysis)

- `jig -t nonexistent` → miette error with did-you-mean suggestion from available templates
- `persona.ref: nonexistent` → miette error listing available persona names
- Three concurrent sessions, same server name → all three get unique suffixes, all `allowedTools` entries correctly rewritten
- Wildcard permission entry `mcp__postgres__*` rewritten correctly after server rename
- `jig --last` with empty `history.jsonl` → clear error "No previous session found."
- `jig --last` with corrupted `history.jsonl` → scans upward, skips invalid lines, clear error if none found
- Mount path that does not exist → error at assembly time (fail fast, before writing any state)
- `jig --dry-run --json` with active concurrent session → JSON output reflects suffixed server names
- Template extends cycle (direct) → clean miette error with full cycle path
- Template extends cycle (indirect) → same
- Skill symlink path traversal → abort with "Skill path resolves outside allowed directory" error
- `hook-approvals.jsonl` corrupted line → skipped with warning; subsequent valid approvals work
- Refcount race: concurrent sessions decrement to 0 simultaneously → only one removes MCP entries

---

## Observability Plan

### Structured Logging

Add `tracing` to `jig-core` and `tracing-subscriber` to `jig-cli`. Initialize subscriber in `main.rs` based on `--verbose` count (or `RUST_LOG`):
- `-v` = `INFO`
- `-vv` = `DEBUG`
- `-vvv` = `TRACE`

Zero overhead when subscriber is not initialized. Does not add to binary size when the feature is disabled.

### Config Resolution Trace

In `--verbose` / `-v` mode, trace which layer set each resolved field:
```
[DEBUG] persona: .jig.local.yaml sets extends: project
[DEBUG] persona: merging base from .jig.yaml (name: strict-security, 3 rules)
[DEBUG] mcp: team config adds server "postgres" (command: npx @mcp/server-postgres)
[DEBUG] mcp: no conflicts detected
```
Add `jig config show --explain` (or include a `resolution_trace` field in `--dry-run --json`) showing per-field provenance.

### `--dry-run --json` Schema (stable, versioned)

The JSON output must be a stable, versioned contract:
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
  "token_breakdown": {"persona": 800, "fragments": [{"name": "code-standards.md", "tokens": 1400}]},
  "hooks": {"pre_launch": [{"command": "python scripts/pull.py", "source": "team", "tier": "team"}]},
  "warnings": [{"code": "CONCURRENT_SESSION", "message": "Another jig session is active in this directory"}],
  "env_vars": ["DATABASE_URL", "OPENAI_KEY"]  // names only, not values
}
```

### `history.jsonl` Schema

Start record (Step 13):
```json
{"type":"start","session_id":"uuid","started_at":"2026-03-25T10:00:00Z","template":"base-devops","persona":"strict-security","cwd":"/Users/jforsythe/dev/project","mcp_servers":["postgres__jig_a3f1b2c9"],"skills":["docker","k8s"],"concurrent_sessions":[],"worktree":true}
```
Exit record (Category B cleanup, separate appended line):
```json
{"type":"exit","session_id":"uuid","exit_code":0,"duration_ms":14523,"ended_at":"2026-03-25T10:14:34Z","hook_results":[{"hook":"pre_launch","command":"python scripts/pull.py","exit_code":0,"duration_ms":1200}]}
```
A start record with no matching exit record = crash indicator. `jig doctor` flags these.

### `jig sync` Per-Source Timing

```
Syncing skill sources...
  composio     updated  231 skills  2.3s
  team         up-to-date          0.1s
  local        n/a (no remote)
Sync complete. 231 skills updated.
```

### Observability Additions (Round 2)

**All tracing events go to stderr, never stdout.** stdout is used for machine-readable output (`--json`, `--dry-run --json`). Mixing `tracing` output with structured JSON would break agent parsing. Configure `tracing-subscriber` to write to `std::io::stderr()` unconditionally.

**Expanded `history.jsonl` exit record schema:**
```json
{
  "type": "exit",
  "session_id": "uuid",
  "exit_code": 0,
  "duration_ms": 14523,
  "ended_at": "2026-03-25T10:14:34Z",
  "jig_version": "1.0.0",
  "token_count_estimate": 2400,
  "token_count_method": "heuristic",
  "fragment_count": 3,
  "hook_results": [
    {"hook": "pre_launch", "command": "python scripts/pull.py", "source": "team", "exit_code": 0, "duration_ms": 1200}
  ]
}
```
Note: `source` field in each `hook_results` entry distinguishes hook origin for post-session auditing.

**Tail-first reading for `jig history`.** `history.jsonl` grows unboundedly. `jig history` (showing recent sessions) should read from the end of the file, not the beginning. Use `std::io::Seek` to position near the end and scan backwards for newlines. For `jig history --limit 20`, reading the last ~10KB is sufficient. This avoids loading a large history file into memory on every invocation.

**Synthetic exit records for crash detection.** When `jig doctor` finds a start record with no matching exit record, it writes a synthetic exit:
```json
{"type":"exit","session_id":"uuid","exit_code":null,"reason":"crash_detected","detected_at":"2026-03-25T12:00:00Z"}
```
This closes the gap in history for clean `jig history --json` output (no dangling `"status": "crash_or_running"` entries in old sessions).

**`approval_status` in `--dry-run --json` output.** Add per-hook approval status to the dry-run output:
```json
"hooks": {
  "pre_launch": [
    {
      "command": "python scripts/pull.py",
      "source": "team",
      "tier": "team",
      "approval_status": "cached",    // "cached" | "would_prompt" | "auto_approved"
      "last_approved_at": "2026-03-24T09:00:00Z"
    }
  ]
}
```
This lets agents verify that all hooks are pre-approved before running jig non-interactively.

**`resolution_trace` field in `--dry-run --json`.** Already planned in Config Resolution; replicate here as the canonical location in the output schema:
```json
"resolution_trace": {
  "persona.name": "PersonalLocal",
  "persona.extends": "PersonalLocal",
  "mcp.postgres": "TeamProject",
  "profile.skills": ["GlobalUser", "TeamProject", "PersonalLocal"]
}
```

---

## Agent-Native Parity

`jig` is currently ~75% agent-native. The following gaps prevent an agent from performing all the actions a user can perform.

### Parity Gaps

| User Action | CLI Equivalent | Status |
|---|---|---|
| Select and launch template+persona | `jig -t T -p P` | Done |
| Relaunch most recent session | `jig --last` | Done |
| Relaunch arbitrary history entry | None | Gap — add `jig --session <UUID>` (replaces `--last-id <N>`) |
| Preview assembled config | `jig --dry-run` | Done |
| Preview as machine-readable JSON | `jig --dry-run --json` | Partial — needs stable schema + `resolution_trace` |
| List available templates | `jig template list` | Partial — needs `--json` |
| List available personas | `jig persona list` | Partial — needs `--json` |
| List installed skills | `jig skill list` | Partial — needs `--json` |
| Edit config scalar fields | `$EDITOR` only | Gap — add `jig config set` |
| Add/remove array config fields | None | Gap — add `jig config add` / `jig config remove` |
| View session history (joined) | `jig history` | Gap — needs `--json` with joined start+exit objects |
| Ask "what config is this session using?" | None | Gap — jig MCP server |
| Modify next-session config from within session | None | Gap — jig MCP server |

### Recommended Additions

1. **`jig config set <key> <value> [--scope local|project|global]`** — dotted path notation, machine-consumable config mutation. Highest-priority gap.
2. **`jig config add <key> <value>` / `jig config remove <key> <value>`** — for array fields (`profile.skills`, `context.fragments`). Without these, agents must parse and rewrite raw YAML.
3. **`--json` as global flag** — all list/show commands emit structured JSON to stdout, no human text mixed in.
4. **Stable `--dry-run --json` schema** — versioned JSON contract (see Observability Plan above) including `resolution_trace` and `approval_status`.
5. **`jig --session <UUID>`** — relaunch history entry by session UUID. Replaces `--last-id <N>` (positional index is fragile across concurrent writes).
6. **`jig serve --mcp`** — expose jig as an MCP server so Claude Code sessions can query their own config, list templates, and modify the next session's config. Recommended for Phase 5.
7. **Stdin config assembly** — `jig --dry-run --json --config -` reads YAML config from stdin for ad-hoc agent use without touching the filesystem.

### MCP Tool Surface (Round 2 — expanded to 14 tools)

The original plan specified 5 MCP tools. Round 2 research identified the full required set:

| Tool | Description |
|---|---|
| `jig_get_active_config` | Return resolved config for the current session |
| `jig_list_templates` | List available templates with metadata |
| `jig_list_personas` | List available personas with metadata |
| `jig_list_skills` | List installed skills with source info |
| `jig_get_template` | Get full details for a named template |
| `jig_get_persona` | Get full details for a named persona |
| `jig_write_config_field` | Set a config field at a given scope (replaces `jig_set_template`) |
| `jig_add_config_array_item` | Append to an array config field |
| `jig_remove_config_array_item` | Remove from an array config field |
| `jig_dry_run` | Return assembled session as JSON without launching |
| `jig_get_history` | Return joined session history objects |
| `jig_get_session` | Get details for a specific session by UUID |
| `jig_sync` | Trigger skill source sync |
| `jig_get_capabilities` | Return jig version, installed features, MCP tool list |

**All MCP write tools must go through `flock`.** Any `jig_write_config_field` or `jig_add_config_array_item` call that modifies `~/.claude.json` must acquire the same `~/.claude.json.jig.lock` flock used by the launch path. Never write to state files from the MCP server without the lock.

**Expose a `jig-capabilities.md` fragment.** When jig launches a session, inject a `jig-capabilities.md` context fragment describing the available MCP tools and their schemas. This enables Claude Code (within the session) to discover and use the MCP tools without out-of-band documentation.

**Use stdio transport for the MCP server.** `jig serve --mcp` should communicate over stdin/stdout (stdio transport), not a TCP port. This avoids port conflicts on shared machines and works correctly in all environments where jig runs.

---

## Dependencies & Prerequisites

| Crate | Version | Role | Notes |
|---|---|---|---|
| `clap` (derive) | 4.5.x | CLI argument parsing | Use `["std", "derive"]` features; disable unicode if ASCII-only args |
| `serde` + `serde_json` | latest | Config serialization + claude.json handling | serde_json for claude.json roundtrip (raw Value) |
| `figment` | latest | Config file loading + 4-layer merge | **Replaces `serde_yaml`** — serde_yaml archived 2024, YAML 1.1 foot-guns |
| `serde-yaml-ng` | latest | Alternative to figment if direct YAML API needed | Community fork of archived serde_yaml |
| `miette` + `thiserror` | 7.x + 2.x | Rich error diagnostics | Use `fancy` feature in binary only; thiserror 2.x required for miette 7 compat |
| `tempfile` | 3.10.x | Temp directory management | — |
| `fd-lock` | latest | Cross-platform file locking | **Replaces `fs2`** — fs2 abandoned since 2016 |
| `ratatui` + `crossterm` | 0.28.x + 0.27.x | TUI framework (Phase 2, feature-gated) | ratatui is community fork of tui-rs (do not use tui-rs) |
| `nucleo-matcher` | latest | TUI list filtering | **Replaces `fuzzy-matcher`** — fuzzy-matcher stagnant; nucleo-matcher is best-in-class |
| `pulldown-cmark` | 0.12.x | Markdown rendering in preview pane | Use `simd` feature for large prompts |
| `dirs` replacement: `directories` | latest | XDG directory resolution | **Replaces `dirs`** — `directories::ProjectDirs` gives correct namespaced paths per platform |
| `sha2` | latest | SHA-256 for approval cache hashing | **Use `sha2` (RustCrypto) directly** — not the `sha256` wrapper crate |
| `getrandom` | latest | Session suffix generation | **Replaces `rand`** — just 4 random bytes needed; getrandom is minimal |
| `signal-hook` | latest | POSIX signal handling (SIGINT/SIGTERM/SIGHUP forwarding) | Purpose-built; smaller than pulling in all of `nix` |
| `scopeguard` | 1.2.x | `defer!` macro for cleanup | No stdlib equivalent; effectively done/stable |
| `tracing` + `tracing-subscriber` | latest | Structured logging | `tracing` in `jig-core`; `tracing-subscriber` initialized in `jig-cli` only |
| `unicode-width` | 0.1.x | TUI cell-width for CJK chars | Required in `jig-tui` |
| `hex` | 0.4.x | Hex-encoding session suffix | Minimal |
| `proptest` | 1.5.x | Property-based testing | `[dev-dependencies]` only |
| `insta` | 1.39.x | Snapshot testing | `[dev-dependencies]` only; use `yaml` feature |
| `criterion` | 0.5.x | Performance benchmarks | `[dev-dependencies]` only; benches in `crates/jig-core/benches/` |

**Removed from original list:**
- `fs2` → replaced by `fd-lock`
- `serde_yaml` → replaced by `figment` or `serde-yaml-ng`
- `dirs` → replaced by `directories`
- `fuzzy-matcher` → replaced by `nucleo-matcher`
- `tiktoken-rs` → feature-gated behind optional `tokens` feature; character heuristic is default

**External runtime dependency:** `claude` binary must be installed and on `$PATH` (or at known fallback paths). `jig bootstrap` checks, falls back to known paths, and guides installation. Minimum claude version checked at Step 1.

### Dependency Additions (Round 2)

| Crate | Version | Role | Notes |
|---|---|---|---|
| `nix` | 0.29.x | `fork()`, `execvp()`, `setpgid()`, `waitpid()`, `killpg()` | Use alongside `signal-hook`; provides POSIX primitives that `signal-hook` alone does not cover |
| `ureq` | 3.x | HTTP client for `jig sync` skill fetching | **Replaces `reqwest`** — synchronous, ~0.5MB vs ~3MB; no tokio dependency needed |
| `rustc-hash` | 2.x | Fast non-cryptographic hash for TUI preview cache | `FxHasher` for content-change detection in `jig-tui`; security hashes stay as `sha2` |
| `libc` | 0.2.x | `libc::_exit(127)` after failed exec in child | Minimal; already an indirect dep of most crates |

**Added to existing entries:**
- `ratatui` version bumped to `0.29.x` — 0.29 has breaking API changes (`frame.area()`, non-generic `Frame`); pin to `0.29.x` not `0.28.x`
- `fd-lock` — note added: guard MUST be dropped before `exec`; open lock file with `O_CLOEXEC` or ensure drop before exec path

---

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| `~/.claude.json` schema changes between Claude Code versions | Medium | High | Use raw `serde_json::Value` roundtrip — only touch `projects."<cwd>".mcpServers`; detect unexpected structure and warn |
| Refcount corruption from concurrent launch/cleanup race | Medium | High | Refcount read/write must be inside same flock as MCP write; re-read under lock before acting |
| `fd-lock` not available on NFS (some Linux filesystems) | Low | Medium | Fall back to `.jig.lock` advisory lock file with PID; warn user |
| `tiktoken-rs` inflating headless binary above 5 MB | High | High | Feature-gate behind optional `tokens` feature; use character heuristic by default |
| `serde_yaml` (if retained) YAML 1.1 foot-guns | High | Medium | Replace with `figment` or `serde-yaml-ng` |
| `ratatui` terminal state not restored on panic | Low | High | Install panic hook before `init_terminal()`; also use `scopeguard::defer!` |
| Permission rewrite misses a glob pattern format | Medium | Medium | Comprehensive test suite covering `mcp__*__*`, `mcp__server__*`, exact matches |
| Supply-chain attack via external skill hook + `--yes` CI | Medium | High | Scope `--yes` to cache-only; external hooks require first-run manual approval |
| CWD hash collision (symlinks, mounts) | Low | Medium | Canonicalize path before hashing; store canonical path in refcount file |
| `hook-approvals.jsonl` concurrent write corruption | Medium | Medium | Use JSONL append (atomic for records < PIPE_BUF); do not use JSON read-modify-write |
| History credentials leak (expanded MCP args) | Medium | High | Store pre-expansion template strings only; set 0600 permissions on history.jsonl |
| **fd-lock fd inherited by claude child** | **High** | **High** | **Drop lock guard before `exec`; claude holds exclusive flock for session duration, blocking all concurrent jig instances** |
| **Hook execution shell injection** | **High** | **High** | **`exec: []` array is default; `shell: true` is explicit opt-in; `command: string` without `shell: true` is an error** |
| **`ApprovalUi` coupling — core pulls in TUI types** | **Medium** | **High** | **Define `ApprovalUi` trait in `jig-core`; implement in `jig-cli` and `jig-tui` separately** |
| **`PreviewData` ratatui type leak into `jig-core`** | **Medium** | **High** | **`system_prompt_lines: Vec<String>` in `jig-core`; convert to `Vec<Line<'static>>` in `jig-tui` only** |
| **`panic = "abort"` skips Drop** | **Medium** | **High** | **Panic hook must run Category A cleanup (MCP entry removal) before aborting** |
| **TOCTOU on global config ownership check** | **Low** | **Medium** | **Use open-then-fstat; never stat-then-open** |
| **`jig sync` git clone submodule attack** | **Low** | **High** | **Always pass `--no-recurse-submodules`; clone to temp dir, validate, then rename** |
| ratatui 0.29 API breaks existing code | High | Medium | Use `frame.area()` not `frame.size()`; pin to `ratatui = "0.29"` explicitly; audit before writing TUI code |

---

## Future Considerations

The following are explicitly out of scope for Phase 1 and Phase 2 (brainstorm YAGNI list):

- Collaborative/team TUI (session sharing, pairing)
- Plugin marketplace browsing in TUI
- Session recording/replay
- Custom TUI themes (beyond 16-color + true-color enhancement)

Future phases from PRD:
- **Phase 3:** Skill registry + sync (`jig sync`, `sources:` in global config, git-backed skill repos)
- **Phase 4:** Bootstrapping + team features (`jig init`, `jig import`, `jig doctor --fix`)
- **Phase 5:** Ecosystem (plugins, CI/CD integration, packaging, completions, `jig serve --mcp`)

---

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-03-25-jig-technical-and-ux-brainstorm.md](../brainstorms/2026-03-25-jig-technical-and-ux-brainstorm.md)

  Key decisions carried forward:
  1. MCP conflict → namespace by session suffix + permission rewrite (brainstorm §1)
  2. Persona → explicit `extends` syntax in `.jig.local.yaml` only (brainstorm §2)
  3. Hooks → 4-tier trust with source-aware prompting + approval cache (brainstorm §3)
  4. TUI trigger → always show on bare `jig`, flags bypass (brainstorm §4)
  5. TUI layout → two-pane split with live preview (brainstorm §5)
  6. Preview depth → always full composed system prompt, scrollable (brainstorm §6)
  7. Launch transition → brief status screen, min 500ms, stays on failure (brainstorm §7)

### Internal References

- PRD: [docs/jig-prd-v0.6.0.md](../jig-prd-v0.6.0.md)
  - Core concepts + vision: lines 1–115
  - MCP protocol + atomic write: lines 119–178
  - Full config schema: lines 214–322
  - Merge semantics table: lines 359–369
  - Global config schema: lines 391–443
  - Directory layout: lines 447–504
  - Assembly pipeline (16 steps): lines 512–583
  - Security model + hook tiers: lines 693–734
  - TUI design: lines 840–896
  - CLI interface spec: lines 899–941
  - Crate layout + dependencies: lines 988–1138

### Research Agents (2026-03-25 deepening — Round 1)

- Rust CLI & ratatui best practices
- Architecture strategist
- Security sentinel
- Performance oracle
- Data integrity guardian
- Spec flow analyzer (Q13–Q30)
- Testing patterns
- Dependency analyst
- Agent-native architecture
- Observability reviewer
- Code simplicity reviewer
- ratatui/crossterm patterns

### Research Agents (2026-03-25 deepening — Round 2)

- fd-lock lifecycle + O_CLOEXEC fd inheritance
- figment layered config + `.admerge()` vs `.merge()`
- nucleo-matcher 0.3 lifecycle + `FilterableListState` implementation
- POSIX signal handling: `nix 0.29` + `signal-hook` + `setpgid`/`killpg`/`_exit`
- ratatui 0.29 API changes: `frame.area()`, non-generic `Frame`, `Layout::areas()`
- miette 7 + thiserror 2: `#[diagnostic(transparent)]`, `NamedSource`, `#[from]` changes
- Security sentinel (Round 2): O_CLOEXEC, TOCTOU, hook execution model, expanded credential masking
- Testing patterns: `INSTA_UPDATE=no`, `CARGO_BIN_EXE_*`, flock tests with OS processes, `serial_test`
- Architecture strategist (Round 2): `ApprovalUi` trait boundary, `PreviewData` type, `HookTrustTier` enum
- Agent-native parity (Round 2): `--session UUID`, 14 MCP tools, `resolution_trace`, `jig-capabilities.md`
- Observability reviewer (Round 2): stderr-only tracing, `token_count_method`, tail-first reading, synthetic exit records
- Performance oracle (Round 2): `[profile.release-headless]`, `FxHasher`, `ureq`, nucleo pre-population, `cargo bloat`
- ratatui + crossterm framework docs
- Code simplicity reviewer
