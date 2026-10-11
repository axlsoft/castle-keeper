//! The single error type returned by every command.
//!
//! It serializes as `{ kind, message, hint? }` so the UI can show what went
//! wrong and what to do next without parsing strings.

use rdkafka::error::{KafkaError, RDKafkaErrorCode};
use serde::Serialize;
use specta::Type;

/// Broad category of a failure, used by the UI to pick wording and icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// The broker could not be reached (DNS, refused, network).
    Unreachable,
    /// SASL or SSL authentication was rejected.
    Auth,
    /// TLS handshake or certificate problem.
    Tls,
    /// An operation did not finish within its timeout.
    Timeout,
    /// Any other error reported by Kafka or librdkafka.
    Kafka,
    /// A topic, group, profile or similar does not exist.
    NotFound,
    /// The profile is read-only and the operation would write.
    ReadOnly,
    /// A destructive or production write needs a typed confirmation.
    ConfirmationRequired,
    /// The request itself is invalid.
    InvalidInput,
    /// Client or profile configuration is invalid.
    Config,
    /// The OS credential store failed.
    Secret,
    /// A bug or unexpected state inside the app.
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[error("{message}")]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    /// What the user can do about it, when we know.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn timeout(operation: &str, after: std::time::Duration) -> Self {
        Self::new(
            ErrorKind::Timeout,
            format!("{operation} timed out after {} ms", after.as_millis()),
        )
        .with_hint(
            "Check that the broker is reachable, or raise the timeout in the connection settings.",
        )
    }

    pub fn not_found(what: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, format!("{} not found", what.into()))
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }
}

fn error_code(error: &KafkaError) -> Option<RDKafkaErrorCode> {
    use KafkaError::*;
    match error {
        AdminOp(code)
        | ConsumerCommit(code)
        | ConsumerQueueClose(code)
        | Flush(code)
        | Global(code)
        | GroupListFetch(code)
        | MessageConsumption(code)
        | MessageConsumptionFatal(code)
        | MessageProduction(code)
        | MetadataFetch(code)
        | OffsetFetch(code)
        | Rebalance(code)
        | SetPartitionOffset(code)
        | StoreOffset(code)
        | MockCluster(code) => Some(*code),
        Transaction(e) => Some(e.code()),
        _ => None,
    }
}

impl From<KafkaError> for AppError {
    fn from(error: KafkaError) -> Self {
        use RDKafkaErrorCode as C;
        let message = error.to_string();
        match error_code(&error) {
            Some(C::BrokerTransportFailure | C::AllBrokersDown | C::Resolve | C::BrokerNotAvailable) => {
                AppError::new(ErrorKind::Unreachable, message).with_hint(
                    "Check the bootstrap address and port, that the broker is running, and that \
                     the advertised listeners resolve from this machine.",
                )
            }
            Some(C::Authentication | C::SaslAuthenticationFailed) => {
                AppError::new(ErrorKind::Auth, message)
                    .with_hint("Check the username, password and SASL mechanism for this connection.")
            }
            Some(C::SSL) => AppError::new(ErrorKind::Tls, message).with_hint(
                "Check the security protocol, CA certificate and client certificate for this connection.",
            ),
            Some(
                C::OperationTimedOut | C::RequestTimedOut | C::MessageTimedOut | C::TimedOutQueue,
            ) => AppError::new(ErrorKind::Timeout, message).with_hint(
                "Check that the broker is reachable, or raise the timeout in the connection settings.",
            ),
            Some(C::UnknownTopicOrPartition | C::UnknownTopic | C::UnknownPartition) => {
                AppError::new(ErrorKind::NotFound, message)
            }
            Some(C::TopicAuthorizationFailed | C::GroupAuthorizationFailed | C::ClusterAuthorizationFailed) => {
                AppError::new(ErrorKind::Auth, message)
                    .with_hint("The connection's principal lacks the ACLs for this operation.")
            }
            _ => match error {
                KafkaError::ClientConfig(..) | KafkaError::ClientCreation(_) => {
                    AppError::new(ErrorKind::Config, message)
                }
                _ => AppError::new(ErrorKind::Kafka, message),
            },
        }
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(error: tokio::task::JoinError) -> Self {
        AppError::internal(format!("background task failed: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_message_and_hint() {
        let error = AppError::new(ErrorKind::ReadOnly, "read-only").with_hint("unlock it");
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "readOnly", "message": "read-only", "hint": "unlock it" })
        );
    }

    #[test]
    fn omits_missing_hint() {
        let json = serde_json::to_value(AppError::internal("boom")).unwrap();
        assert!(json.get("hint").is_none());
    }

    #[test]
    fn classifies_kafka_errors() {
        let cases = [
            (
                RDKafkaErrorCode::BrokerTransportFailure,
                ErrorKind::Unreachable,
            ),
            (RDKafkaErrorCode::AllBrokersDown, ErrorKind::Unreachable),
            (RDKafkaErrorCode::SaslAuthenticationFailed, ErrorKind::Auth),
            (RDKafkaErrorCode::SSL, ErrorKind::Tls),
            (RDKafkaErrorCode::MessageTimedOut, ErrorKind::Timeout),
            (
                RDKafkaErrorCode::UnknownTopicOrPartition,
                ErrorKind::NotFound,
            ),
            (RDKafkaErrorCode::InvalidReplicationFactor, ErrorKind::Kafka),
        ];
        for (code, kind) in cases {
            assert_eq!(
                AppError::from(KafkaError::MetadataFetch(code)).kind,
                kind,
                "{code:?}"
            );
        }
        assert_eq!(
            AppError::from(KafkaError::ClientCreation("bad".into())).kind,
            ErrorKind::Config
        );
    }

    #[test]
    fn unreachable_errors_carry_a_hint() {
        let error = AppError::from(KafkaError::MetadataFetch(
            RDKafkaErrorCode::BrokerTransportFailure,
        ));
        assert!(error.hint.is_some());
    }
}
