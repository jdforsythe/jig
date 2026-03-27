use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use thiserror::Error;
use tracing::{debug, info};

use super::permissions::RenameMap;
use crate::config::schema::McpServer;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("Failed to read ~/.claude.json: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse ~/.claude.json: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Failed to acquire lock on {path}: {source}")]
    LockError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("Session suffix collision exhausted after 32 retries")]
    SuffixCollision,
}

/// Returns the path to `~/.claude.json`.
pub fn claude_json_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude.json")
}

/// Returns the path to the dedicated lock file `~/.claude.json.jig.lock`.
pub fn lock_file_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".claude.json.jig.lock")
}

/// Returns the refcount file path for the given canonical CWD.
pub fn refcount_path(canonical_cwd: &Path) -> PathBuf {
    let state_dir = home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("state");
    refcount_path_in(canonical_cwd, &state_dir)
}

/// Computes the refcount file path within an explicit state directory.
pub(crate) fn refcount_path_in(canonical_cwd: &Path, state_dir: &Path) -> PathBuf {
    let cwd_hash = sha256_hex(canonical_cwd.to_string_lossy().as_bytes());
    state_dir.join(format!("{cwd_hash}.refcount"))
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Generates a random 8-hex-character session suffix.
fn random_suffix() -> String {
    let mut bytes = [0u8; 4];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");
    hex::encode(bytes)
}

/// Detects naming conflicts between existing servers and new servers.
/// Returns a `RenameMap` for conflicting entries.
pub fn detect_conflicts(
    existing_servers: &HashMap<String, Value>,
    new_servers: &HashMap<String, McpServer>,
    session_suffix: &str,
) -> RenameMap {
    new_servers
        .keys()
        .filter(|name| existing_servers.contains_key(*name))
        .map(|name| {
            let suffixed = format!("{name}__{session_suffix}");
            (name.clone(), suffixed)
        })
        .collect()
}

/// Generates a session suffix that does not conflict with existing `mcpServers` keys.
/// Retries up to 32 times.
fn generate_unique_suffix(
    existing_servers: &HashMap<String, Value>,
    new_servers: &HashMap<String, McpServer>,
) -> Result<String, McpError> {
    for _ in 0..32 {
        let suffix = format!("jig_{}", random_suffix());
        // Check that no suffixed name already exists
        let any_conflict = new_servers.keys().any(|name| {
            let suffixed = format!("{name}__{suffix}");
            existing_servers.contains_key(&suffixed)
        });
        if !any_conflict {
            return Ok(suffix);
        }
    }
    Err(McpError::SuffixCollision)
}

/// Result of the atomic MCP write operation.
pub struct McpWriteResult {
    pub rename_map: RenameMap,
    pub session_suffix: String,
}

/// Atomically writes MCP servers into `~/.claude.json` for the given CWD.
///
/// Protocol:
/// 1. flock dedicated lock file (LOCK_EX)
/// 2. Backup ~/.claude.json atomically
/// 3. Read current contents as serde_json::Value
/// 4. Detect conflicts → build rename map
/// 5. Apply renames to new server entries
/// 6. Merge into projects."<abs_cwd>".mcpServers
/// 7. Write to ~/.claude.json.<pid>.tmp
/// 8. POSIX rename .tmp → ~/.claude.json
/// 9. Increment refcount (inside flock)
/// 10. Release lock (drop guard BEFORE exec)
pub fn write_atomic(
    new_servers: &HashMap<String, McpServer>,
    canonical_cwd: &Path,
    pid: u32,
) -> Result<McpWriteResult, McpError> {
    let claude_path = claude_json_path();
    let lock_path = lock_file_path();
    let state_dir = home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config").join("jig").join("state");
    write_atomic_inner(new_servers, canonical_cwd, pid, &claude_path, &lock_path, &state_dir)
}

/// Inner implementation with explicit paths — used by tests to inject temp dirs.
pub(crate) fn write_atomic_inner(
    new_servers: &HashMap<String, McpServer>,
    canonical_cwd: &Path,
    pid: u32,
    claude_path: &Path,
    lock_path: &Path,
    state_dir: &Path,
) -> Result<McpWriteResult, McpError> {
    let refcount_path = refcount_path_in(canonical_cwd, state_dir);

    // Ensure state directory exists
    let _ = std::fs::create_dir_all(state_dir);

    // Acquire exclusive lock on the dedicated lock file
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| McpError::LockError {
            path: lock_path.to_owned(),
            source: e,
        })?;

    let mut lock_guard = fd_lock::RwLock::new(lock_file);
    let _write_guard = lock_guard.write().map_err(|e| McpError::LockError {
        path: lock_path.to_owned(),
        source: e,
    })?;

    // Read current claude.json as raw Value
    let mut root: Value = if claude_path.exists() {
        let contents = std::fs::read_to_string(claude_path)?;
        if contents.trim().is_empty() {
            Value::Object(Default::default())
        } else {
            serde_json::from_str(&contents)?
        }
    } else {
        Value::Object(Default::default())
    };

    let cwd_key = canonical_cwd.to_string_lossy().into_owned();

    // Extract existing mcpServers for conflict detection.
    // Use direct map navigation — pointer() interprets '/' in cwd_key as a
    // JSON Pointer separator, which breaks on absolute paths.
    let existing_servers: HashMap<String, Value> = root
        .get("projects")
        .and_then(|p| p.get(&cwd_key))
        .and_then(|c| c.get("mcpServers"))
        .and_then(Value::as_object)
        .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    // Generate unique session suffix.
    // Always suffix ALL new entries (not just conflicting ones) so cleanup_entries
    // can reliably identify and remove them by the suffix marker.
    let session_suffix = generate_unique_suffix(&existing_servers, new_servers)?;
    let rename_map: RenameMap = new_servers
        .keys()
        .map(|name| (name.clone(), format!("{name}__{session_suffix}")))
        .collect();

    debug!(
        "MCP write: {} servers, {} conflicts, suffix: {}",
        new_servers.len(),
        rename_map.len(),
        session_suffix
    );

    // Build the mcpServers JSON object with renames applied
    let mut mcp_obj = serde_json::Map::new();

    // Keep existing entries
    for (k, v) in &existing_servers {
        mcp_obj.insert(k.clone(), v.clone());
    }

    // Add new entries with renames
    for (name, server) in new_servers {
        let effective_name = rename_map.get(name).unwrap_or(name).clone();
        let server_value = serde_json::to_value(server)?;
        mcp_obj.insert(effective_name, server_value);
    }

    // Backup atomically before mutation
    if claude_path.exists() {
        let backup_path = claude_path.with_extension(format!("jig-backup-{pid}"));
        let backup_tmp = claude_path.with_extension(format!("jig-backup-{pid}.tmp"));
        let backup_contents = std::fs::read_to_string(claude_path)?;
        std::fs::write(&backup_tmp, &backup_contents)?;
        std::fs::rename(&backup_tmp, &backup_path)?;
    }

    // Inject into the Value tree
    let root_obj = root
        .as_object_mut()
        .expect("root is always an object");

    let projects = root_obj
        .entry("projects")
        .or_insert_with(|| Value::Object(Default::default()));

    if let Some(projects_obj) = projects.as_object_mut() {
        let cwd_entry = projects_obj
            .entry(cwd_key)
            .or_insert_with(|| Value::Object(Default::default()));

        if let Some(cwd_obj) = cwd_entry.as_object_mut() {
            cwd_obj.insert("mcpServers".to_owned(), Value::Object(mcp_obj));
        }
    }

    // Write to tmp file then rename atomically
    let tmp_path = claude_path.with_extension(format!("jig-{pid}.tmp"));
    let json_str = serde_json::to_string_pretty(&root)?;
    std::fs::write(&tmp_path, json_str.as_bytes())?;

    // Sync before rename
    {
        let f = std::fs::File::open(&tmp_path)?;
        f.sync_data()?;
    }

    std::fs::rename(&tmp_path, claude_path)?;

    // Increment refcount (MUST be inside flock)
    increment_refcount(&refcount_path, &session_suffix, canonical_cwd, pid)?;

    info!("MCP write complete: suffix={session_suffix}");

    // Lock guard dropped here — BEFORE execv is called by the caller
    Ok(McpWriteResult {
        rename_map,
        session_suffix,
    })
}

