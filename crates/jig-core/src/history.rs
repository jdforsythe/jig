use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Path to the history JSONL file.
pub fn history_path() -> PathBuf {
    home::home_dir()
        .unwrap_or_default()
        .join(".config")
        .join("jig")
        .join("state")
        .join("history.jsonl")
}

/// A session start record written to history.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    #[serde(rename = "type")]
    pub entry_type: String,
    pub session_id: String,
    pub started_at: String,
    pub template: Option<String>,
    pub persona: Option<String>,
    pub cwd: String,
    pub mcp_servers: Vec<String>,
    pub skills: Vec<String>,
}

impl HistoryEntry {
    pub fn new_start(
        session_id: &str,
        template: Option<&str>,
        persona: Option<&str>,
        cwd: &Path,
        mcp_servers: &[String],
    ) -> Self {
        Self {
            entry_type: "start".to_owned(),
            session_id: session_id.to_owned(),
            started_at: chrono::Utc::now().to_rfc3339(),
            template: template.map(str::to_owned),
            persona: persona.map(str::to_owned),
            cwd: cwd.to_string_lossy().into_owned(),
            mcp_servers: mcp_servers.to_vec(),
            skills: Vec::new(),
        }
    }
}

/// Records a session start entry to history.jsonl.
pub fn record_start(entry: &HistoryEntry) -> std::io::Result<()> {
    let path = history_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    let line = serde_json::to_string(entry)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Records a session exit entry to history.jsonl (separate appended line).
pub fn record_exit(session_id: &str, exit_code: i32) -> std::io::Result<()> {
    let path = history_path();

    let record = serde_json::json!({
        "type": "exit",
        "session_id": session_id,
        "exit_code": exit_code,
        "ended_at": chrono::Utc::now().to_rfc3339(),
        "jig_version": env!("CARGO_PKG_VERSION"),
        "token_count_estimate": null,
        "token_count_method": "heuristic",
    });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    let line = serde_json::to_string(&record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(file, "{line}")?;
    Ok(())
}

/// Reads the most recent complete session from history.jsonl (tail-first).
pub fn last_session() -> Option<HistoryEntry> {
    let path = history_path();
    let contents = std::fs::read_to_string(&path).ok()?;

    // Scan from end of file upward
    for line in contents.lines().rev() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(line) {
            if value.get("type").and_then(|v| v.as_str()) == Some("start") {
                if let Ok(entry) = serde_json::from_value::<HistoryEntry>(value) {
                    return Some(entry);
                }
            }
        }
    }
    None
}
