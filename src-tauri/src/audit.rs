//! Local append-only action log (JSON Lines) of every attempted write.

use std::{
    io::{BufRead, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
    error::{AppError, AppResult, ErrorKind},
    profiles::Environment,
};

/// Operations that change a cluster or lift a safeguard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum WriteAction {
    Produce,
    UnlockSession,
}

impl WriteAction {
    /// Destructive actions always need a typed confirmation.
    pub fn is_destructive(self) -> bool {
        match self {
            WriteAction::Produce | WriteAction::UnlockSession => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Ok,
    Failed,
    Blocked,
    DryRun,
    /// The operation was authorized but never reported a result.
    Abandoned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActionEntry {
    pub id: String,
    /// RFC 3339, UTC.
    pub timestamp: String,
    pub profile_id: String,
    pub profile_name: String,
    pub environment: Environment,
    pub action: WriteAction,
    pub target: String,
    pub dry_run: bool,
    pub outcome: Outcome,
    #[serde(default)]
    pub detail: Option<String>,
}

pub struct ActionLog {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl ActionLog {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn append(&self, entry: &ActionEntry) -> AppResult<()> {
        let io = |e: std::io::Error| {
            AppError::new(
                ErrorKind::Internal,
                format!(
                    "could not write the action log at {}: {e}",
                    self.path.display()
                ),
            )
        };
        let mut line = serde_json::to_string(entry)
            .map_err(|e| AppError::internal(format!("could not serialize action: {e}")))?;
        line.push('\n');
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| AppError::internal("action log lock poisoned"))?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(io)?;
        file.write_all(line.as_bytes()).map_err(io)?;
        file.sync_data().map_err(io)
    }

    /// The most recent entries, newest first. Unreadable lines are skipped.
    pub fn recent(&self, limit: usize) -> AppResult<Vec<ActionEntry>> {
        let file = match std::fs::File::open(&self.path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(AppError::internal(format!(
                    "could not read the action log at {}: {e}",
                    self.path.display()
                )))
            }
        };
        let mut entries: Vec<ActionEntry> = std::io::BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();
        entries.reverse();
        entries.truncate(limit);
        Ok(entries)
    }
}

pub fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}