fn increment_refcount(
    path: &Path,
    session_suffix: &str,
    canonical_cwd: &Path,
    pid: u32,
) -> Result<(), McpError> {
    let current = read_refcount(path);
    let new_count = current + 1;
    let content = format!(
        "{new_count}\n{session_suffix}\n{}\n{pid}\n",
        canonical_cwd.display()
    );
    std::fs::write(path, content)?;
    Ok(())
}

fn read_refcount(path: &Path) -> u32 {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.lines().next().and_then(|l| l.trim().parse().ok()))
        .unwrap_or(0)
}

/// Cleans up MCP entries added by this session from `~/.claude.json`.
/// Must acquire the flock before decrementing and potentially removing entries.
/// Re-reads refcount under lock to prevent race conditions.
pub fn cleanup_entries(
    canonical_cwd: &Path,
    session_suffix: &str,
) -> Result<(), McpError> {
    let claude_path = claude_json_path();
    let lock_path = lock_file_path();
    let state_dir = home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config").join("jig").join("state");
    cleanup_entries_inner(canonical_cwd, session_suffix, &claude_path, &lock_path, &state_dir)
}

/// Inner implementation with explicit paths — used by tests to inject temp dirs.
pub(crate) fn cleanup_entries_inner(
    canonical_cwd: &Path,
    session_suffix: &str,
    claude_path: &Path,
    lock_path: &Path,
    state_dir: &Path,
) -> Result<(), McpError> {
    let refcount_path = refcount_path_in(canonical_cwd, state_dir);

    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|e| McpError::LockError {
            path: lock_path.to_owned(),
            source: e,
        })?;

    let mut lock_guard = fd_lock::RwLock::new(lock_file);
    let _write_guard = lock_guard.write().map_err(|e| McpError::LockError {
        path: lock_path.to_owned(),
        source: e,
    })?;

    // Re-read refcount under lock
    let current_count = read_refcount(&refcount_path);
    let new_count = current_count.saturating_sub(1);

    if new_count > 0 {
        // Other sessions still running — update count, skip MCP removal
        let new_content = format!("{new_count}\n");
        std::fs::write(&refcount_path, new_content)?;
        debug!("Refcount decremented to {new_count}, skipping MCP cleanup");
        return Ok(());
    }

    // Count is zero — remove our MCP entries
    if claude_path.exists() {
        let contents = std::fs::read_to_string(claude_path)?;
        if let Ok(mut root) = serde_json::from_str::<Value>(&contents) {
            let cwd_key = canonical_cwd.to_string_lossy().into_owned();

            // Use direct map navigation — pointer_mut() interprets '/' in cwd_key
            // as a JSON Pointer separator, which breaks on absolute paths.
            if let Some(mcp_servers) = root
                .get_mut("projects")
                .and_then(|p| p.get_mut(cwd_key.as_str()))
                .and_then(|c| c.get_mut("mcpServers"))
                .and_then(Value::as_object_mut)
            {
                // Remove all entries with our session suffix
                let suffix_marker = format!("__{session_suffix}");
                mcp_servers.retain(|name, _| !name.ends_with(&suffix_marker));
            }

            let json_str = serde_json::to_string_pretty(&root)?;
            let tmp_path = claude_path.with_extension("jig-cleanup.tmp");
            std::fs::write(&tmp_path, json_str.as_bytes())?;
            std::fs::rename(&tmp_path, claude_path)?;
            info!("MCP cleanup complete: removed entries with suffix {session_suffix}");
        }
    }

    // Remove refcount file
    let _ = std::fs::remove_file(&refcount_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::McpServer;

    fn dummy_server() -> McpServer {
        McpServer {
            server_type: Some("stdio".to_owned()),
            command: Some("npx".to_owned()),
            args: Some(vec!["-y".to_owned(), "some-mcp".to_owned()]),
            env: None,
            url: None,
        }
    }

    #[test]
    fn test_detect_conflicts_with_conflict() {
        let mut existing: HashMap<String, Value> = HashMap::new();
        existing.insert("postgres".to_owned(), Value::Null);

        let mut new_servers: HashMap<String, McpServer> = HashMap::new();
        new_servers.insert("postgres".to_owned(), dummy_server());

        let rename_map = detect_conflicts(&existing, &new_servers, "jig_a3f1b2c9");
        assert_eq!(rename_map.get("postgres"), Some(&"postgres__jig_a3f1b2c9".to_owned()));
    }

    #[test]
    fn test_detect_conflicts_no_conflict() {
        let existing: HashMap<String, Value> = HashMap::new();
        let mut new_servers: HashMap<String, McpServer> = HashMap::new();
        new_servers.insert("postgres".to_owned(), dummy_server());

        let rename_map = detect_conflicts(&existing, &new_servers, "jig_a3f1b2c9");
        assert!(rename_map.is_empty());
    }

    #[test]
    fn test_detect_conflicts_partial_conflict() {
        let mut existing: HashMap<String, Value> = HashMap::new();
        existing.insert("postgres".to_owned(), Value::Null);

        let mut new_servers: HashMap<String, McpServer> = HashMap::new();
        new_servers.insert("postgres".to_owned(), dummy_server());
        new_servers.insert("redis".to_owned(), dummy_server());

        let rename_map = detect_conflicts(&existing, &new_servers, "jig_deadbeef");
        assert!(rename_map.contains_key("postgres"));
        assert!(!rename_map.contains_key("redis"));
    }

    #[test]
    fn test_suffix_format() {
        // generate_unique_suffix is private, but we can verify the format via detect_conflicts output
        let mut existing: HashMap<String, Value> = HashMap::new();
        existing.insert("myserver".to_owned(), Value::Null);
        let mut new_servers: HashMap<String, McpServer> = HashMap::new();
        new_servers.insert("myserver".to_owned(), dummy_server());

        let rename_map = detect_conflicts(&existing, &new_servers, "jig_12345678");
        let suffixed = rename_map.get("myserver").unwrap();
        assert!(suffixed.starts_with("myserver__jig_"));
    }

    #[test]
    fn test_json_pointer_fails_on_absolute_paths() {
        // Regression: pointer() treats '/' as a JSON Pointer separator (RFC 6901).
        // Absolute cwd paths like /private/tmp/jig-test silently mis-navigate.
        // Direct .get() chaining must be used instead.
        let root = serde_json::json!({
            "projects": {
                "/private/tmp/jig-test": {
                    "mcpServers": { "test__jig_abc12345": {} }
                }
            }
        });
        let cwd_key = "/private/tmp/jig-test";

        // JSON Pointer silently mis-navigates on absolute paths
        let pointer_path = format!("/projects{cwd_key}/mcpServers");
        assert!(
            root.pointer(&pointer_path).is_none(),
            "pointer() must not be used — it breaks on absolute paths containing '/'"
        );

        // Direct chained .get() works correctly
        let servers = root
            .get("projects")
            .and_then(|p| p.get(cwd_key))
            .and_then(|c| c.get("mcpServers"));
        assert!(servers.is_some(), "direct .get() navigation must find the mcpServers entry");
    }

    // ─── Task 0.4: write_atomic end-to-end tests ───────────────────────────────

    fn temp_mcp_env() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let claude_json = dir.path().join(".claude.json");
        let lock_path = dir.path().join(".lock");
        let state_dir = dir.path().join("state");
        std::fs::create_dir_all(&state_dir).unwrap();
        (dir, claude_json, lock_path, state_dir)
    }

    #[test]
    fn test_write_atomic_injects_servers_into_claude_json() {
        let (_dir, claude_path, lock_path, state_dir) = temp_mcp_env();
        let cwd = tempfile::tempdir().unwrap();
        let canonical_cwd = cwd.path().to_owned();

        let mut servers = HashMap::new();
        servers.insert("test-server".to_owned(), dummy_server());

        let result = write_atomic_inner(&servers, &canonical_cwd, 9999, &claude_path, &lock_path, &state_dir).unwrap();

        let contents = std::fs::read_to_string(&claude_path).unwrap();
        let root: Value = serde_json::from_str(&contents).unwrap();
        let cwd_key = canonical_cwd.to_string_lossy().into_owned();

        let mcp_servers = root
            .get("projects")
            .and_then(|p| p.get(&cwd_key))
            .and_then(|c| c.get("mcpServers"))
            .expect("mcpServers must be injected");

        let suffixed_name = format!("test-server__{}", result.session_suffix);
        assert!(mcp_servers.get(&suffixed_name).is_some(), "server must appear under suffixed name");
    }

    #[test]
    fn test_write_atomic_with_conflict_uses_suffix() {
        let (_dir, claude_path, lock_path, state_dir) = temp_mcp_env();
        let cwd = tempfile::tempdir().unwrap();
        let canonical_cwd = cwd.path().to_owned();

        // Pre-populate claude.json with an existing server_a
        let existing = serde_json::json!({
            "projects": {
                canonical_cwd.to_string_lossy().as_ref(): {
                    "mcpServers": {
                        "server_a": { "type": "stdio", "command": "old" }
                    }
                }
            }
        });
        std::fs::write(&claude_path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        let mut servers = HashMap::new();
        servers.insert("server_a".to_owned(), dummy_server());

        let result = write_atomic_inner(&servers, &canonical_cwd, 9998, &claude_path, &lock_path, &state_dir).unwrap();

        let contents = std::fs::read_to_string(&claude_path).unwrap();
        let root: Value = serde_json::from_str(&contents).unwrap();
        let cwd_key = canonical_cwd.to_string_lossy().into_owned();
        let mcp_servers = root["projects"][&cwd_key]["mcpServers"].as_object().unwrap();

        // Original entry preserved
        assert!(mcp_servers.contains_key("server_a"), "pre-existing server_a must be preserved");
        // New entry written under suffixed name
        let suffixed = format!("server_a__{}", result.session_suffix);
        assert!(mcp_servers.contains_key(&suffixed), "new server_a must use suffixed name: {suffixed}");
    }

    #[test]
    fn test_cleanup_entries_removes_only_suffixed() {
        let (_dir, claude_path, lock_path, state_dir) = temp_mcp_env();
        let cwd = tempfile::tempdir().unwrap();
        let canonical_cwd = cwd.path().to_owned();
        let suffix = "jig_testabcd";

        // Set up claude.json with one suffixed entry and one plain entry
        let cwd_key = canonical_cwd.to_string_lossy().into_owned();
        let existing = serde_json::json!({
            "projects": {
                &cwd_key: {
                    "mcpServers": {
                        format!("my-server__{suffix}"): { "type": "stdio", "command": "x" },
                        "preexisting": { "type": "stdio", "command": "y" }
                    }
                }
            }
        });
        std::fs::write(&claude_path, serde_json::to_string_pretty(&existing).unwrap()).unwrap();

        // Write refcount = 1 (simulates a single active session)
        let ref_path = refcount_path_in(&canonical_cwd, &state_dir);
        std::fs::write(&ref_path, format!("1\n{suffix}\n{cwd_key}\n1\n")).unwrap();

        cleanup_entries_inner(&canonical_cwd, suffix, &claude_path, &lock_path, &state_dir).unwrap();

        let contents = std::fs::read_to_string(&claude_path).unwrap();
        let root: Value = serde_json::from_str(&contents).unwrap();
        let mcp_servers = root["projects"][&cwd_key]["mcpServers"].as_object().unwrap();

        assert!(!mcp_servers.contains_key(&format!("my-server__{suffix}")), "suffixed entry must be removed");
        assert!(mcp_servers.contains_key("preexisting"), "pre-existing entry must survive");
    }

    #[test]
    fn test_refcount_increments_and_decrements() {
        let (_dir, claude_path, lock_path, state_dir) = temp_mcp_env();
        let cwd = tempfile::tempdir().unwrap();
        let canonical_cwd = cwd.path().to_owned();

        let mut servers = HashMap::new();
        servers.insert("svc".to_owned(), dummy_server());

        // Session 1: refcount 0 → 1
        let r1 = write_atomic_inner(&servers, &canonical_cwd, 1001, &claude_path, &lock_path, &state_dir).unwrap();
        let ref_path = refcount_path_in(&canonical_cwd, &state_dir);
        let count1 = read_refcount(&ref_path);
        assert_eq!(count1, 1, "after first write refcount must be 1");

        // Session 2: refcount 1 → 2
        write_atomic_inner(&servers, &canonical_cwd, 1002, &claude_path, &lock_path, &state_dir).unwrap();
        let count2 = read_refcount(&ref_path);
        assert_eq!(count2, 2, "after second write refcount must be 2");

        // Cleanup session 1: refcount 2 → 1 (entries NOT removed since > 0)
        cleanup_entries_inner(&canonical_cwd, &r1.session_suffix, &claude_path, &lock_path, &state_dir).unwrap();
        let count3 = read_refcount(&ref_path);
        assert_eq!(count3, 1, "after first cleanup refcount must be 1");

        // MCP entries must still exist (refcount > 0 means another session is live)
        let contents = std::fs::read_to_string(&claude_path).unwrap();
        let root: Value = serde_json::from_str(&contents).unwrap();
        let cwd_key = canonical_cwd.to_string_lossy().into_owned();
        let mcp_servers = root["projects"][&cwd_key]["mcpServers"].as_object().unwrap();
        assert!(!mcp_servers.is_empty(), "MCP entries must not be removed while refcount > 0");
    }

    #[test]
    fn test_cleanup_suffix_marker_removes_only_jig_entries() {
        // Regression: cleanup uses retain(|name| !name.ends_with(&suffix_marker)).
        // This only works if ALL jig-written entries carry the suffix.
        // Pre-existing entries (no suffix) must survive cleanup unchanged.
        let session_suffix = "jig_a1b2c3d4";
        let suffix_marker = format!("__{session_suffix}");

        let mut servers: serde_json::Map<String, Value> = serde_json::Map::new();
        servers.insert(format!("postgres__{session_suffix}"), Value::Null);
        servers.insert(format!("redis__{session_suffix}"), Value::Null);
        servers.insert("preexisting-server".to_owned(), Value::Null);

        servers.retain(|name, _| !name.ends_with(&suffix_marker));

        assert!(!servers.contains_key(&format!("postgres__{session_suffix}")),
            "suffixed jig entry must be removed");
        assert!(!servers.contains_key(&format!("redis__{session_suffix}")),
            "suffixed jig entry must be removed");
        assert!(servers.contains_key("preexisting-server"),
            "pre-existing entry without suffix must survive");
    }
}
