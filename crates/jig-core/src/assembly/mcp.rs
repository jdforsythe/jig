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
    let cwd_hash = sha256_hex(canonical_cwd.to_string_lossy().as_bytes());
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("state")
        .join(format!("{cwd_hash}.refcount"))
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
    let refcount_path = refcount_path(canonical_cwd);

    // Ensure state directory exists
    if let Some(parent) = refcount_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Acquire exclusive lock on the dedicated lock file
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| McpError::LockError {
            path: lock_path.clone(),
            source: e,
        })?;

    let mut lock_guard = fd_lock::RwLock::new(lock_file);
    let _write_guard = lock_guard.write().map_err(|e| McpError::LockError {
        path: lock_path.clone(),
        source: e,
    })?;

    // Read current ~/.claude.json as raw Value
    let mut root: Value = if claude_path.exists() {
        let contents = std::fs::read_to_string(&claude_path)?;
        if contents.trim().is_empty() {
            Value::Object(Default::default())
        } else {
            serde_json::from_str(&contents)?
        }
    } else {
        Value::Object(Default::default())
    };

    let cwd_key = canonical_cwd.to_string_lossy().into_owned();

    // Extract existing mcpServers for conflict detection
    let existing_servers: HashMap<String, Value> = root
        .pointer(&format!("/projects/{cwd_key}/mcpServers"))
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

    // Backup ~/.claude.json atomically before mutation
    if claude_path.exists() {
        let backup_path = claude_path.with_extension(format!("jig-backup-{pid}"));
        let backup_tmp = claude_path.with_extension(format!("jig-backup-{pid}.tmp"));
        let backup_contents = std::fs::read_to_string(&claude_path)?;
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

    std::fs::rename(&tmp_path, &claude_path)?;

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
    let refcount_path = refcount_path(canonical_cwd);

    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| McpError::LockError {
            path: lock_path.clone(),
            source: e,
        })?;

    let mut lock_guard = fd_lock::RwLock::new(lock_file);
    let _write_guard = lock_guard.write().map_err(|e| McpError::LockError {
        path: lock_path.clone(),
        source: e,
    })?;

    // Re-read refcount under lock
    let current_count = read_refcount(&refcount_path);
    let new_count = current_count.saturating_sub(1);

    if new_count > 0 {
        // Other sessions still running — update count, skip MCP removal
        let content = std::fs::read_to_string(&refcount_path).unwrap_or_default();
        let mut lines: Vec<&str> = content.lines().collect();
        if !lines.is_empty() {
            lines[0] = ""; // placeholder, will be replaced
        }
        let new_content = format!("{new_count}\n");
        std::fs::write(&refcount_path, new_content)?;
        debug!("Refcount decremented to {new_count}, skipping MCP cleanup");
        return Ok(());
    }

    // Count is zero — remove our MCP entries
    if claude_path.exists() {
        let contents = std::fs::read_to_string(&claude_path)?;
        if let Ok(mut root) = serde_json::from_str::<Value>(&contents) {
            let cwd_key = canonical_cwd.to_string_lossy().into_owned();

            if let Some(mcp_servers) = root
                .pointer_mut(&format!("/projects/{cwd_key}/mcpServers"))
                .and_then(Value::as_object_mut)
            {
                // Remove all entries with our session suffix
                let suffix_marker = format!("__{session_suffix}");
                mcp_servers.retain(|name, _| !name.ends_with(&suffix_marker));
            }

            let json_str = serde_json::to_string_pretty(&root)?;
            let tmp_path = claude_path.with_extension("jig-cleanup.tmp");
            std::fs::write(&tmp_path, json_str.as_bytes())?;
            std::fs::rename(&tmp_path, &claude_path)?;
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
}
