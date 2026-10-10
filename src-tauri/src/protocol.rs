//! Versioned typed stdio protocol between the OpenTUI client and the Rust
//! backend (epic #294, issue #297).
//!
//! Framing is newline-delimited UTF-8 JSON: standard output carries protocol
//! messages only and standard error carries diagnostics. Every frame is a
//! single JSON object with no embedded newline. Unknown fields, unknown methods,
//! invalid enums, and oversized or malformed frames are rejected. The concrete
//! contract lives in `protocol/norn-protocol.schema.json`; the TypeScript types
//! are generated from it under `src/protocol/generated/`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The only supported protocol version for this contract.
pub const PROTOCOL_VERSION: u32 = 1;
/// Maximum accepted frame size in bytes (defensive bound on a single line).
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Structured protocol error carried in error responses and state events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProtocolError {
    pub code: ProtocolErrorCode,
    pub message: String,
    #[serde(default)]
    pub retryable: bool,
}

impl ProtocolError {
    pub fn new(code: ProtocolErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retryable: false,
        }
    }

    /// The stable machine code used across the Rust and TypeScript sides.
    pub fn code_str(&self) -> &'static str {
        self.code.as_str()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtocolErrorCode {
    InvalidRequest,
    UnsupportedVersion,
    UnknownMethod,
    InvalidParams,
    Cancelled,
    NotFound,
    TargetStale,
    Conflict,
    Internal,
}

impl ProtocolErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalidRequest",
            Self::UnsupportedVersion => "unsupportedVersion",
            Self::UnknownMethod => "unknownMethod",
            Self::InvalidParams => "invalidParams",
            Self::Cancelled => "cancelled",
            Self::NotFound => "notFound",
            Self::TargetStale => "targetStale",
            Self::Conflict => "conflict",
            Self::Internal => "internal",
        }
    }
}

/// Identity of the review target a request or event applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TargetIdentity {
    Provider {
        provider: ProviderKind,
        workspace: String,
        repo: String,
        #[serde(rename = "prId", default, skip_serializing_if = "Option::is_none")]
        pr_id: Option<u32>,
        #[serde(rename = "runId", default, skip_serializing_if = "Option::is_none")]
        run_id: Option<String>,
    },
    Local {
        #[serde(rename = "localSnapshotSha256")]
        local_snapshot_sha256: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Github,
    Bitbucket,
}

/// Parameters for `file.preview`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilePreviewParams {
    pub target: TargetIdentity,
    pub path: String,
}

/// Parameters for `diff.file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffFileParams {
    pub target: TargetIdentity,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_lines: Option<u32>,
}

/// Parameters for `review.start`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewStartParams {
    pub target: TargetIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Parameters for the control methods (`repository.status`, `operation.status`,
/// `operation.cancel`, `shutdown`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationControlParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
}

/// Parameters for methods that take no arguments; rejects unknown fields so the
/// schema and the Rust types agree on what a valid request looks like.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyParams {}

/// Parameters carrying only a review target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetParams {
    pub target: TargetIdentity,
}

