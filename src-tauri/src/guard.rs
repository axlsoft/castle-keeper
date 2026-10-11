//! The write guard: the only way to obtain a [`WriteTicket`], which every
//! cluster-changing operation must consume. Read-only profiles, production
//! confirmations and the action log are enforced here, in Rust.

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::{
    audit::{ActionEntry, ActionLog, Outcome, WriteAction, now_rfc3339},
    error::{AppError, AppResult, ErrorKind},
    profiles::{Environment, Profile},
};

/// Per-session safety state. Never persisted: a restart re-locks everything.
#[derive(Default)]
pub struct SessionLocks {
    unlocked: Mutex<HashSet<String>>,
}

impl SessionLocks {
    pub fn is_unlocked(&self, profile_id: &str) -> bool {
        self.unlocked
            .lock()
            .map(|set| set.contains(profile_id))
            .unwrap_or(false)
    }

    fn unlock(&self, profile_id: &str) -> AppResult<()> {
        self.unlocked
            .lock()
            .map_err(|_| AppError::internal("session lock poisoned"))?
            .insert(profile_id.to_string());
        Ok(())
    }

    pub fn lock(&self, profile_id: &str) {
        if let Ok(mut set) = self.unlocked.lock() {
            set.remove(profile_id);
        }
    }

    /// Re-locks every profile, e.g. when the active profile changes.
    pub fn lock_all(&self) {
        if let Ok(mut set) = self.unlocked.lock() {
            set.clear();
        }
    }
}

#[derive(Debug, Clone)]
pub struct WriteRequest {
    pub action: WriteAction,
    /// What is being written to, e.g. the topic name.
    pub target: String,
    pub dry_run: bool,
    /// The profile name typed by the user, when a confirmation is required.
    pub confirmation: Option<String>,
}

pub struct WriteGuard {
    log: Arc<ActionLog>,
    session: Arc<SessionLocks>,
}

impl WriteGuard {
    pub fn new(log: Arc<ActionLog>, session: Arc<SessionLocks>) -> Self {
        Self { log, session }
    }

    pub fn session(&self) -> &SessionLocks {
        &self.session
    }

    pub fn log(&self) -> &ActionLog {
        &self.log
    }

    /// Whether writes to this profile are currently refused.
    pub fn is_read_only(&self, profile: &Profile) -> bool {
        profile.read_only && !self.session.is_unlocked(&profile.id)
    }

    /// Checks a write against the profile's safeguards. Blocked attempts are
    /// logged and returned as errors; allowed ones yield a ticket that logs
    /// the outcome once the operation completes.
    pub fn authorize(&self, profile: &Profile, request: WriteRequest) -> AppResult<WriteTicket> {
        let entry = ActionEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_rfc3339(),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            environment: profile.environment,
            action: request.action,
            target: request.target.clone(),
            dry_run: request.dry_run,
            outcome: Outcome::Blocked,
            detail: None,
        };

        if !request.dry_run
            && let Err(error) = self.check(profile, &request)
        {
            let blocked = ActionEntry {
                detail: Some(error.message.clone()),
                ..entry
            };
            self.log.append(&blocked)?;
            return Err(error);
        }

        Ok(WriteTicket {
            entry,
            log: self.log.clone(),
            finished: false,
        })
    }

    fn check(&self, profile: &Profile, request: &WriteRequest) -> AppResult<()> {
        if self.is_read_only(profile) {
            let hint = if profile.environment == Environment::Prod {
                "Production connections are read-only. Unlock this connection for the current session to write."
            } else {
                "Turn off read-only in the connection settings, or unlock it for this session."
            };
            return Err(AppError::new(
                ErrorKind::ReadOnly,
                format!("\"{}\" is read-only; the write was blocked.", profile.name),
            )
            .with_hint(hint));
        }
        let needs_confirmation =
            profile.environment == Environment::Prod || request.action.is_destructive();
        if needs_confirmation && request.confirmation.as_deref() != Some(profile.name.as_str()) {
            return Err(AppError::new(
                ErrorKind::ConfirmationRequired,
                format!("Type \"{}\" to confirm this write.", profile.name),
            ));
        }
        Ok(())
    }

    /// Lifts read-only for the current session after a typed confirmation of
    /// the profile name. Logged like a write.
    pub fn unlock_session(&self, profile: &Profile, confirmation: &str) -> AppResult<()> {
        let mut entry = ActionEntry {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: now_rfc3339(),
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            environment: profile.environment,
            action: WriteAction::UnlockSession,
            target: profile.name.clone(),
            dry_run: false,
            outcome: Outcome::Ok,
            detail: None,
        };
        if confirmation != profile.name {
            entry.outcome = Outcome::Blocked;
            entry.detail = Some("confirmation did not match the connection name".into());
            self.log.append(&entry)?;
            return Err(AppError::new(
                ErrorKind::ConfirmationRequired,
                format!("Type \"{}\" exactly to unlock it.", profile.name),
            ));
        }
        self.session.unlock(&profile.id)?;
        self.log.append(&entry)
    }
}

/// Proof that a write passed the guard. Write paths take it by value and must
/// report how the operation ended; dropping it unfinished logs `abandoned`.
#[must_use = "a write ticket must be completed so the action log records the outcome"]
pub struct WriteTicket {
    entry: ActionEntry,
    log: Arc<ActionLog>,
    finished: bool,
}

impl WriteTicket {
    pub fn is_dry_run(&self) -> bool {
        self.entry.dry_run
    }

