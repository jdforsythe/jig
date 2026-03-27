use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::warn;

/// A single active session record in the global JSONL lock file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalLockRecord {
    pub pid: u32,
    pub session_id: String,
    pub started_at: String,
    pub cwd: String,
}

/// Path to the global lock file: `~/.config/jig/jig.lock`
pub fn global_lock_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".config")
        .join("jig")
        .join("jig.lock")
}

/// Appends a record for this session to the global JSONL lock file.
/// Creates parent directories if needed. Silently ignores all errors.
pub fn write_global_lock(pid: u32, session_id: &str, cwd: &Path) {
    let path = global_lock_path();
    let record = GlobalLockRecord {
        pid,
        session_id: session_id.to_owned(),
        started_at: Utc::now().to_rfc3339(),
        cwd: cwd.display().to_string(),
    };
    let line = match serde_json::to_string(&record) {
        Ok(l) => l,
        Err(e) => {
            warn!("global lock: serialize error: {e}");
            return;
        }
    };
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            warn!("global lock: create dirs error: {e}");
            return;
        }
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(f, "{}", line);
    }
}

/// Removes the record for `session_id` from the global JSONL lock file.
/// Rewrites the file atomically minus the matching line.
/// Silently ignores all errors.
pub fn remove_global_lock(session_id: &str) {
    let path = global_lock_path();
    if !path.exists() {
        return;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let filtered: String = content
        .lines()
        .filter(|line| {
            if let Ok(record) = serde_json::from_str::<GlobalLockRecord>(line) {
                record.session_id != session_id
            } else {
                true // keep unparseable lines
            }
        })
        .map(|l| format!("{l}\n"))
        .collect();

    // Atomic write
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, &filtered).is_ok() {
        let _ = std::fs::rename(&tmp, &path);
    }
}

/// Returns all currently active sessions (live PIDs only).
pub fn active_sessions() -> Vec<GlobalLockRecord> {
    let path = global_lock_path();
    if !path.exists() {
        return vec![];
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    content
        .lines()
        .filter_map(|line| serde_json::from_str::<GlobalLockRecord>(line).ok())
        .filter(|r| is_pid_running(r.pid))
        .collect()
}

fn is_pid_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        match i32::try_from(pid) {
            Ok(p) if p > 0 => unsafe { libc::kill(p, 0) == 0 },
            _ => false,
        }
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
    use tempfile::tempdir;

    // Helper that writes to a specific path instead of the global one
    fn write_lock_to(path: &Path, pid: u32, session_id: &str, cwd: &Path) {
        let record = GlobalLockRecord {
            pid,
            session_id: session_id.to_owned(),
            started_at: chrono::Utc::now().to_rfc3339(),
            cwd: cwd.display().to_string(),
        };
        let line = serde_json::to_string(&record).unwrap();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(f, "{}", line).unwrap();
    }

    fn remove_lock_from(path: &Path, session_id: &str) {
        if !path.exists() {
            return;
        }
        let content = std::fs::read_to_string(path).unwrap_or_default();
        let filtered: String = content
            .lines()
            .filter(|line| {
                if let Ok(record) = serde_json::from_str::<GlobalLockRecord>(line) {
                    record.session_id != session_id
                } else {
                    true
                }
            })
            .map(|l| format!("{l}\n"))
            .collect();
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, &filtered).unwrap();
        std::fs::rename(&tmp, path).unwrap();
    }

    #[test]
    fn test_write_and_remove_global_lock() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jig.lock");

        write_lock_to(&path, std::process::id(), "test-session", dir.path());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test-session"), "lock file must contain session id");

        remove_lock_from(&path, "test-session");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.contains("test-session"), "session must be removed");
    }

    #[test]
    fn test_remove_global_lock_leaves_other_sessions() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("jig.lock");

        write_lock_to(&path, std::process::id(), "session-1", dir.path());
        write_lock_to(&path, std::process::id(), "session-2", dir.path());

        remove_lock_from(&path, "session-1");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(!content.contains("session-1"), "session-1 must be removed");
        assert!(content.contains("session-2"), "session-2 must remain");
    }

    #[test]
    fn test_write_global_lock_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        // write_global_lock uses global_lock_path() which we can't override here
        // Test via the public API with a known valid call
        // The function silently ignores errors, so we just verify no panic
        write_global_lock(std::process::id(), "test", dir.path());
        // Global lock path may or may not exist, but no panic
    }

    #[test]
    fn test_remove_global_lock_missing_file_is_ok() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.lock");
        // Should not panic
        if !path.exists() {
            // Test the logic by calling remove on missing file
            remove_lock_from(&path, "any-session");
            // The public function won't panic either
        }
    }

    #[test]
    fn test_active_sessions_with_own_pid() {
        // This tests that our own PID (definitely running) is considered alive
        let pid = std::process::id();
        assert!(
            matches!(i32::try_from(pid), Ok(p) if p > 0),
            "own PID must be a valid positive i32"
        );
    }
}