/// A first-slice request. The `method` discriminates the typed parameters, so
/// an unknown method or a malformed target is rejected at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum Request {
    #[serde(rename = "repository.status")]
    RepositoryStatus {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
    #[serde(rename = "diff.file")]
    DiffFile { id: String, params: DiffFileParams },
    #[serde(rename = "file.preview")]
    FilePreview {
        id: String,
        params: FilePreviewParams,
    },
    #[serde(rename = "review.files")]
    ReviewFiles { id: String, params: TargetParams },
    #[serde(rename = "review.findings")]
    ReviewFindings { id: String, params: TargetParams },
    #[serde(rename = "review.history")]
    ReviewHistory { id: String, params: TargetParams },
    #[serde(rename = "review.targets")]
    ReviewTargets {
        id: String,
        #[serde(default)]
        params: EmptyParams,
    },
    #[serde(rename = "browser.open")]
    BrowserOpen { id: String, params: TargetParams },
    #[serde(rename = "review.start")]
    ReviewStart {
        id: String,
        params: ReviewStartParams,
    },
    #[serde(rename = "operation.status")]
    OperationStatus {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
    #[serde(rename = "operation.cancel")]
    OperationCancel {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
    #[serde(rename = "shutdown")]
    Shutdown {
        id: String,
        #[serde(default)]
        params: OperationControlParams,
    },
}

impl Request {
    pub fn id(&self) -> &str {
        match self {
            Self::RepositoryStatus { id, .. }
            | Self::DiffFile { id, .. }
            | Self::FilePreview { id, .. }
            | Self::ReviewFiles { id, .. }
            | Self::ReviewFindings { id, .. }
            | Self::ReviewTargets { id, .. }
            | Self::ReviewHistory { id, .. }
            | Self::BrowserOpen { id, .. }
            | Self::ReviewStart { id, .. }
            | Self::OperationStatus { id, .. }
            | Self::OperationCancel { id, .. }
            | Self::Shutdown { id, .. } => id,
        }
    }

    pub fn method(&self) -> &'static str {
        match self {
            Self::RepositoryStatus { .. } => "repository.status",
            Self::DiffFile { .. } => "diff.file",
            Self::FilePreview { .. } => "file.preview",
            Self::ReviewFiles { .. } => "review.files",
            Self::ReviewFindings { .. } => "review.findings",
            Self::ReviewTargets { .. } => "review.targets",
            Self::ReviewHistory { .. } => "review.history",
            Self::BrowserOpen { .. } => "browser.open",
            Self::ReviewStart { .. } => "review.start",
            Self::OperationStatus { .. } => "operation.status",
            Self::OperationCancel { .. } => "operation.cancel",
            Self::Shutdown { .. } => "shutdown",
        }
    }

    /// Enforce the schema rules the envelope types do not express: a non-empty
    /// id, a valid target, a non-empty path, and the `contextLines` range.
    fn validate(&self) -> Result<(), ProtocolError> {
        if self.id().is_empty() {
            return Err(invalid_request("request id must not be empty"));
        }
        match self {
            Self::DiffFile { params, .. } => {
                params.target.validate()?;
                if params.path.is_empty() {
                    return Err(invalid_request("`diff.file` path must not be empty"));
                }
                if params.context_lines.is_some_and(|lines| lines > 100) {
                    return Err(invalid_request("`diff.file` contextLines must be <= 100"));
                }
            }
            Self::FilePreview { params, .. } => {
                params.target.validate()?;
                if params.path.is_empty() {
                    return Err(invalid_request("`file.preview` path must not be empty"));
                }
            }
            Self::ReviewStart { params, .. } => params.target.validate()?,
            Self::ReviewFiles { params, .. }
            | Self::ReviewFindings { params, .. }
            | Self::ReviewHistory { params, .. }
            | Self::BrowserOpen { params, .. } => params.target.validate()?,
            Self::RepositoryStatus { .. }
            | Self::ReviewTargets { .. }
            | Self::OperationStatus { .. }
            | Self::OperationCancel { .. }
            | Self::Shutdown { .. } => {}
        }
        Ok(())
    }
}

impl TargetIdentity {
    fn validate(&self) -> Result<(), ProtocolError> {
        match self {
            Self::Provider {
                workspace,
                repo,
                run_id,
                ..
            } => {
                if workspace.is_empty() || repo.is_empty() {
                    return Err(invalid_request(
                        "provider target workspace and repo must not be empty",
                    ));
                }
                if run_id.as_deref() == Some("") {
                    return Err(invalid_request("provider target `runId` must not be empty"));
                }
                Ok(())
            }
            Self::Local {
                local_snapshot_sha256,
            } => {
                if local_snapshot_sha256.len() != 64 {
                    return Err(invalid_request(
                        "local target `localSnapshotSha256` must be 64 characters",
                    ));
                }
                Ok(())
            }
        }
    }
}

impl Response {
    /// A success carries exactly a result; a failure carries exactly an error.
    fn validate(&self) -> Result<(), ProtocolError> {
        if self.id.is_empty() {
            return Err(invalid_request("response id must not be empty"));
        }
        match (self.ok, self.result.is_some(), self.error.is_some()) {
            (true, true, false) | (false, false, true) => Ok(()),
            _ => Err(invalid_request(
                "response must carry a result when ok and an error otherwise",
            )),
        }
    }
}

fn invalid_request(message: impl Into<String>) -> ProtocolError {
    ProtocolError::new(ProtocolErrorCode::InvalidRequest, message)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hello {
    pub protocol_version: u32,
    pub client: ClientInfo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ready {
    pub protocol_version: u32,
    pub server_version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ProtocolError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationEventFrame {
    pub operation_id: String,
    pub sequence: u64,
    pub target: TargetIdentity,
    pub event: OperationEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OperationEvent {
    Progress {
        message: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        completed: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
    },
    Log {
        level: LogLevel,
        message: String,
    },
    State {
        state: OperationState,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<ProtocolError>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationState {
    Accepted,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// A request the client sends to the backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    Hello(Hello),
    Request(Request),
}

/// A message the backend sends to the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    Ready(Ready),
    Response(Response),
    Event(OperationEventFrame),
}

fn frame_too_large() -> ProtocolError {
    ProtocolError::new(
        ProtocolErrorCode::InvalidRequest,
        format!("frame exceeds {MAX_FRAME_BYTES} bytes"),
    )
}

fn decode_json<T: serde::de::DeserializeOwned>(line: &[u8]) -> Result<T, ProtocolError> {
    if line.len() > MAX_FRAME_BYTES {
        return Err(frame_too_large());
    }
    let text = std::str::from_utf8(line).map_err(|_| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            "frame is not valid UTF-8",
        )
    })?;
    let trimmed = text.trim_end_matches(['\n', '\r']);
    if trimmed.is_empty() {
        return Err(ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            "empty frame",
        ));
    }
    serde_json::from_str(trimmed).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::InvalidRequest,
            format!("malformed frame: {error}"),
        )
    })
}