    pub fn target(&self) -> &str {
        &self.entry.target
    }

    /// Records the result of the operation in the action log.
    pub fn complete<T>(mut self, result: &AppResult<T>) {
        let (outcome, detail) = match result {
            Ok(_) if self.entry.dry_run => (Outcome::DryRun, None),
            Ok(_) => (Outcome::Ok, None),
            Err(error) => (Outcome::Failed, Some(error.message.clone())),
        };
        self.finish(outcome, detail);
    }

    pub fn complete_with_detail<T>(mut self, result: &AppResult<T>, detail: String) {
        let outcome = match result {
            Ok(_) if self.entry.dry_run => Outcome::DryRun,
            Ok(_) => Outcome::Ok,
            Err(_) => Outcome::Failed,
        };
        let detail = match result {
            Err(error) => format!("{detail}: {}", error.message),
            Ok(_) => detail,
        };
        self.finish(outcome, Some(detail));
    }

    fn finish(&mut self, outcome: Outcome, detail: Option<String>) {
        self.finished = true;
        let entry = ActionEntry {
            timestamp: now_rfc3339(),
            outcome,
            detail,
            ..self.entry.clone()
        };
        if let Err(error) = self.log.append(&entry) {
            tracing::error!(%error, "failed to record action outcome");
        }
    }
}

impl Drop for WriteTicket {
    fn drop(&mut self) {
        if !self.finished {
            self.finish(Outcome::Abandoned, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::TlsConfig;

    fn profile(environment: Environment, read_only: bool) -> Profile {
        Profile {
            id: "p1".into(),
            name: "orders-prod".into(),
            environment,
            bootstrap: "localhost:9092".into(),
            sasl: None,
            tls: TlsConfig::default(),
            read_only,
            admin_timeout_ms: 10_000,
            delivery_timeout_ms: 15_000,
        }
    }

    fn produce(confirmation: Option<&str>, dry_run: bool) -> WriteRequest {
        WriteRequest {
            action: WriteAction::Produce,
            target: "orders".into(),
            dry_run,
            confirmation: confirmation.map(str::to_string),
        }
    }

    fn guard() -> (tempfile::TempDir, WriteGuard) {
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(ActionLog::new(dir.path().join("actions.jsonl")));
        (dir, WriteGuard::new(log, Arc::new(SessionLocks::default())))
    }

    #[test]
    fn read_only_profile_blocks_and_logs() {
        let (_dir, guard) = guard();
        let err = guard
            .authorize(&profile(Environment::Prod, true), produce(None, false))
            .err()
            .unwrap();
        assert_eq!(err.kind, ErrorKind::ReadOnly);
        let log = guard.log().recent(10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].outcome, Outcome::Blocked);
        assert_eq!(log[0].target, "orders");
    }

    #[test]
    fn prod_unlock_needs_exact_name_and_writes_need_confirmation() {
        let (_dir, guard) = guard();
        let prod = profile(Environment::Prod, true);
        assert_eq!(
            guard.unlock_session(&prod, "orders").unwrap_err().kind,
            ErrorKind::ConfirmationRequired
        );
        guard.unlock_session(&prod, "orders-prod").unwrap();

        let err = guard.authorize(&prod, produce(None, false)).err().unwrap();
        assert_eq!(err.kind, ErrorKind::ConfirmationRequired);

        let ticket = guard
            .authorize(&prod, produce(Some("orders-prod"), false))
            .unwrap();
        ticket.complete(&Ok::<(), AppError>(()));

        guard.session().lock_all();
        assert!(guard.is_read_only(&prod));

        let outcomes: Vec<_> = guard
            .log()
            .recent(10)
            .unwrap()
            .into_iter()
            .map(|e| (e.action, e.outcome))
            .collect();
        assert_eq!(
            outcomes,
            vec![
                (WriteAction::Produce, Outcome::Ok),
                (WriteAction::Produce, Outcome::Blocked),
                (WriteAction::UnlockSession, Outcome::Ok),
                (WriteAction::UnlockSession, Outcome::Blocked),
            ]
        );
    }

    #[test]
    fn dry_run_is_allowed_on_read_only_and_logged_as_dry_run() {
        let (_dir, guard) = guard();
        let ticket = guard
            .authorize(&profile(Environment::Prod, true), produce(None, true))
            .unwrap();
        assert!(ticket.is_dry_run());
        ticket.complete(&Ok::<(), AppError>(()));
        assert_eq!(guard.log().recent(1).unwrap()[0].outcome, Outcome::DryRun);
    }

    #[test]
    fn dropped_ticket_is_logged_as_abandoned() {
        let (_dir, guard) = guard();
        let ticket = guard
            .authorize(&profile(Environment::Dev, false), produce(None, false))
            .unwrap();
        drop(ticket);
        assert_eq!(
            guard.log().recent(1).unwrap()[0].outcome,
            Outcome::Abandoned
        );
    }

    #[test]
    fn failures_are_logged_with_the_error() {
        let (_dir, guard) = guard();
        let ticket = guard
            .authorize(&profile(Environment::Local, false), produce(None, false))
            .unwrap();
        ticket.complete(&Err::<(), _>(AppError::internal("broker said no")));
        let entry = &guard.log().recent(1).unwrap()[0];
        assert_eq!(entry.outcome, Outcome::Failed);
        assert_eq!(entry.detail.as_deref(), Some("broker said no"));
    }
}
