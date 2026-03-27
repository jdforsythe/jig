/// Project-level lock file for detecting concurrent jig instances.
///
/// When jig launches, it checks for an existing `.jig.lock` in the project
/// directory. If the lock file exists and the recorded PID is still running,
/// it warns about concurrency. On exit, the lock file is removed.
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct LockRecord {
    pub pid: u32,
    pub session_id: String,
    pub started_at: String,
}

/// Path to the project lock file.
pub fn lock_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".jig.lock")
}

/// Writes the lock file for this session. Silently ignores write errors.
pub fn write_lock(project_dir: &Path, pid: u32, session_id: &str) {
    let path = lock_path(project_dir);
    let record = LockRecord {
        pid,
        session_id: session_id.to_owned(),
        started_at: chrono::Utc::now().to_rfc3339(),
    };
    if let Ok(json) = serde_json::to_string(&record) {
        let _ = std::fs::write(&path, json);
    }
}

/// Removes the lock file. Silently ignores errors.
pub fn remove_lock(project_dir: &Path) {
    let _ = std::fs::remove_file(lock_path(project_dir));
}

/// Checks for an existing lock and returns a warning string if a concurrent
/// jig instance appears to be running. Returns `None` if no conflict.
pub fn check_existing_lock(project_dir: &Path) -> Option<String> {
    let path = lock_path(project_dir);
    let contents = std::fs::read_to_string(&path).ok()?;
    let record: LockRecord = serde_json::from_str(&contents).ok()?;

    // Check if the PID is still alive using kill(pid, 0)
    if is_pid_running(record.pid) {
        Some(format!(
            "Another jig instance (PID {}) is running in this directory (session: {}). \
             Concurrent sessions share ~/.claude.json — MCP servers may conflict.",
            record.pid,
            &record.session_id[..8.min(record.session_id.len())],
        ))
    } else {
        // Stale lock from a crashed/killed previous session — clean it up
        remove_lock(project_dir);
        None
    }
}

/// Returns true if a process with the given PID is currently running.
fn is_pid_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // pid_t is i32 on most Unix systems. Values > i32::MAX overflow to negative,
        // which would cause kill(-1, 0) to target all processes — not what we want.
        // Any valid OS PID fits in a positive i32.
        let signed_pid = match i32::try_from(pid) {
            Ok(p) if p > 0 => p,
            _ => return false,
        };
        // kill(pid, 0) returns 0 if the process exists, -1/ESRCH if not
        let result = unsafe { libc::kill(signed_pid, 0) };
        result == 0
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_and_remove_lock() {
        let dir = tempfile::tempdir().unwrap();
        write_lock(dir.path(), 12345, "test-session-id");
        let path = lock_path(dir.path());
        assert!(path.exists(), "lock file must be created");
        let contents = std::fs::read_to_string(&path).unwrap();
        let record: LockRecord = serde_json::from_str(&contents).unwrap();
        assert_eq!(record.pid, 12345);
        assert_eq!(record.session_id, "test-session-id");
        remove_lock(dir.path());
        assert!(!path.exists(), "lock file must be removed");
    }

    #[test]
    fn test_check_existing_lock_with_stale_pid() {
        let dir = tempfile::tempdir().unwrap();
        // Write a lock with a PID that definitely doesn't exist (u32::MAX)
        write_lock(dir.path(), u32::MAX, "old-session");
        let result = check_existing_lock(dir.path());
        // Stale PID → lock should be cleaned up and None returned
        assert!(result.is_none(), "stale lock must be cleaned up");
        assert!(!lock_path(dir.path()).exists(), "stale lock file must be removed");
    }

    #[test]
    fn test_check_existing_lock_with_own_pid() {
        let dir = tempfile::tempdir().unwrap();
        let own_pid = std::process::id();
        write_lock(dir.path(), own_pid, "current-session");
        let result = check_existing_lock(dir.path());
        // Our own PID is running — should return a warning
        assert!(result.is_some(), "own PID must be detected as running");
        assert!(result.unwrap().contains(&own_pid.to_string()));
        // Clean up
        remove_lock(dir.path());
    }

    #[test]
    fn test_check_existing_lock_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = check_existing_lock(dir.path());
        assert!(result.is_none(), "no lock file means no conflict");
    }
}