/// Decode one client frame, enforcing framing limits and the envelope shape.
pub fn decode_client_line(line: &[u8]) -> Result<ClientMessage, ProtocolError> {
    let message: ClientMessage = decode_json(line)?;
    match &message {
        ClientMessage::Hello(hello) => validate_handshake(hello)?,
        ClientMessage::Request(request) => request.validate()?,
    }
    Ok(message)
}

/// Decode one server frame, enforcing framing limits and the envelope shape.
pub fn decode_server_line(line: &[u8]) -> Result<ServerMessage, ProtocolError> {
    let message: ServerMessage = decode_json(line)?;
    match &message {
        ServerMessage::Ready(ready) => {
            if ready.protocol_version != PROTOCOL_VERSION {
                return Err(ProtocolError::new(
                    ProtocolErrorCode::UnsupportedVersion,
                    format!(
                        "unsupported protocol version {}; this build speaks {PROTOCOL_VERSION}",
                        ready.protocol_version
                    ),
                ));
            }
        }
        ServerMessage::Response(response) => response.validate()?,
        ServerMessage::Event(frame) => {
            if frame.operation_id.is_empty() {
                return Err(invalid_request("event operationId must not be empty"));
            }
        }
    }
    Ok(message)
}

/// Encode a message as a single protocol frame (one line, no embedded newline).
pub fn encode_line<T: Serialize>(message: &T) -> Result<String, ProtocolError> {
    let mut encoded = serde_json::to_string(message).map_err(|error| {
        ProtocolError::new(
            ProtocolErrorCode::Internal,
            format!("failed to encode frame: {error}"),
        )
    })?;
    if encoded.len() + 1 > MAX_FRAME_BYTES {
        return Err(frame_too_large());
    }
    encoded.push('\n');
    Ok(encoded)
}

/// Reject an incompatible handshake before any review action is enabled.
pub fn validate_handshake(hello: &Hello) -> Result<(), ProtocolError> {
    if hello.protocol_version != PROTOCOL_VERSION {
        return Err(ProtocolError::new(
            ProtocolErrorCode::UnsupportedVersion,
            format!(
                "unsupported protocol version {}; this build speaks {PROTOCOL_VERSION}",
                hello.protocol_version
            ),
        ));
    }
    Ok(())
}

/// Tracks the highest observed event sequence per operation. Duplicate or
/// out-of-order event sequences are rejected so the client never renders a
/// stale snapshot; the caller resynchronizes with an `operation.status`
/// request when it detects a gap.
#[derive(Debug, Default)]
pub struct SequenceGuard {
    highest: HashMap<String, u64>,
}

impl SequenceGuard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe an event sequence. Returns the previous highest sequence, or an
    /// error when the sequence is a duplicate or moves backwards.
    pub fn observe(
        &mut self,
        operation_id: &str,
        sequence: u64,
    ) -> Result<Option<u64>, ProtocolError> {
        match self.highest.get(operation_id).copied() {
            Some(previous) if sequence <= previous => Err(ProtocolError::new(
                ProtocolErrorCode::Conflict,
                format!(
                    "out-of-order event sequence {sequence} for operation {operation_id}; expected greater than {previous}"
                ),
            )),
            previous => {
                self.highest.insert(operation_id.to_string(), sequence);
                Ok(previous)
            }
        }
    }

    /// Highest sequence observed so far for an operation, if any.
    pub fn highest(&self, operation_id: &str) -> Option<u64> {
        self.highest.get(operation_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../protocol/fixtures")
    }

    fn read_fixture(relative: &str) -> Vec<u8> {
        std::fs::read(fixtures_dir().join(relative))
            .unwrap_or_else(|error| panic!("failed to read fixture {relative}: {error}"))
    }

    #[test]
    fn valid_fixtures_decode() {
        for fixture in [
            "valid/hello.json",
            "valid/ready.json",
            "valid/request-diff-file.json",
            "valid/request-review-start.json",
            "valid/request-operation-cancel.json",
            "valid/response-ok.json",
            "valid/response-error.json",
            "valid/event-progress.json",
            "valid/event-state.json",
        ] {
            let bytes = read_fixture(fixture);
            let client = decode_client_line(&bytes);
            let server = decode_server_line(&bytes);
            assert!(
                client.is_ok() || server.is_ok(),
                "fixture {fixture} should decode: client={client:?} server={server:?}"
            );
        }
    }

    #[test]
    fn invalid_fixtures_are_rejected() {
        for fixture in [
            "invalid/unknown-method.json",
            "invalid/error-code.json",
            "invalid/missing-id.json",
            "invalid/unknown-field.json",
            "invalid/version-mismatch.json",
            "invalid/bad-target.json",
        ] {
            let bytes = read_fixture(fixture);
            let client = decode_client_line(&bytes);
            let server = decode_server_line(&bytes);
            assert!(
                client.is_err() && server.is_err(),
                "fixture {fixture} should be rejected by both sides"
            );
        }
    }

    #[test]
    fn version_mismatch_is_reported_as_unsupported_version() {
        let bytes = read_fixture("invalid/version-mismatch.json");
        let error = decode_client_line(&bytes).expect_err("version mismatch should fail");
        assert_eq!(error.code, ProtocolErrorCode::UnsupportedVersion);
    }

    #[test]
    fn framing_rejects_oversized_and_empty_frames() {
        let oversized = vec![b'a'; MAX_FRAME_BYTES + 1];
        assert_eq!(
            decode_client_line(&oversized).unwrap_err().code,
            ProtocolErrorCode::InvalidRequest
        );
        assert!(decode_client_line(b"\n").is_err());
        assert!(decode_client_line(b"").is_err());
    }

    #[test]
    fn encoding_escapes_newlines_into_one_frame() {
        let frame = ServerMessage::Event(OperationEventFrame {
            operation_id: "op-1".to_string(),
            sequence: 1,
            target: TargetIdentity::Local {
                local_snapshot_sha256: "a".repeat(64),
            },
            event: OperationEvent::Progress {
                message: "line one\nline two ✓".to_string(),
                completed: Some(1),
                total: Some(3),
            },
        });
        let encoded = encode_line(&frame).expect("encode");
        assert_eq!(encoded.matches('\n').count(), 1);
        assert!(encoded.ends_with('\n'));
        let decoded = decode_server_line(encoded.as_bytes()).expect("round trip");
        assert_eq!(decoded, frame);
    }

    #[test]
    fn sequence_guard_rejects_duplicates_and_backwards_progress() {
        let mut guard = SequenceGuard::new();
        assert_eq!(guard.observe("op-1", 1).expect("first"), None);
        assert_eq!(guard.observe("op-1", 2).expect("second"), Some(1));
        let duplicate = guard.observe("op-1", 2).expect_err("duplicate");
        assert_eq!(duplicate.code, ProtocolErrorCode::Conflict);
        let backwards = guard.observe("op-1", 0).expect_err("backwards");
        assert_eq!(backwards.code, ProtocolErrorCode::Conflict);
        // Different operation identities are tracked independently.
        assert_eq!(guard.observe("op-2", 1).expect("other op"), None);
        assert_eq!(guard.highest("op-1"), Some(2));
    }

    #[test]
    fn target_identity_round_trips_provider_and_local() {
        for value in [
            serde_json::json!({"kind":"provider","provider":"bitbucket","workspace":"ws","repo":"r","prId":7}),
            serde_json::json!({"kind":"local","localSnapshotSha256":"b".repeat(64)}),
        ] {
            let target: TargetIdentity = serde_json::from_value(value).expect("target");
            let encoded = serde_json::to_value(&target).expect("encode");
            let decoded: TargetIdentity = serde_json::from_value(encoded).expect("decode");
            assert_eq!(decoded, target);
        }
    }
}
