use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::administrative_audit::{
    validate_identifier as validate_audit_identifier, AdministrativeAuditAppendResult,
    AdministrativeAuditEvent,
};
use crate::finding_publication::{
    legacy_finding_marker, FindingPublicationLease, FindingPublicationRequest,
    FindingPublicationReservation, ProviderCommentIdentity,
};
use crate::review_event::PullRequestReviewEventProvider;
use crate::review_feedback::{
    derive_finding_feedback_state, ReviewFindingFeedbackAction, ReviewFindingFeedbackEvent,
    ReviewFindingFeedbackIdentity, ReviewFindingFeedbackState, ReviewFindingFeedbackTarget,
};
use crate::review_job::{
    ReviewConcurrencyLimits, ReviewJobEnqueueOutcome, ReviewJobExecution, ReviewJobIgnoredReason,
    ReviewJobRecord, ReviewJobRequest, ReviewJobSchemaVersion, ReviewJobScope,
    ReviewJobStatus as SharedReviewJobStatus,
};

const APP_DIR: &str = "lachesi";
const DB_FILE: &str = "lachesi.sqlite3";
const LEGACY_REVIEWS_DIR: &str = "reviews";
const MAX_ADMINISTRATIVE_AUDIT_TIMESTAMP_MS: i64 = 4_102_444_800_000;
const SHARED_REVIEW_JOB_LEASE_MS: i64 = 15 * 60 * 1000;
const FINDING_PUBLICATION_LEASE_MS: i64 = 5 * 60 * 1000;
const MAX_SHARED_REVIEW_JOB_ATTEMPTS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SharedReviewEventFreshness {
    Newer,
    SameRevision,
    Ambiguous,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewJobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl ReviewJobStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn from_str(value: &str) -> Self {
        match value {
            "queued" => Self::Queued,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Running,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewJob {
    pub id: String,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
    pub pr_title: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub status: ReviewJobStatus,
    pub trigger: String,
    pub thread_id: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCursorIdentity {
    pub tenant_id: String,
    pub provider: PullRequestReviewEventProvider,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewCursor {
    pub identity: ReviewCursorIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_base_sha: Option<String>,
    pub reviewed_head_sha: String,
    pub run_id: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "cursor", rename_all = "camelCase")]
pub enum ReviewCursorState {
    NotReviewed,
    Reviewed(ReviewCursor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewRunOutcome {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRunCompletion {
    pub identity: ReviewCursorIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_base_sha: Option<String>,
    pub reviewed_head_sha: String,
    pub current_head_sha: String,
    pub expected_previous_head_sha: Option<String>,
    pub run_id: String,
    pub completed_at: String,
    pub outcome: ReviewRunOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPrCount {
    pub key: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPrRiskSummary {
    pub has_ai_review: bool,
    pub impact: String,
    pub total_findings: u32,
    pub high_or_critical_findings: u32,
    pub severity_counts: Vec<ClosedPrCount>,
    pub category_counts: Vec<ClosedPrCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPrMetric {
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
    pub title: String,
    pub author_display_name: String,
    pub author_account_id: Option<String>,
    pub state: String,
    pub source_branch: String,
    pub destination_branch: String,
    pub created_on: String,
    pub updated_on: String,
    pub additions: u32,
    pub deletions: u32,
    pub files_changed: u32,
    pub diffstat_cached: bool,
    pub risk: ClosedPrRiskSummary,
    pub synced_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredReviewStore {
    #[serde(default)]
    threads: Vec<StoredReviewThread>,
    #[serde(default)]
    review_runs: Vec<StoredReviewRun>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredReviewThread {
    id: String,
    title: String,
    created_at: String,
    #[serde(default)]
    messages: Vec<StoredReviewMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredReviewMessage {
    role: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredReviewRun {
    status: String,
    source_branch: String,
    destination_branch: String,
    created_at: String,
    finished_at: Option<String>,
    thread_id: Option<String>,
}

fn local_data_dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("LACHESI_REVIEW_DATA_DIR") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        return Ok(dir);
    }
    if let Some(dir) = std::env::var_os("LACHESI_DATA_DIR") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        return Ok(dir);
    }
    let dir = dirs::data_local_dir()
        .ok_or_else(|| "Cannot determine local data directory".to_string())?
        .join(APP_DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn legacy_reviews_dir() -> Result<PathBuf, String> {
    let dir = local_data_dir()?.join(LEGACY_REVIEWS_DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn legacy_review_file_name(workspace: &str, repo: &str, id: u32) -> String {
    format!("{workspace}_{repo}_{id}.json")
}

pub fn legacy_review_path(workspace: &str, repo: &str, id: u32) -> Result<PathBuf, String> {
    Ok(legacy_reviews_dir()?.join(legacy_review_file_name(workspace, repo, id)))
}

fn db_path() -> Result<PathBuf, String> {
    Ok(local_data_dir()?.join(DB_FILE))
}

fn review_key(workspace: &str, repo: &str, id: u32) -> String {
    format!("{workspace}_{repo}_{id}")
}

fn open() -> Result<Connection, String> {
    let mut conn = Connection::open(db_path()?).map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
          version INTEGER PRIMARY KEY,
          applied_at TEXT NOT NULL DEFAULT (strftime('%s','now') || '000')
        );

        CREATE TABLE IF NOT EXISTS ai_review_stores (
          review_key TEXT PRIMARY KEY,
          workspace TEXT NOT NULL,
          repo TEXT NOT NULL,
          pr_id INTEGER NOT NULL,
          store_json TEXT NOT NULL,
          migrated_from_json INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_ai_review_stores_repo
          ON ai_review_stores(workspace, repo, pr_id);

        CREATE TABLE IF NOT EXISTS ai_review_jobs (
          id TEXT PRIMARY KEY,
          workspace TEXT NOT NULL,
          repo TEXT NOT NULL,
          pr_id INTEGER NOT NULL,
          pr_title TEXT NOT NULL,
          source_branch TEXT NOT NULL,
          destination_branch TEXT NOT NULL,
          status TEXT NOT NULL,
          trigger TEXT NOT NULL,
          thread_id TEXT,
          error TEXT,
          created_at TEXT NOT NULL,
          started_at TEXT,
          finished_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_ai_review_jobs_status
          ON ai_review_jobs(status, created_at);

        CREATE INDEX IF NOT EXISTS idx_ai_review_jobs_pr
          ON ai_review_jobs(workspace, repo, pr_id, created_at);

        CREATE TABLE IF NOT EXISTS closed_pr_metrics (
          metric_key TEXT PRIMARY KEY,
          workspace TEXT NOT NULL,
          repo TEXT NOT NULL,
          pr_id INTEGER NOT NULL,
          title TEXT NOT NULL,
          author_display_name TEXT NOT NULL,
          author_account_id TEXT,
          state TEXT NOT NULL,
          source_branch TEXT NOT NULL,
          destination_branch TEXT NOT NULL,
          created_on TEXT NOT NULL,
          updated_on TEXT NOT NULL,
          additions INTEGER NOT NULL DEFAULT 0,
          deletions INTEGER NOT NULL DEFAULT 0,
          files_changed INTEGER NOT NULL DEFAULT 0,
          diffstat_cached INTEGER NOT NULL DEFAULT 0,
          has_ai_review INTEGER NOT NULL DEFAULT 0,
          impact TEXT NOT NULL DEFAULT 'low',
          total_findings INTEGER NOT NULL DEFAULT 0,
          high_or_critical_findings INTEGER NOT NULL DEFAULT 0,
          severity_counts_json TEXT NOT NULL DEFAULT '[]',
          category_counts_json TEXT NOT NULL DEFAULT '[]',
          synced_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_closed_pr_metrics_repo
          ON closed_pr_metrics(workspace, repo, updated_on);

        CREATE INDEX IF NOT EXISTS idx_closed_pr_metrics_state
          ON closed_pr_metrics(state, updated_on);

        CREATE TABLE IF NOT EXISTS review_cursors (
          tenant_id TEXT NOT NULL,
          provider TEXT NOT NULL,
          workspace TEXT NOT NULL,
          repo TEXT NOT NULL,
          pr_id INTEGER NOT NULL,
          reviewed_head_sha TEXT NOT NULL,
          run_id TEXT NOT NULL,
          completed_at TEXT NOT NULL,
          PRIMARY KEY (tenant_id, provider, workspace, repo, pr_id)
        );

        "#,
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version) VALUES (1)",
        [],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_migrations(version) VALUES (2)",
        [],
    )
    .map_err(|e| e.to_string())?;
    let migration = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let has_reviewed_base_sha = migration
        .prepare("PRAGMA table_info(review_cursors)")
        .and_then(|mut statement| {
            let names = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(names.iter().any(|name| name == "reviewed_base_sha"))
        })
        .map_err(|error| error.to_string())?;
    if !has_reviewed_base_sha {
        migration
            .execute(
                "ALTER TABLE review_cursors ADD COLUMN reviewed_base_sha TEXT",
                [],
            )
            .map_err(|error| error.to_string())?;
    }
    migration
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS review_finding_feedback_events (
              tenant_id TEXT NOT NULL,
              event_id TEXT NOT NULL,
              provider TEXT NOT NULL,
              workspace TEXT NOT NULL,
              repo TEXT NOT NULL,
              pr_id INTEGER NOT NULL CHECK (pr_id > 0),
              review_run_id TEXT NOT NULL,
              finding_fingerprint TEXT NOT NULL,
              action TEXT NOT NULL CHECK (
                action IN ('accepted', 'dismissed', 'false_positive', 'fixed', 'reopened')
              ),
              occurred_at TEXT NOT NULL,
              actor_id TEXT NOT NULL,
              reason TEXT,
              PRIMARY KEY (tenant_id, event_id)
            );

            CREATE INDEX IF NOT EXISTS idx_review_finding_feedback_target
              ON review_finding_feedback_events(
                tenant_id, provider, workspace, repo, pr_id,
                review_run_id, finding_fingerprint, occurred_at, event_id
              );
            "#,
        )
        .map_err(|error| error.to_string())?;
    migration
        .execute(
            "INSERT OR IGNORE INTO schema_migrations(version) VALUES (3)",
            [],
        )
        .map_err(|error| error.to_string())?;
    migration.commit().map_err(|error| error.to_string())?;

    let migration = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    migration
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS team_audit_settings (
              tenant_id TEXT PRIMARY KEY,
              enabled INTEGER NOT NULL CHECK (enabled IN (0, 1))
            );

            CREATE TABLE IF NOT EXISTS administrative_audit_delivery_receipts (
              tenant_id TEXT NOT NULL,
              delivery_id TEXT NOT NULL,
              occurred_at_ms INTEGER NOT NULL CHECK (
                typeof(occurred_at_ms) = 'integer'
                AND occurred_at_ms BETWEEN 0 AND 4102444800000
              ),
              PRIMARY KEY (tenant_id, delivery_id)
            );

            CREATE TABLE IF NOT EXISTS administrative_audit_events (
              tenant_id TEXT NOT NULL,
              delivery_id TEXT NOT NULL,
              occurred_at_ms INTEGER NOT NULL CHECK (
                typeof(occurred_at_ms) = 'integer'
                AND occurred_at_ms BETWEEN 0 AND 4102444800000
              ),
              event_json TEXT NOT NULL,
              PRIMARY KEY (tenant_id, delivery_id),
              FOREIGN KEY (tenant_id, delivery_id)
                REFERENCES administrative_audit_delivery_receipts(tenant_id, delivery_id)
                ON DELETE RESTRICT
            );

            CREATE TABLE IF NOT EXISTS administrative_audit_purge_authorizations (
              tenant_id TEXT PRIMARY KEY,
              occurred_before_ms INTEGER NOT NULL CHECK (
                typeof(occurred_before_ms) = 'integer'
                AND occurred_before_ms BETWEEN 0 AND 4102444800000
              )
            );

            CREATE TABLE IF NOT EXISTS administrative_audit_retention_watermarks (
              tenant_id TEXT PRIMARY KEY,
              occurred_before_ms INTEGER NOT NULL CHECK (
                typeof(occurred_before_ms) = 'integer'
                AND occurred_before_ms BETWEEN 0 AND 4102444800000
              )
            );

            CREATE INDEX IF NOT EXISTS idx_administrative_audit_export
              ON administrative_audit_events(tenant_id, occurred_at_ms, delivery_id);

            CREATE TRIGGER IF NOT EXISTS administrative_audit_events_immutable
            BEFORE UPDATE ON administrative_audit_events
            BEGIN
              SELECT RAISE(ABORT, 'administrative audit events are immutable');
            END;

            CREATE TRIGGER IF NOT EXISTS administrative_audit_events_append_only
            BEFORE DELETE ON administrative_audit_events
            WHEN NOT EXISTS (
              SELECT 1
              FROM administrative_audit_purge_authorizations
              WHERE tenant_id = OLD.tenant_id
                AND OLD.occurred_at_ms < occurred_before_ms
            )
            BEGIN
              SELECT RAISE(ABORT, 'administrative audit events are append-only');
            END;
            "#,
        )
        .map_err(|error| error.to_string())?;
    migration
        .execute(
            "INSERT OR IGNORE INTO schema_migrations(version) VALUES (4)",
            [],
        )
        .map_err(|error| error.to_string())?;
    migration.commit().map_err(|error| error.to_string())?;

    let migration = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    migration
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS shared_review_jobs (
              id TEXT PRIMARY KEY,
              tenant_id TEXT NOT NULL,
              provider TEXT NOT NULL CHECK (provider IN ('github', 'bitbucket')),
              delivery_id TEXT NOT NULL,
              workspace TEXT NOT NULL,
              repo TEXT NOT NULL,
              pr_id INTEGER NOT NULL CHECK (pr_id > 0),
              trigger TEXT NOT NULL CHECK (
                trigger IN ('opened', 'reopened', 'synchronized', 'ready_for_review')
              ),
              base_ref_name TEXT NOT NULL,
              base_sha TEXT NOT NULL,
              head_ref_name TEXT NOT NULL,
              head_sha TEXT NOT NULL,
              scope_kind TEXT NOT NULL CHECK (
                scope_kind IN ('full_branch', 'incremental')
              ),
              previous_head_sha TEXT,
              status TEXT NOT NULL CHECK (
                status IN ('queued', 'running', 'completed', 'failed', 'cancelled')
              ),
              run_id TEXT,
              error_code TEXT,
              attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
              lease_expires_at_ms INTEGER CHECK (lease_expires_at_ms >= 0),
              created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
              started_at_ms INTEGER CHECK (started_at_ms >= 0),
              finished_at_ms INTEGER CHECK (finished_at_ms >= 0),
              UNIQUE (tenant_id, provider, delivery_id),
              CHECK (
                (scope_kind = 'full_branch' AND previous_head_sha IS NULL)
                OR
                (scope_kind = 'incremental' AND previous_head_sha IS NOT NULL)
              )
            );

            CREATE INDEX IF NOT EXISTS idx_shared_review_jobs_queue
              ON shared_review_jobs(status, created_at_ms, id);

            CREATE INDEX IF NOT EXISTS idx_shared_review_jobs_repository
              ON shared_review_jobs(
                tenant_id, provider, workspace, repo, status, created_at_ms
              );

            CREATE INDEX IF NOT EXISTS idx_shared_review_jobs_pull_request
              ON shared_review_jobs(
                tenant_id, provider, workspace, repo, pr_id, status, created_at_ms
              );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_shared_review_jobs_active_head
              ON shared_review_jobs(
                tenant_id, provider, workspace, repo, pr_id, base_sha, head_sha
              )
              WHERE status IN ('queued', 'running', 'completed', 'failed');

            CREATE TABLE IF NOT EXISTS shared_review_pull_request_state (
              tenant_id TEXT NOT NULL,
              provider TEXT NOT NULL CHECK (provider IN ('github', 'bitbucket')),
              workspace TEXT NOT NULL,
              repo TEXT NOT NULL,
              pr_id INTEGER NOT NULL CHECK (pr_id > 0),
              current_base_sha TEXT NOT NULL,
              current_head_sha TEXT NOT NULL,
              provider_updated_at_ms INTEGER NOT NULL CHECK (provider_updated_at_ms >= 0),
              reviewable INTEGER NOT NULL CHECK (reviewable IN (0, 1)),
              ambiguous INTEGER NOT NULL DEFAULT 0 CHECK (ambiguous IN (0, 1)),
              updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0),
              PRIMARY KEY (tenant_id, provider, workspace, repo, pr_id)
            );
            "#,
        )
        .map_err(|error| error.to_string())?;
    migration
        .execute(
            "INSERT OR IGNORE INTO schema_migrations(version) VALUES (5)",
            [],
        )
        .map_err(|error| error.to_string())?;
    migration.commit().map_err(|error| error.to_string())?;

    let migration = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    migration
        .execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS shared_finding_publications (
              marker TEXT PRIMARY KEY,
              tenant_id TEXT NOT NULL,
              provider TEXT NOT NULL CHECK (provider IN ('github', 'bitbucket')),
              workspace TEXT NOT NULL,
              repo TEXT NOT NULL,
              pr_id INTEGER NOT NULL CHECK (pr_id > 0),
              head_sha TEXT NOT NULL,
              finding_fingerprint TEXT NOT NULL,
              status TEXT NOT NULL CHECK (status IN ('publishing', 'published')),
              lease_token TEXT,
              lease_expires_at_ms INTEGER CHECK (lease_expires_at_ms >= 0),
              comment_id TEXT,
              created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
              updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= 0),
              CHECK (
                (status = 'publishing' AND lease_token IS NOT NULL
                  AND lease_expires_at_ms IS NOT NULL AND comment_id IS NULL)
                OR
                (status = 'published' AND lease_token IS NULL
                  AND lease_expires_at_ms IS NULL AND comment_id IS NOT NULL)
              )
            );

            CREATE INDEX IF NOT EXISTS idx_shared_finding_publications_target
              ON shared_finding_publications(
                tenant_id, provider, workspace, repo, pr_id, head_sha
              );
            "#,
        )
        .map_err(|error| error.to_string())?;
    migration
        .execute(
            "INSERT OR IGNORE INTO schema_migrations(version) VALUES (6)",
            [],
        )
        .map_err(|error| error.to_string())?;
    migration.commit().map_err(|error| error.to_string())?;

    let migration = conn
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let has_publication_base_sha = migration
        .prepare("PRAGMA table_info(shared_finding_publications)")
        .and_then(|mut statement| {
            let names = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(names.iter().any(|name| name == "base_sha"))
        })
        .map_err(|error| error.to_string())?;
    if !has_publication_base_sha {
        migration
            .execute(
                "ALTER TABLE shared_finding_publications ADD COLUMN base_sha TEXT NOT NULL DEFAULT ''",
                [],
            )
            .map_err(|error| error.to_string())?;
    }
    migration
        .execute_batch(
            r#"
            DROP INDEX IF EXISTS idx_shared_finding_publications_target;
            CREATE INDEX idx_shared_finding_publications_target
              ON shared_finding_publications(
                tenant_id, provider, workspace, repo, pr_id, base_sha, head_sha
              );
            "#,
        )
        .map_err(|error| error.to_string())?;
    migration
        .execute(
            "INSERT OR IGNORE INTO schema_migrations(version) VALUES (7)",
            [],
        )
        .map_err(|error| error.to_string())?;
    migration.commit().map_err(|error| error.to_string())?;
    Ok(())
}

fn now_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

pub fn set_team_audit_collection_enabled(tenant_id: &str, enabled: bool) -> Result<(), String> {
    validate_audit_identifier("tenantId", tenant_id).map_err(|error| error.to_string())?;
    let conn = open()?;
    conn.execute(
        r#"
        INSERT INTO team_audit_settings (tenant_id, enabled)
        VALUES (?1, ?2)
        ON CONFLICT(tenant_id) DO UPDATE SET enabled = excluded.enabled
        "#,
        params![tenant_id, if enabled { 1 } else { 0 }],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn team_audit_collection_enabled(tenant_id: &str) -> Result<bool, String> {
    validate_audit_identifier("tenantId", tenant_id).map_err(|error| error.to_string())?;
    let conn = open()?;
    team_audit_collection_enabled_from(&conn, tenant_id)
}

fn team_audit_collection_enabled_from(conn: &Connection, tenant_id: &str) -> Result<bool, String> {
    let enabled = conn
        .query_row(
            "SELECT enabled FROM team_audit_settings WHERE tenant_id = ?1",
            params![tenant_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    Ok(enabled.unwrap_or(1) == 1)
}

pub fn append_administrative_audit_event(
    event: &AdministrativeAuditEvent,
) -> Result<AdministrativeAuditAppendResult, String> {
    let event = event
        .prepare_for_storage()
        .map_err(|error| error.to_string())?;
    // The redacted v1 event is the idempotency payload. Sensitive raw values are
    // intentionally neither stored nor hashed into the audit trail.
    let event_json = serde_json::to_string(&event).map_err(|error| error.to_string())?;
    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let occurred_at_ms = event
        .occurred_at
        .parse::<i64>()
        .expect("prepared audit timestamp must parse");
    let retention_watermark = transaction
        .query_row(
            r#"
            SELECT occurred_before_ms
            FROM administrative_audit_retention_watermarks
            WHERE tenant_id = ?1
            "#,
            params![event.tenant_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if retention_watermark.is_some_and(|watermark| occurred_at_ms < watermark) {
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(AdministrativeAuditAppendResult::Duplicate);
    }
    let existing = transaction
        .query_row(
            r#"
            SELECT 1
            FROM administrative_audit_delivery_receipts
            WHERE tenant_id = ?1 AND delivery_id = ?2
            "#,
            params![event.tenant_id, event.delivery_id],
            |_| Ok(()),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if existing.is_some() {
        let collected_event = transaction
            .query_row(
                r#"
                SELECT event_json
                FROM administrative_audit_events
                WHERE tenant_id = ?1 AND delivery_id = ?2
                "#,
                params![event.tenant_id, event.delivery_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if collected_event
            .as_ref()
            .is_some_and(|existing| existing != &event_json)
        {
            return Err(
                "`deliveryId` is already associated with a different audit event".to_string(),
            );
        }
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(AdministrativeAuditAppendResult::Duplicate);
    }
    transaction
        .execute(
            r#"
            INSERT INTO administrative_audit_delivery_receipts (
              tenant_id, delivery_id, occurred_at_ms
            )
            VALUES (?1, ?2, ?3)
            "#,
            params![event.tenant_id, event.delivery_id, occurred_at_ms],
        )
        .map_err(|error| error.to_string())?;
    if !team_audit_collection_enabled_from(&transaction, &event.tenant_id)? {
        // A content-free receipt preserves at-least-once delivery semantics
        // without collecting an administrative audit event.
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(AdministrativeAuditAppendResult::CollectionDisabled);
    }
    transaction
        .execute(
            r#"
            INSERT INTO administrative_audit_events (
              tenant_id, delivery_id, occurred_at_ms, event_json
            )
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                event.tenant_id,
                event.delivery_id,
                occurred_at_ms,
                event_json
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(AdministrativeAuditAppendResult::Appended)
}

#[cfg(test)]
fn export_administrative_audit_jsonl(tenant_id: &str) -> Result<String, String> {
    let mut output = Vec::new();
    write_administrative_audit_jsonl(tenant_id, &mut output)?;
    String::from_utf8(output).map_err(|_| "Administrative audit export is not UTF-8.".to_string())
}

pub fn write_administrative_audit_jsonl<W: Write>(
    tenant_id: &str,
    writer: &mut W,
) -> Result<(), String> {
    validate_audit_identifier("tenantId", tenant_id).map_err(|error| error.to_string())?;
    let conn = open()?;
    let mut statement = conn
        .prepare(
            r#"
            SELECT delivery_id, event_json
            FROM administrative_audit_events
            WHERE tenant_id = ?1
            ORDER BY occurred_at_ms ASC, delivery_id ASC
            "#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(params![tenant_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| error.to_string())?;
    for row in rows {
        let (delivery_id, event_json) = row.map_err(|error| error.to_string())?;
        let event: AdministrativeAuditEvent =
            serde_json::from_str(&event_json).map_err(|_| "Stored audit event is invalid.")?;
        event
            .validate_stored()
            .map_err(|_| "Stored audit event is invalid.")?;
        if event.tenant_id != tenant_id || event.delivery_id != delivery_id {
            return Err("Stored audit event is invalid.".to_string());
        }
        serde_json::to_writer(&mut *writer, &event).map_err(|error| error.to_string())?;
        writer.write_all(b"\n").map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn purge_administrative_audit_events_before(
    tenant_id: &str,
    occurred_before_ms: i64,
) -> Result<usize, String> {
    validate_audit_identifier("tenantId", tenant_id).map_err(|error| error.to_string())?;
    if !(0..=MAX_ADMINISTRATIVE_AUDIT_TIMESTAMP_MS).contains(&occurred_before_ms) {
        return Err("Audit purge cutoff is outside the supported epoch range.".to_string());
    }
    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            r#"
            INSERT INTO administrative_audit_retention_watermarks (
              tenant_id, occurred_before_ms
            )
            VALUES (?1, ?2)
            ON CONFLICT(tenant_id) DO UPDATE SET
              occurred_before_ms = MAX(
                administrative_audit_retention_watermarks.occurred_before_ms,
                excluded.occurred_before_ms
              )
            "#,
            params![tenant_id, occurred_before_ms],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            r#"
            INSERT INTO administrative_audit_purge_authorizations (
              tenant_id, occurred_before_ms
            )
            VALUES (?1, ?2)
            ON CONFLICT(tenant_id) DO UPDATE
              SET occurred_before_ms = excluded.occurred_before_ms
            "#,
            params![tenant_id, occurred_before_ms],
        )
        .map_err(|error| error.to_string())?;
    let deleted = transaction
        .execute(
            r#"
            DELETE FROM administrative_audit_events
            WHERE tenant_id = ?1 AND occurred_at_ms < ?2
            "#,
            params![tenant_id, occurred_before_ms],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            "DELETE FROM administrative_audit_purge_authorizations WHERE tenant_id = ?1",
            params![tenant_id],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(deleted)
}

pub fn record_finding_feedback(
    event: &ReviewFindingFeedbackEvent,
) -> Result<ReviewFindingFeedbackState, String> {
    event.validate().map_err(|error| error.to_string())?;
    let target = event.target();
    let pr_id = feedback_pr_id(&event.identity)?;
    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let existing = transaction
        .query_row(
            r#"
            SELECT event_id, tenant_id, provider, workspace, repo, pr_id,
              review_run_id, finding_fingerprint, action, occurred_at, actor_id, reason
            FROM review_finding_feedback_events
            WHERE tenant_id = ?1 AND event_id = ?2
            "#,
            params![event.identity.tenant_id, event.event_id],
            row_to_finding_feedback_event,
        )
        .optional()
        .map_err(|error| error.to_string())?;
    if let Some(existing) = existing {
        if existing != *event {
            return Err(
                "`eventId` is already associated with different reviewer feedback".to_string(),
            );
        }
    } else {
        transaction
            .execute(
                r#"
                INSERT INTO review_finding_feedback_events (
                  tenant_id, event_id, provider, workspace, repo, pr_id,
                  review_run_id, finding_fingerprint, action, occurred_at, actor_id, reason
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                "#,
                params![
                    event.identity.tenant_id,
                    event.event_id,
                    event.identity.provider.as_str(),
                    event.identity.workspace,
                    event.identity.repo,
                    pr_id,
                    event.review_run_id,
                    event.finding_fingerprint,
                    event.action.as_str(),
                    event.occurred_at,
                    event.actor_id,
                    event.reason
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    let state = get_finding_feedback_state_from(&transaction, &target)?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(state)
}

pub fn get_finding_feedback_state(
    target: &ReviewFindingFeedbackTarget,
) -> Result<ReviewFindingFeedbackState, String> {
    target.validate().map_err(|error| error.to_string())?;
    let conn = open()?;
    get_finding_feedback_state_from(&conn, target)
}

fn get_finding_feedback_state_from(
    conn: &Connection,
    target: &ReviewFindingFeedbackTarget,
) -> Result<ReviewFindingFeedbackState, String> {
    let mut statement = conn
        .prepare(
            r#"
            SELECT event_id, tenant_id, provider, workspace, repo, pr_id,
              review_run_id, finding_fingerprint, action, occurred_at, actor_id, reason
            FROM review_finding_feedback_events
            WHERE tenant_id = ?1
              AND provider = ?2
              AND workspace = ?3
              AND repo = ?4
              AND pr_id = ?5
              AND review_run_id = ?6
              AND finding_fingerprint = ?7
            ORDER BY CAST(occurred_at AS INTEGER) ASC, event_id ASC
            "#,
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map(
            params![
                target.identity.tenant_id,
                target.identity.provider.as_str(),
                target.identity.workspace,
                target.identity.repo,
                feedback_pr_id(&target.identity)?,
                target.review_run_id,
                target.finding_fingerprint
            ],
            row_to_finding_feedback_event,
        )
        .map_err(|error| error.to_string())?;
    let mut events = Vec::new();
    for row in rows {
        events.push(row.map_err(|error| error.to_string())?);
    }
    derive_finding_feedback_state(events).map_err(|error| error.to_string())
}

fn row_to_finding_feedback_event(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ReviewFindingFeedbackEvent> {
    let provider = match row.get::<_, String>(2)?.as_str() {
        "github" => PullRequestReviewEventProvider::Github,
        "bitbucket" => PullRequestReviewEventProvider::Bitbucket,
        _ => {
            return Err(feedback_conversion_error(
                2,
                rusqlite::types::Type::Text,
                "unsupported feedback provider",
            ));
        }
    };
    let action =
        ReviewFindingFeedbackAction::from_str(&row.get::<_, String>(8)?).ok_or_else(|| {
            feedback_conversion_error(
                8,
                rusqlite::types::Type::Text,
                "unsupported feedback action",
            )
        })?;
    let pr_id = u64::try_from(row.get::<_, i64>(5)?).map_err(|_| {
        feedback_conversion_error(
            5,
            rusqlite::types::Type::Integer,
            "invalid feedback pull-request id",
        )
    })?;
    let reason = row
        .get::<_, Option<String>>(11)?
        .filter(|reason| !reason.trim().is_empty());
    Ok(ReviewFindingFeedbackEvent {
        event_id: row.get(0)?,
        identity: ReviewFindingFeedbackIdentity {
            tenant_id: row.get(1)?,
            provider,
            workspace: row.get(3)?,
            repo: row.get(4)?,
            pr_id,
        },
        review_run_id: row.get(6)?,
        finding_fingerprint: row.get(7)?,
        action,
        occurred_at: row.get(9)?,
        actor_id: row.get(10)?,
        reason,
    })
}

fn feedback_conversion_error(
    column: usize,
    data_type: rusqlite::types::Type,
    message: &'static str,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        data_type,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message,
        )),
    )
}

fn feedback_pr_id(identity: &ReviewFindingFeedbackIdentity) -> Result<i64, String> {
    if identity.pr_id == 0 {
        return Err("`prId` must be a positive integer".to_string());
    }
    i64::try_from(identity.pr_id).map_err(|_| "`prId` exceeds the supported range".to_string())
}

pub fn get_review_cursor(identity: &ReviewCursorIdentity) -> Result<ReviewCursorState, String> {
    validate_cursor_identity(identity)?;
    let conn = open()?;
    get_review_cursor_from(&conn, identity)
}

pub fn record_review_completion(
    completion: &ReviewRunCompletion,
) -> Result<ReviewCursorState, String> {
    validate_cursor_identity(&completion.identity)?;
    validate_review_completion(completion)?;
    if completion.outcome != ReviewRunOutcome::Succeeded {
        let conn = open()?;
        return get_review_cursor_from(&conn, &completion.identity);
    }
    if completion.reviewed_head_sha != completion.current_head_sha {
        let conn = open()?;
        return get_review_cursor_from(&conn, &completion.identity);
    }

    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|e| e.to_string())?;
    let current = get_review_cursor_from(&transaction, &completion.identity)?;
    let current_head_sha = match &current {
        ReviewCursorState::NotReviewed => None,
        ReviewCursorState::Reviewed(cursor) => Some(cursor.reviewed_head_sha.as_str()),
    };
    if current_head_sha != completion.expected_previous_head_sha.as_deref() {
        transaction.commit().map_err(|e| e.to_string())?;
        return Ok(current);
    }
    transaction
        .execute(
            r#"
            INSERT INTO review_cursors (
              tenant_id, provider, workspace, repo, pr_id,
              reviewed_base_sha, reviewed_head_sha, run_id, completed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(tenant_id, provider, workspace, repo, pr_id) DO UPDATE SET
              reviewed_base_sha = excluded.reviewed_base_sha,
              reviewed_head_sha = excluded.reviewed_head_sha,
              run_id = excluded.run_id,
              completed_at = excluded.completed_at
            "#,
            params![
                completion.identity.tenant_id,
                completion.identity.provider.as_str(),
                completion.identity.workspace,
                completion.identity.repo,
                cursor_pr_id(&completion.identity)?,
                completion.reviewed_base_sha,
                completion.reviewed_head_sha,
                completion.run_id,
                completion.completed_at
            ],
        )
        .map_err(|e| e.to_string())?;
    let state = get_review_cursor_from(&transaction, &completion.identity)?;
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(state)
}

fn get_review_cursor_from(
    conn: &Connection,
    identity: &ReviewCursorIdentity,
) -> Result<ReviewCursorState, String> {
    let cursor = conn
        .query_row(
            r#"
            SELECT reviewed_base_sha, reviewed_head_sha, run_id, completed_at
            FROM review_cursors
            WHERE tenant_id = ?1
              AND provider = ?2
              AND workspace = ?3
              AND repo = ?4
              AND pr_id = ?5
            "#,
            params![
                identity.tenant_id,
                identity.provider.as_str(),
                identity.workspace,
                identity.repo,
                cursor_pr_id(identity)?
            ],
            |row| {
                Ok(ReviewCursor {
                    identity: identity.clone(),
                    reviewed_base_sha: row.get(0)?,
                    reviewed_head_sha: row.get(1)?,
                    run_id: row.get(2)?,
                    completed_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(match cursor {
        Some(cursor) => ReviewCursorState::Reviewed(cursor),
        None => ReviewCursorState::NotReviewed,
    })
}

fn validate_cursor_identity(identity: &ReviewCursorIdentity) -> Result<(), String> {
    require_cursor_value("tenantId", &identity.tenant_id)?;
    require_cursor_value("workspace", &identity.workspace)?;
    require_cursor_value("repo", &identity.repo)?;
    cursor_pr_id(identity)?;
    Ok(())
}

fn validate_review_completion(completion: &ReviewRunCompletion) -> Result<(), String> {
    require_cursor_value("runId", &completion.run_id)?;
    validate_cursor_sha("reviewedHeadSha", &completion.reviewed_head_sha)?;
    if let Some(reviewed_base_sha) = &completion.reviewed_base_sha {
        validate_cursor_sha("reviewedBaseSha", reviewed_base_sha)?;
    }
    validate_cursor_sha("currentHeadSha", &completion.current_head_sha)?;
    if let Some(previous_head_sha) = &completion.expected_previous_head_sha {
        validate_cursor_sha("expectedPreviousHeadSha", previous_head_sha)?;
    }
    let completed_at = completion
        .completed_at
        .parse::<i64>()
        .map_err(|_| "`completedAt` must be a non-negative Unix timestamp in milliseconds")?;
    if completed_at < 0 {
        return Err(
            "`completedAt` must be a non-negative Unix timestamp in milliseconds".to_string(),
        );
    }
    Ok(())
}

fn validate_cursor_sha(field: &str, value: &str) -> Result<(), String> {
    if matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("`{field}` must be a full hexadecimal commit SHA"))
    }
}

fn require_cursor_value(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("`{field}` must not be empty"))
    } else {
        Ok(())
    }
}

fn cursor_pr_id(identity: &ReviewCursorIdentity) -> Result<i64, String> {
    if identity.pr_id == 0 {
        return Err("`prId` must be a positive integer".to_string());
    }
    i64::try_from(identity.pr_id).map_err(|_| "`prId` exceeds the supported range".to_string())
}

const SHARED_REVIEW_JOB_SELECT: &str = r#"
    SELECT
      id, tenant_id, provider, delivery_id, workspace, repo, pr_id, trigger,
      base_ref_name, base_sha, head_ref_name, head_sha, scope_kind,
      previous_head_sha, status, run_id, error_code,
      attempt_count, lease_expires_at_ms,
      created_at_ms, started_at_ms, finished_at_ms
    FROM shared_review_jobs
"#;

pub(crate) fn enqueue_shared_review_job(
    event: &crate::review_event::PullRequestReviewEvent,
) -> Result<ReviewJobEnqueueOutcome, String> {
    validate_shared_review_event_for_storage(event)?;
    if event.kind == crate::review_event::PullRequestReviewEventKind::Closed || event.draft {
        return Err("Only non-draft, open pull-request events can be enqueued".to_string());
    }

    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;

    if let Some(existing) = shared_review_job_by_delivery_from(&transaction, event)? {
        if !shared_job_matches_event(&existing, event) {
            return Err("Delivery id conflicts with an existing review job".to_string());
        }
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(ReviewJobEnqueueOutcome::DuplicateDelivery(Box::new(
            existing,
        )));
    }
    let freshness = shared_review_event_freshness(&transaction, event)?;
    if freshness == SharedReviewEventFreshness::Stale {
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(ReviewJobEnqueueOutcome::Ignored {
            reason: ReviewJobIgnoredReason::Stale,
            cancelled_queued_jobs: 0,
        });
    }
    let now = now_ms_i64();
    if let Some(existing) = shared_review_job_by_head_from(&transaction, event)? {
        if freshness == SharedReviewEventFreshness::Ambiguous {
            mark_shared_review_event_ambiguous(&transaction, event, now)?;
        } else {
            mark_shared_review_event_current(&transaction, event, now)?;
        }
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(ReviewJobEnqueueOutcome::DuplicateHead(Some(Box::new(
            existing,
        ))));
    }

    let identity = cursor_identity_from_event(event);
    let cursor = get_review_cursor_from(&transaction, &identity)?;
    if matches!(
        &cursor,
        ReviewCursorState::Reviewed(cursor)
            if cursor.reviewed_base_sha.as_deref() == Some(event.base.sha.as_str())
                && cursor.reviewed_head_sha == event.head.sha
    ) {
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(ReviewJobEnqueueOutcome::DuplicateHead(None));
    }
    let scope = match cursor {
        ReviewCursorState::Reviewed(cursor)
            if cursor.reviewed_base_sha.as_deref() == Some(event.base.sha.as_str()) =>
        {
            ReviewJobScope::Incremental {
                previous_head_sha: cursor.reviewed_head_sha,
                current_head_sha: event.head.sha.clone(),
            }
        }
        ReviewCursorState::NotReviewed | ReviewCursorState::Reviewed(_) => {
            ReviewJobScope::FullBranch {
                base_sha: event.base.sha.clone(),
                head_sha: event.head.sha.clone(),
            }
        }
    };
    let request = ReviewJobRequest {
        schema_version: ReviewJobSchemaVersion::V1,
        id: shared_review_job_id(event),
        tenant_id: event.tenant_id.clone(),
        provider: event.provider,
        delivery_id: event.delivery_id.clone(),
        workspace: event.workspace.clone(),
        repository: event.repository.clone(),
        pull_request_id: event.pull_request_id,
        trigger: event.kind,
        base: event.base.clone(),
        head: event.head.clone(),
        scope,
    };
    let (scope_kind, previous_head_sha) = shared_scope_columns(&request.scope);
    transaction
        .execute(
            r#"
            INSERT INTO shared_review_jobs (
              id, tenant_id, provider, delivery_id, workspace, repo, pr_id,
              trigger, base_ref_name, base_sha, head_ref_name, head_sha,
              scope_kind, previous_head_sha, status, run_id, error_code,
              attempt_count, lease_expires_at_ms,
              created_at_ms, started_at_ms, finished_at_ms
            )
            VALUES (
              ?1, ?2, ?3, ?4, ?5, ?6, ?7,
              ?8, ?9, ?10, ?11, ?12,
              ?13, ?14, 'queued', NULL, NULL,
              0, NULL, ?15, NULL, NULL
            )
            "#,
            params![
                request.id,
                request.tenant_id,
                request.provider.as_str(),
                request.delivery_id,
                request.workspace,
                request.repository,
                shared_review_pr_id(request.pull_request_id)?,
                shared_event_kind(request.trigger)?,
                request.base.ref_name,
                request.base.sha,
                request.head.ref_name,
                request.head.sha,
                scope_kind,
                previous_head_sha,
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    if freshness == SharedReviewEventFreshness::Ambiguous {
        mark_shared_review_event_ambiguous(&transaction, event, now)?;
    } else {
        mark_shared_review_event_current(&transaction, event, now)?;
    }
    let record = get_shared_review_job_from(&transaction, &request.id)?
        .ok_or_else(|| "Failed to reload queued shared review job".to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(ReviewJobEnqueueOutcome::Queued(Box::new(record)))
}

pub(crate) fn suppress_shared_review_jobs(
    event: &crate::review_event::PullRequestReviewEvent,
    reason: ReviewJobIgnoredReason,
) -> Result<usize, String> {
    validate_shared_review_event_for_storage(event)?;
    match reason {
        ReviewJobIgnoredReason::Draft if !event.draft => {
            return Err("Draft suppression requires a draft event".to_string());
        }
        ReviewJobIgnoredReason::Closed
            if event.kind != crate::review_event::PullRequestReviewEventKind::Closed =>
        {
            return Err("Closed suppression requires a closed event".to_string());
        }
        ReviewJobIgnoredReason::Stale => {
            return Err("Stale events are ignored by freshness checks".to_string());
        }
        _ => {}
    }

    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let now = now_ms_i64();
    let freshness = shared_review_event_freshness(&transaction, event)?;
    if freshness == SharedReviewEventFreshness::Stale {
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(0);
    }
    if freshness == SharedReviewEventFreshness::Ambiguous {
        mark_shared_review_event_ambiguous(&transaction, event, now)?;
        transaction.commit().map_err(|error| error.to_string())?;
        return Ok(0);
    }
    upsert_shared_pull_request_state(&transaction, event, false, now)?;
    let reason_code = match reason {
        ReviewJobIgnoredReason::Draft => "pull_request_draft",
        ReviewJobIgnoredReason::Closed => "pull_request_closed",
        ReviewJobIgnoredReason::Stale => unreachable!("stale suppression is rejected above"),
    };
    let cancelled = transaction
        .execute(
            r#"
            UPDATE shared_review_jobs
            SET status = 'cancelled',
                error_code = ?1,
                finished_at_ms = ?2
            WHERE tenant_id = ?3
              AND provider = ?4
              AND workspace = ?5
              AND repo = ?6
              AND pr_id = ?7
              AND status = 'queued'
            "#,
            params![
                reason_code,
                now,
                event.tenant_id,
                event.provider.as_str(),
                event.workspace,
                event.repository,
                shared_review_pr_id(event.pull_request_id)?,
            ],
        )
        .map_err(|error| error.to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(cancelled)
}

pub(crate) fn claim_next_shared_review_job(
    limits: ReviewConcurrencyLimits,
) -> Result<Option<ReviewJobRecord>, String> {
    let limits = limits.validate()?;
    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let now = now_ms_i64();
    transaction
        .execute(
            r#"
            UPDATE shared_review_jobs
            SET status = 'failed',
                error_code = 'worker_lease_exhausted',
                lease_expires_at_ms = NULL,
                finished_at_ms = ?1
            WHERE status = 'running'
              AND lease_expires_at_ms IS NOT NULL
              AND lease_expires_at_ms <= ?1
              AND attempt_count >= ?2
            "#,
            params![now, i64::from(MAX_SHARED_REVIEW_JOB_ATTEMPTS)],
        )
        .map_err(|error| error.to_string())?;
    transaction
        .execute(
            r#"
            UPDATE shared_review_jobs
            SET status = 'queued',
                error_code = 'worker_lease_expired',
                started_at_ms = NULL,
                lease_expires_at_ms = NULL
            WHERE status = 'running'
              AND lease_expires_at_ms IS NOT NULL
              AND lease_expires_at_ms <= ?1
              AND attempt_count < ?2
            "#,
            params![now, i64::from(MAX_SHARED_REVIEW_JOB_ATTEMPTS)],
        )
        .map_err(|error| error.to_string())?;
    let queued_ids = {
        let mut statement = transaction
            .prepare(
                r#"
                SELECT id
                FROM shared_review_jobs
                WHERE status = 'queued'
                ORDER BY created_at_ms, id
                "#,
            )
            .map_err(|error| error.to_string())?;
        let queued_ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| error.to_string())?;
        queued_ids
    };

    for job_id in queued_ids {
        let Some(job) = get_shared_review_job_from(&transaction, &job_id)? else {
            continue;
        };
        let repository_running = shared_running_count(&transaction, &job, false)?;
        let pull_request_running = shared_running_count(&transaction, &job, true)?;
        if repository_running >= limits.per_repository
            || pull_request_running >= limits.per_pull_request
        {
            continue;
        }
        let changed = transaction
            .execute(
                r#"
                UPDATE shared_review_jobs
                SET status = 'running',
                    attempt_count = attempt_count + 1,
                    error_code = NULL,
                    started_at_ms = ?1,
                    lease_expires_at_ms = ?2
                WHERE id = ?3 AND status = 'queued'
                "#,
                params![now, now.saturating_add(SHARED_REVIEW_JOB_LEASE_MS), job_id],
            )
            .map_err(|error| error.to_string())?;
        if changed == 1 {
            let claimed = get_shared_review_job_from(&transaction, &job_id)?
                .ok_or_else(|| "Failed to reload claimed shared review job".to_string())?;
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(Some(claimed));
        }
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(None)
}

pub(crate) fn finish_shared_review_job(
    job_id: &str,
    expected_attempt_count: u32,
    execution: &ReviewJobExecution,
) -> Result<ReviewJobRecord, String> {
    if job_id.trim().is_empty() {
        return Err("`jobId` must not be empty".to_string());
    }
    let (requested_status, run_id, requested_error_code) = shared_execution_columns(execution)?;
    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let current = get_shared_review_job_from(&transaction, job_id)?
        .ok_or_else(|| format!("Unknown shared review job: {job_id}"))?;
    if matches!(
        current.status,
        SharedReviewJobStatus::Completed
            | SharedReviewJobStatus::Failed
            | SharedReviewJobStatus::Cancelled
    ) {
        let matching_terminal_state = current.status == requested_status
            && current.error_code.as_deref() == requested_error_code;
        let matching_stale_completion = requested_status == SharedReviewJobStatus::Completed
            && current.status == SharedReviewJobStatus::Cancelled
            && current.error_code.as_deref() == Some("stale_pull_request_state");
        if current.attempt_count == expected_attempt_count
            && (matching_terminal_state || matching_stale_completion)
            && current.run_id.as_deref() == run_id
        {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(current);
        }
        return Err("Shared review job already has a different terminal state".to_string());
    }
    if current.status != SharedReviewJobStatus::Running {
        return Err("Only a running shared review job can be finished".to_string());
    }
    if current.attempt_count != expected_attempt_count {
        return Err("Shared review job lease belongs to a different attempt".to_string());
    }

    let finished_at = now_ms_i64();
    let lease_expires_at = current
        .lease_expires_at
        .as_deref()
        .ok_or_else(|| "Running shared review job has no active lease".to_string())?
        .parse::<i64>()
        .map_err(|_| "Running shared review job has an invalid lease".to_string())?;
    if lease_expires_at <= finished_at {
        return Err("Shared review job lease expired before completion".to_string());
    }
    let (target_status, error_code) = if requested_status == SharedReviewJobStatus::Completed {
        if advance_shared_review_cursor_if_current(&transaction, &current, run_id, finished_at)? {
            (SharedReviewJobStatus::Completed, requested_error_code)
        } else {
            (
                SharedReviewJobStatus::Cancelled,
                Some("stale_pull_request_state"),
            )
        }
    } else {
        (requested_status, requested_error_code)
    };
    let changed = transaction
        .execute(
            r#"
            UPDATE shared_review_jobs
            SET status = ?1,
                run_id = ?2,
                error_code = ?3,
                lease_expires_at_ms = NULL,
                finished_at_ms = ?4
            WHERE id = ?5
              AND status = 'running'
              AND attempt_count = ?6
              AND lease_expires_at_ms > ?7
            "#,
            params![
                target_status.as_str(),
                run_id,
                error_code,
                finished_at,
                job_id,
                i64::from(expected_attempt_count),
                finished_at,
            ],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("Shared review job lease changed before completion".to_string());
    }
    let finished = get_shared_review_job_from(&transaction, job_id)?
        .ok_or_else(|| "Failed to reload finished shared review job".to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(finished)
}

pub(crate) fn renew_shared_review_job_lease(
    job_id: &str,
    expected_attempt_count: u32,
) -> Result<ReviewJobRecord, String> {
    if job_id.trim().is_empty() {
        return Err("`jobId` must not be empty".to_string());
    }
    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let now = now_ms_i64();
    let changed = transaction
        .execute(
            r#"
            UPDATE shared_review_jobs
            SET lease_expires_at_ms = ?1
            WHERE id = ?2
              AND status = 'running'
              AND attempt_count = ?3
              AND lease_expires_at_ms > ?4
            "#,
            params![
                now.saturating_add(SHARED_REVIEW_JOB_LEASE_MS),
                job_id,
                i64::from(expected_attempt_count),
                now,
            ],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("Shared review job lease is expired or belongs to another attempt".to_string());
    }
    let renewed = get_shared_review_job_from(&transaction, job_id)?
        .ok_or_else(|| "Failed to reload renewed shared review job".to_string())?;
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(renewed)
}

pub(crate) fn get_shared_review_job(job_id: &str) -> Result<Option<ReviewJobRecord>, String> {
    if job_id.trim().is_empty() {
        return Err("`jobId` must not be empty".to_string());
    }
    let conn = open()?;
    get_shared_review_job_from(&conn, job_id)
}

fn shared_review_job_by_delivery_from(
    conn: &Connection,
    event: &crate::review_event::PullRequestReviewEvent,
) -> Result<Option<ReviewJobRecord>, String> {
    let id = conn
        .query_row(
            r#"
            SELECT id
            FROM shared_review_jobs
            WHERE tenant_id = ?1 AND provider = ?2 AND delivery_id = ?3
            "#,
            params![event.tenant_id, event.provider.as_str(), event.delivery_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    id.map(|id| get_shared_review_job_from(conn, &id))
        .transpose()
        .map(Option::flatten)
}

fn shared_review_job_by_head_from(
    conn: &Connection,
    event: &crate::review_event::PullRequestReviewEvent,
) -> Result<Option<ReviewJobRecord>, String> {
    let id = conn
        .query_row(
            r#"
            SELECT id
            FROM shared_review_jobs
            WHERE tenant_id = ?1
              AND provider = ?2
              AND workspace = ?3
              AND repo = ?4
              AND pr_id = ?5
              AND base_sha = ?6
              AND head_sha = ?7
              AND status IN ('queued', 'running', 'completed', 'failed')
            "#,
            params![
                event.tenant_id,
                event.provider.as_str(),
                event.workspace,
                event.repository,
                shared_review_pr_id(event.pull_request_id)?,
                event.base.sha,
                event.head.sha,
            ],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    id.map(|id| get_shared_review_job_from(conn, &id))
        .transpose()
        .map(Option::flatten)
}

fn get_shared_review_job_from(
    conn: &Connection,
    job_id: &str,
) -> Result<Option<ReviewJobRecord>, String> {
    let sql = format!("{SHARED_REVIEW_JOB_SELECT} WHERE id = ?1");
    conn.query_row(&sql, params![job_id], row_to_shared_review_job)
        .optional()
        .map_err(|error| error.to_string())
}

fn row_to_shared_review_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewJobRecord> {
    let provider = parse_shared_provider(&row.get::<_, String>(2)?)
        .map_err(|message| shared_job_conversion_error(2, message))?;
    let trigger = parse_shared_event_kind(&row.get::<_, String>(7)?)
        .map_err(|message| shared_job_conversion_error(7, message))?;
    let base = crate::review_event::PullRequestRevision {
        ref_name: row.get(8)?,
        sha: row.get(9)?,
    };
    let head = crate::review_event::PullRequestRevision {
        ref_name: row.get(10)?,
        sha: row.get(11)?,
    };
    let scope_kind: String = row.get(12)?;
    let previous_head_sha: Option<String> = row.get(13)?;
    let scope = match (scope_kind.as_str(), previous_head_sha) {
        ("full_branch", None) => ReviewJobScope::FullBranch {
            base_sha: base.sha.clone(),
            head_sha: head.sha.clone(),
        },
        ("incremental", Some(previous_head_sha)) => ReviewJobScope::Incremental {
            previous_head_sha,
            current_head_sha: head.sha.clone(),
        },
        _ => {
            return Err(shared_job_conversion_error(
                12,
                "Invalid shared review job scope",
            ));
        }
    };
    let status = SharedReviewJobStatus::from_str(&row.get::<_, String>(14)?)
        .map_err(|message| shared_job_conversion_error(14, &message))?;
    Ok(ReviewJobRecord {
        request: ReviewJobRequest {
            schema_version: ReviewJobSchemaVersion::V1,
            id: row.get(0)?,
            tenant_id: row.get(1)?,
            provider,
            delivery_id: row.get(3)?,
            workspace: row.get(4)?,
            repository: row.get(5)?,
            pull_request_id: u64::try_from(row.get::<_, i64>(6)?).map_err(|_| {
                shared_job_conversion_error(6, "Invalid shared review job pull-request id")
            })?,
            trigger,
            base,
            head,
            scope,
        },
        status,
        attempt_count: u32::try_from(row.get::<_, i64>(17)?).map_err(|_| {
            shared_job_conversion_error(17, "Invalid shared review job attempt count")
        })?,
        lease_expires_at: row
            .get::<_, Option<i64>>(18)?
            .map(|value| value.to_string()),
        run_id: row.get(15)?,
        error_code: row.get(16)?,
        created_at: row.get::<_, i64>(19)?.to_string(),
        started_at: row
            .get::<_, Option<i64>>(20)?
            .map(|value| value.to_string()),
        finished_at: row
            .get::<_, Option<i64>>(21)?
            .map(|value| value.to_string()),
    })
}

fn shared_running_count(
    conn: &Connection,
    job: &ReviewJobRecord,
    include_pull_request: bool,
) -> Result<usize, String> {
    let mut sql = r#"
        SELECT COUNT(*)
        FROM shared_review_jobs
        WHERE tenant_id = ?1
          AND provider = ?2
          AND workspace = ?3
          AND repo = ?4
          AND status = 'running'
    "#
    .to_string();
    if include_pull_request {
        sql.push_str(" AND pr_id = ?5");
    }
    let count: i64 = if include_pull_request {
        conn.query_row(
            &sql,
            params![
                job.request.tenant_id,
                job.request.provider.as_str(),
                job.request.workspace,
                job.request.repository,
                shared_review_pr_id(job.request.pull_request_id)?,
            ],
            |row| row.get(0),
        )
    } else {
        conn.query_row(
            &sql,
            params![
                job.request.tenant_id,
                job.request.provider.as_str(),
                job.request.workspace,
                job.request.repository,
            ],
            |row| row.get(0),
        )
    }
    .map_err(|error| error.to_string())?;
    usize::try_from(count).map_err(|_| "Invalid running review job count".to_string())
}

fn shared_review_event_freshness(
    conn: &Connection,
    event: &crate::review_event::PullRequestReviewEvent,
) -> Result<SharedReviewEventFreshness, String> {
    let event_updated_at_ms = event
        .provider_updated_at_ms
        .ok_or_else(|| "`providerUpdatedAtMs` is required".to_string())?;
    let state = conn
        .query_row(
            r#"
            SELECT current_base_sha, current_head_sha, provider_updated_at_ms, ambiguous
            FROM shared_review_pull_request_state
            WHERE tenant_id = ?1
              AND provider = ?2
              AND workspace = ?3
              AND repo = ?4
              AND pr_id = ?5
            "#,
            params![
                event.tenant_id,
                event.provider.as_str(),
                event.workspace,
                event.repository,
                shared_review_pr_id(event.pull_request_id)?,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((current_base_sha, current_head_sha, provider_updated_at_ms, ambiguous)) = state
    else {
        return Ok(SharedReviewEventFreshness::Newer);
    };
    if event_updated_at_ms < provider_updated_at_ms {
        return Ok(SharedReviewEventFreshness::Stale);
    }
    if event_updated_at_ms > provider_updated_at_ms {
        return Ok(SharedReviewEventFreshness::Newer);
    }
    if ambiguous == 1 {
        return Ok(SharedReviewEventFreshness::Ambiguous);
    }
    if event.base.sha == current_base_sha && event.head.sha == current_head_sha {
        Ok(SharedReviewEventFreshness::SameRevision)
    } else {
        Ok(SharedReviewEventFreshness::Ambiguous)
    }
}

fn upsert_shared_pull_request_state(
    conn: &Connection,
    event: &crate::review_event::PullRequestReviewEvent,
    reviewable: bool,
    now: i64,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO shared_review_pull_request_state (
          tenant_id, provider, workspace, repo, pr_id,
          current_base_sha, current_head_sha, provider_updated_at_ms,
          reviewable, ambiguous, updated_at_ms
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?10)
        ON CONFLICT(tenant_id, provider, workspace, repo, pr_id) DO UPDATE SET
          current_base_sha = excluded.current_base_sha,
          current_head_sha = excluded.current_head_sha,
          provider_updated_at_ms = excluded.provider_updated_at_ms,
          reviewable = excluded.reviewable,
          ambiguous = 0,
          updated_at_ms = excluded.updated_at_ms
        WHERE excluded.provider_updated_at_ms
                > shared_review_pull_request_state.provider_updated_at_ms
           OR (
                excluded.provider_updated_at_ms
                  = shared_review_pull_request_state.provider_updated_at_ms
                AND excluded.current_base_sha
                  = shared_review_pull_request_state.current_base_sha
                AND excluded.current_head_sha
                  = shared_review_pull_request_state.current_head_sha
              )
        "#,
        params![
            event.tenant_id,
            event.provider.as_str(),
            event.workspace,
            event.repository,
            shared_review_pr_id(event.pull_request_id)?,
            event.base.sha,
            event.head.sha,
            event
                .provider_updated_at_ms
                .ok_or_else(|| "`providerUpdatedAtMs` is required".to_string())?,
            if reviewable { 1 } else { 0 },
            now,
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn mark_shared_review_event_current(
    conn: &Connection,
    event: &crate::review_event::PullRequestReviewEvent,
    now: i64,
) -> Result<(), String> {
    upsert_shared_pull_request_state(conn, event, true, now)?;
    conn.execute(
        r#"
        UPDATE shared_review_jobs
        SET status = 'cancelled',
            error_code = 'superseded',
            finished_at_ms = ?1
        WHERE tenant_id = ?2
          AND provider = ?3
          AND workspace = ?4
          AND repo = ?5
          AND pr_id = ?6
          AND status = 'queued'
          AND (base_sha <> ?7 OR head_sha <> ?8)
        "#,
        params![
            now,
            event.tenant_id,
            event.provider.as_str(),
            event.workspace,
            event.repository,
            shared_review_pr_id(event.pull_request_id)?,
            event.base.sha,
            event.head.sha,
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn mark_shared_review_event_ambiguous(
    conn: &Connection,
    event: &crate::review_event::PullRequestReviewEvent,
    now: i64,
) -> Result<(), String> {
    let changed = conn
        .execute(
            r#"
            UPDATE shared_review_pull_request_state
            SET reviewable = 0,
                ambiguous = 1,
                updated_at_ms = ?1
            WHERE tenant_id = ?2
              AND provider = ?3
              AND workspace = ?4
              AND repo = ?5
              AND pr_id = ?6
              AND provider_updated_at_ms = ?7
            "#,
            params![
                now,
                event.tenant_id,
                event.provider.as_str(),
                event.workspace,
                event.repository,
                shared_review_pr_id(event.pull_request_id)?,
                event
                    .provider_updated_at_ms
                    .ok_or_else(|| "`providerUpdatedAtMs` is required".to_string())?,
            ],
        )
        .map_err(|error| error.to_string())?;
    if changed != 1 {
        return Err("Failed to persist ambiguous pull-request revision state".to_string());
    }
    Ok(())
}

fn advance_shared_review_cursor_if_current(
    conn: &Connection,
    job: &ReviewJobRecord,
    run_id: Option<&str>,
    completed_at: i64,
) -> Result<bool, String> {
    let Some(run_id) = run_id else {
        return Err("Completed review jobs require a run id".to_string());
    };
    let state = conn
        .query_row(
            r#"
            SELECT current_base_sha, current_head_sha, reviewable
            FROM shared_review_pull_request_state
            WHERE tenant_id = ?1
              AND provider = ?2
              AND workspace = ?3
              AND repo = ?4
              AND pr_id = ?5
            "#,
            params![
                job.request.tenant_id,
                job.request.provider.as_str(),
                job.request.workspace,
                job.request.repository,
                shared_review_pr_id(job.request.pull_request_id)?,
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((current_base_sha, current_head_sha, reviewable)) = state else {
        return Ok(false);
    };
    if reviewable != 1
        || current_base_sha != job.request.base.sha
        || current_head_sha != job.request.head.sha
    {
        return Ok(false);
    }

    let identity = ReviewCursorIdentity {
        tenant_id: job.request.tenant_id.clone(),
        provider: job.request.provider,
        workspace: job.request.workspace.clone(),
        repo: job.request.repository.clone(),
        pr_id: job.request.pull_request_id,
    };
    let current_cursor = get_review_cursor_from(conn, &identity)?;
    let (current_cursor_base, current_cursor_head) = match &current_cursor {
        ReviewCursorState::NotReviewed => (None, None),
        ReviewCursorState::Reviewed(cursor) => (
            cursor.reviewed_base_sha.as_deref(),
            Some(cursor.reviewed_head_sha.as_str()),
        ),
    };
    let cursor_matches_scope = match &job.request.scope {
        ReviewJobScope::FullBranch { .. } => true,
        ReviewJobScope::Incremental {
            previous_head_sha, ..
        } => {
            current_cursor_base == Some(job.request.base.sha.as_str())
                && current_cursor_head == Some(previous_head_sha.as_str())
        }
    };
    if !cursor_matches_scope {
        return Ok(false);
    }
    conn.execute(
        r#"
        INSERT INTO review_cursors (
          tenant_id, provider, workspace, repo, pr_id,
          reviewed_base_sha, reviewed_head_sha, run_id, completed_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(tenant_id, provider, workspace, repo, pr_id) DO UPDATE SET
          reviewed_base_sha = excluded.reviewed_base_sha,
          reviewed_head_sha = excluded.reviewed_head_sha,
          run_id = excluded.run_id,
          completed_at = excluded.completed_at
        "#,
        params![
            identity.tenant_id,
            identity.provider.as_str(),
            identity.workspace,
            identity.repo,
            cursor_pr_id(&identity)?,
            job.request.base.sha,
            job.request.head.sha,
            run_id,
            completed_at.to_string(),
        ],
    )
    .map_err(|error| error.to_string())?;
    Ok(true)
}

fn shared_execution_columns(
    execution: &ReviewJobExecution,
) -> Result<(SharedReviewJobStatus, Option<&str>, Option<&str>), String> {
    match execution {
        ReviewJobExecution::Completed { run_id } => {
            validate_shared_metadata("runId", run_id, 512)?;
            Ok((
                SharedReviewJobStatus::Completed,
                Some(run_id.as_str()),
                None,
            ))
        }
        ReviewJobExecution::Failed { run_id, error_code } => {
            validate_shared_job_code("errorCode", error_code)?;
            if let Some(run_id) = run_id {
                validate_shared_metadata("runId", run_id, 512)?;
            }
            Ok((
                SharedReviewJobStatus::Failed,
                run_id.as_deref(),
                Some(error_code.as_str()),
            ))
        }
        ReviewJobExecution::Cancelled {
            run_id,
            reason_code,
        } => {
            if let Some(run_id) = run_id {
                validate_shared_metadata("runId", run_id, 512)?;
            }
            if let Some(reason_code) = reason_code {
                validate_shared_job_code("reasonCode", reason_code)?;
            }
            Ok((
                SharedReviewJobStatus::Cancelled,
                run_id.as_deref(),
                reason_code.as_deref(),
            ))
        }
    }
}

fn validate_shared_review_event_for_storage(
    event: &crate::review_event::PullRequestReviewEvent,
) -> Result<(), String> {
    event.validate().map_err(|error| error.to_string())?;
    if event.provider_updated_at_ms.is_none() {
        return Err(
            "`providerUpdatedAtMs` is required for automated review coordination".to_string(),
        );
    }
    for (field, value) in [
        ("tenantId", event.tenant_id.as_str()),
        ("deliveryId", event.delivery_id.as_str()),
        ("workspace", event.workspace.as_str()),
        ("repository", event.repository.as_str()),
    ] {
        validate_shared_metadata(field, value, 512)?;
    }
    validate_shared_metadata("base.refName", &event.base.ref_name, 1024)?;
    validate_shared_metadata("head.refName", &event.head.ref_name, 1024)?;
    Ok(())
}

fn validate_shared_metadata(field: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty()
        || value != value.trim()
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        Err(format!(
            "`{field}` must be a bounded, non-control metadata value"
        ))
    } else {
        Ok(())
    }
}

fn validate_shared_job_code(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b'.')
        })
    {
        Err(format!(
            "`{field}` must be a non-empty stable lowercase code"
        ))
    } else {
        Ok(())
    }
}

fn shared_job_matches_event(
    job: &ReviewJobRecord,
    event: &crate::review_event::PullRequestReviewEvent,
) -> bool {
    job.request.tenant_id == event.tenant_id
        && job.request.provider == event.provider
        && job.request.workspace == event.workspace
        && job.request.repository == event.repository
        && job.request.pull_request_id == event.pull_request_id
        && job.request.trigger == event.kind
        && job.request.base == event.base
        && job.request.head == event.head
}

fn cursor_identity_from_event(
    event: &crate::review_event::PullRequestReviewEvent,
) -> ReviewCursorIdentity {
    ReviewCursorIdentity {
        tenant_id: event.tenant_id.clone(),
        provider: event.provider,
        workspace: event.workspace.clone(),
        repo: event.repository.clone(),
        pr_id: event.pull_request_id,
    }
}

fn shared_review_job_id(event: &crate::review_event::PullRequestReviewEvent) -> String {
    let mut digest = Sha256::new();
    for value in [
        event.tenant_id.as_bytes(),
        event.provider.as_str().as_bytes(),
        event.workspace.as_bytes(),
        event.repository.as_bytes(),
        event.head.sha.as_bytes(),
        event.delivery_id.as_bytes(),
    ] {
        digest.update(value.len().to_be_bytes());
        digest.update(value);
    }
    digest.update(event.pull_request_id.to_be_bytes());
    format!("job-{}", hex::encode(digest.finalize()))
}

fn shared_scope_columns(scope: &ReviewJobScope) -> (&'static str, Option<&str>) {
    match scope {
        ReviewJobScope::FullBranch { .. } => ("full_branch", None),
        ReviewJobScope::Incremental {
            previous_head_sha, ..
        } => ("incremental", Some(previous_head_sha)),
    }
}

fn shared_event_kind(
    kind: crate::review_event::PullRequestReviewEventKind,
) -> Result<&'static str, String> {
    match kind {
        crate::review_event::PullRequestReviewEventKind::Opened => Ok("opened"),
        crate::review_event::PullRequestReviewEventKind::Reopened => Ok("reopened"),
        crate::review_event::PullRequestReviewEventKind::Synchronized => Ok("synchronized"),
        crate::review_event::PullRequestReviewEventKind::ReadyForReview => Ok("ready_for_review"),
        crate::review_event::PullRequestReviewEventKind::Closed => {
            Err("Closed events cannot create review jobs".to_string())
        }
    }
}

fn parse_shared_event_kind(
    value: &str,
) -> Result<crate::review_event::PullRequestReviewEventKind, &'static str> {
    match value {
        "opened" => Ok(crate::review_event::PullRequestReviewEventKind::Opened),
        "reopened" => Ok(crate::review_event::PullRequestReviewEventKind::Reopened),
        "synchronized" => Ok(crate::review_event::PullRequestReviewEventKind::Synchronized),
        "ready_for_review" => Ok(crate::review_event::PullRequestReviewEventKind::ReadyForReview),
        _ => Err("Invalid shared review job trigger"),
    }
}

fn parse_shared_provider(value: &str) -> Result<PullRequestReviewEventProvider, &'static str> {
    match value {
        "github" => Ok(PullRequestReviewEventProvider::Github),
        "bitbucket" => Ok(PullRequestReviewEventProvider::Bitbucket),
        _ => Err("Invalid shared review job provider"),
    }
}

fn shared_review_pr_id(pr_id: u64) -> Result<i64, String> {
    if pr_id == 0 {
        return Err("`pullRequestId` must be positive".to_string());
    }
    i64::try_from(pr_id).map_err(|_| "`pullRequestId` exceeds the supported range".to_string())
}

fn now_ms_i64() -> i64 {
    now_ms().parse().unwrap_or(0)
}

type StoredFindingPublicationRow = (
    String,
    String,
    String,
    String,
    i64,
    String,
    String,
    String,
    String,
    Option<i64>,
    Option<String>,
);

fn load_finding_publication_row(
    conn: &Connection,
    marker: &str,
) -> Result<Option<StoredFindingPublicationRow>, String> {
    conn.query_row(
        r#"
        SELECT tenant_id, provider, workspace, repo, pr_id, base_sha, head_sha,
               finding_fingerprint, status, lease_expires_at_ms, comment_id
        FROM shared_finding_publications
        WHERE marker = ?1
        "#,
        params![marker],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        },
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub(crate) fn reserve_finding_publication(
    request: &FindingPublicationRequest,
    marker: &str,
    lease_token: &str,
) -> Result<FindingPublicationReservation, String> {
    request.validate()?;
    if marker.trim().is_empty() || lease_token.trim().is_empty() {
        return Err("Publication marker and lease token are required.".to_string());
    }
    let pr_id = shared_review_pr_id(request.pull_request_id)?;
    let canonical_base_sha = request.base_sha.to_ascii_lowercase();
    let canonical_head_sha = request.head_sha.to_ascii_lowercase();
    let canonical_workspace = request.workspace.to_ascii_lowercase();
    let canonical_repository = request.repository.to_ascii_lowercase();
    let now = now_ms_i64();
    let lease_expires_at = now.saturating_add(FINDING_PUBLICATION_LEASE_MS);
    let mut conn = open()?;
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| error.to_string())?;
    let mut reserved_marker = marker.to_string();
    let mut existing = load_finding_publication_row(&transaction, marker)?;
    if existing.is_none() {
        // v6 markers had no base SHA. Keep that marker for remote-comment
        // recovery while treating its empty base as an explicit legacy identity.
        let legacy_marker = legacy_finding_marker(request);
        if legacy_marker != marker {
            let legacy = load_finding_publication_row(&transaction, &legacy_marker)?;
            if legacy.as_ref().is_some_and(|row| row.5.is_empty()) {
                reserved_marker = legacy_marker;
                existing = legacy;
            }
        }
    }
    let legacy_reservation = reserved_marker != marker;

    if let Some((
        tenant_id,
        provider,
        workspace,
        repo,
        stored_pr_id,
        base_sha,
        head_sha,
        finding_fingerprint,
        status,
        stored_lease_expires_at,
        comment_id,
    )) = existing
    {
        let base_matches = if legacy_reservation {
            base_sha.is_empty()
        } else {
            base_sha.eq_ignore_ascii_case(&request.base_sha)
        };
        if tenant_id != request.tenant_id
            || provider != request.provider.as_str()
            || !workspace.eq_ignore_ascii_case(&request.workspace)
            || !repo.eq_ignore_ascii_case(&request.repository)
            || stored_pr_id != pr_id
            || !base_matches
            || !head_sha.eq_ignore_ascii_case(&request.head_sha)
            || finding_fingerprint != request.finding_fingerprint
        {
            return Err("Publication marker identity collision.".to_string());
        }
        if status == "published" {
            let comment_id = comment_id
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    "Published finding is missing its provider comment id.".to_string()
                })?;
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(FindingPublicationReservation::Published {
                identity: ProviderCommentIdentity { comment_id },
                marker: reserved_marker,
            });
        }
        if stored_lease_expires_at.is_some_and(|expires_at| expires_at > now) {
            transaction.commit().map_err(|error| error.to_string())?;
            return Ok(FindingPublicationReservation::InProgress);
        }
        let updated = transaction
            .execute(
                r#"
                UPDATE shared_finding_publications
                SET lease_token = ?2, lease_expires_at_ms = ?3, updated_at_ms = ?4
                WHERE marker = ?1 AND status = 'publishing'
                  AND lease_expires_at_ms <= ?4
                "#,
                params![reserved_marker, lease_token, lease_expires_at, now],
            )
            .map_err(|error| error.to_string())?;
        if updated != 1 {
            return Err("Publication reservation changed while being reclaimed.".to_string());
        }
    } else {
        transaction
            .execute(
                r#"
                INSERT INTO shared_finding_publications (
                  marker, tenant_id, provider, workspace, repo, pr_id, base_sha,
                  head_sha, finding_fingerprint, status, lease_token,
                  lease_expires_at_ms, comment_id, created_at_ms, updated_at_ms
                )
                VALUES (
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'publishing', ?10, ?11,
                  NULL, ?12, ?12
                )
                "#,
                params![
                    marker,
                    request.tenant_id,
                    request.provider.as_str(),
                    canonical_workspace,
                    canonical_repository,
                    pr_id,
                    canonical_base_sha,
                    canonical_head_sha,
                    request.finding_fingerprint,
                    lease_token,
                    lease_expires_at,
                    now,
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    transaction.commit().map_err(|error| error.to_string())?;
    Ok(FindingPublicationReservation::Acquired(
        FindingPublicationLease {
            marker: reserved_marker,
            token: lease_token.to_string(),
        },
    ))
}

pub(crate) fn complete_finding_publication(
    lease: &FindingPublicationLease,
    identity: &ProviderCommentIdentity,
) -> Result<(), String> {
    if identity.comment_id.trim().is_empty() {
        return Err("Provider comment id is required.".to_string());
    }
    let conn = open()?;
    let updated = conn
        .execute(
            r#"
            UPDATE shared_finding_publications
            SET status = 'published', lease_token = NULL, lease_expires_at_ms = NULL,
                comment_id = ?3, updated_at_ms = ?4
            WHERE marker = ?1 AND status = 'publishing' AND lease_token = ?2
            "#,
            params![lease.marker, lease.token, identity.comment_id, now_ms_i64()],
        )
        .map_err(|error| error.to_string())?;
    if updated == 1 {
        Ok(())
    } else {
        Err("Publication lease was fenced before completion.".to_string())
    }
}

pub(crate) fn release_finding_publication(lease: &FindingPublicationLease) -> Result<(), String> {
    let conn = open()?;
    conn.execute(
        r#"
        DELETE FROM shared_finding_publications
        WHERE marker = ?1 AND status = 'publishing' AND lease_token = ?2
        "#,
        params![lease.marker, lease.token],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn shared_job_conversion_error(column: usize, message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        column,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        )),
    )
}

pub fn load_review_json(workspace: &str, repo: &str, id: u32) -> Result<Option<String>, String> {
    let conn = open()?;
    let key = review_key(workspace, repo, id);
    let db_json = conn
        .query_row(
            "SELECT store_json FROM ai_review_stores WHERE review_key = ?1",
            params![key],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    if db_json.is_some() {
        return Ok(db_json);
    }

    let legacy_path = legacy_review_path(workspace, repo, id)?;
    if !legacy_path.exists() {
        return Ok(None);
    }
    let legacy_json = fs::read_to_string(&legacy_path).map_err(|e| e.to_string())?;
    save_review_json_with_migration_flag(workspace, repo, id, &legacy_json, true)?;
    Ok(Some(legacy_json))
}

pub fn save_review_json(workspace: &str, repo: &str, id: u32, json: &str) -> Result<(), String> {
    save_review_json_with_migration_flag(workspace, repo, id, json, false)
}

fn save_review_json_with_migration_flag(
    workspace: &str,
    repo: &str,
    id: u32,
    json: &str,
    migrated_from_json: bool,
) -> Result<(), String> {
    let conn = open()?;
    let key = review_key(workspace, repo, id);
    let now = now_ms();
    conn.execute(
        r#"
        INSERT INTO ai_review_stores (
          review_key, workspace, repo, pr_id, store_json, migrated_from_json, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
        ON CONFLICT(review_key) DO UPDATE SET
          store_json = excluded.store_json,
          migrated_from_json = ai_review_stores.migrated_from_json OR excluded.migrated_from_json,
          updated_at = excluded.updated_at
        "#,
        params![
            key,
            workspace,
            repo,
            i64::from(id),
            json,
            if migrated_from_json { 1 } else { 0 },
            now
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_review(workspace: &str, repo: &str, id: u32) -> Result<(), String> {
    let conn = open()?;
    let key = review_key(workspace, repo, id);
    conn.execute(
        "DELETE FROM ai_review_stores WHERE review_key = ?1",
        params![key],
    )
    .map_err(|e| e.to_string())?;
    if let Ok(path) = legacy_review_path(workspace, repo, id) {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub fn cleanup_stale_reviews(keep_keys: &[String]) -> Result<(), String> {
    let conn = open()?;
    if keep_keys.is_empty() {
        conn.execute("DELETE FROM ai_review_stores", [])
            .map_err(|e| e.to_string())?;
    } else {
        let mut stmt = conn
            .prepare("SELECT review_key FROM ai_review_stores")
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        for row in rows {
            let key = row.map_err(|e| e.to_string())?;
            if !keep_keys.contains(&key) {
                conn.execute(
                    "DELETE FROM ai_review_stores WHERE review_key = ?1",
                    params![key],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }

    cleanup_legacy_review_files(keep_keys)?;
    Ok(())
}

pub fn create_review_job(
    workspace: &str,
    repo: &str,
    pr_id: u32,
    pr_title: &str,
    source_branch: &str,
    destination_branch: &str,
    trigger: &str,
) -> Result<ReviewJob, String> {
    let conn = open()?;
    let now = now_ms();
    let job = ReviewJob {
        id: format!("job-{}", now),
        workspace: workspace.to_string(),
        repo: repo.to_string(),
        pr_id,
        pr_title: pr_title.to_string(),
        source_branch: source_branch.to_string(),
        destination_branch: destination_branch.to_string(),
        status: ReviewJobStatus::Queued,
        trigger: trigger.to_string(),
        thread_id: None,
        error: None,
        created_at: now,
        started_at: None,
        finished_at: None,
    };
    conn.execute(
        r#"
        INSERT INTO ai_review_jobs (
          id, workspace, repo, pr_id, pr_title, source_branch, destination_branch,
          status, trigger, thread_id, error, created_at, started_at, finished_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        "#,
        params![
            job.id,
            job.workspace,
            job.repo,
            i64::from(job.pr_id),
            job.pr_title,
            job.source_branch,
            job.destination_branch,
            job.status.as_str(),
            job.trigger,
            job.thread_id,
            job.error,
            job.created_at,
            job.started_at,
            job.finished_at
        ],
    )
    .map_err(|e| e.to_string())?;
    get_review_job(&job.id)?.ok_or_else(|| "Failed to reload created review job.".to_string())
}

pub fn update_review_job_status(
    id: &str,
    status: ReviewJobStatus,
    thread_id: Option<&str>,
    error: Option<&str>,
) -> Result<ReviewJob, String> {
    let conn = open()?;
    let now = now_ms();
    let started_at_expr = if status == ReviewJobStatus::Running {
        "COALESCE(started_at, ?4)"
    } else {
        "started_at"
    };
    let finished_at_expr = if matches!(
        status,
        ReviewJobStatus::Succeeded | ReviewJobStatus::Failed | ReviewJobStatus::Cancelled
    ) {
        "?4"
    } else {
        "finished_at"
    };
    let sql = format!(
        r#"
        UPDATE ai_review_jobs
        SET status = ?1,
            thread_id = COALESCE(?2, thread_id),
            error = ?3,
            started_at = {started_at_expr},
            finished_at = {finished_at_expr}
        WHERE id = ?5
        "#
    );
    conn.execute(&sql, params![status.as_str(), thread_id, error, now, id])
        .map_err(|e| e.to_string())?;
    get_review_job(id)?.ok_or_else(|| format!("Unknown review job: {id}"))
}

pub fn get_review_job(id: &str) -> Result<Option<ReviewJob>, String> {
    let conn = open()?;
    conn.query_row(
        r#"
        SELECT id, workspace, repo, pr_id, pr_title, source_branch, destination_branch,
          status, trigger, thread_id, error, created_at, started_at, finished_at
        FROM ai_review_jobs
        WHERE id = ?1
        "#,
        params![id],
        row_to_review_job,
    )
    .optional()
    .map_err(|e| e.to_string())
}

pub fn list_recent_review_jobs(limit: u32) -> Result<Vec<ReviewJob>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT id, workspace, repo, pr_id, pr_title, source_branch, destination_branch,
              status, trigger, thread_id, error, created_at, started_at, finished_at
            FROM ai_review_jobs
            ORDER BY CAST(created_at AS INTEGER) DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![i64::from(limit)], row_to_review_job)
        .map_err(|e| e.to_string())?;
    let mut jobs = Vec::new();
    for row in rows {
        jobs.push(row.map_err(|e| e.to_string())?);
    }
    let existing_thread_ids: HashSet<String> = jobs
        .iter()
        .filter_map(|job| job.thread_id.clone())
        .collect();
    let mut store_stmt = conn
        .prepare(
            r#"
            SELECT review_key, workspace, repo, pr_id, store_json, created_at, updated_at
            FROM ai_review_stores
            ORDER BY CAST(updated_at AS INTEGER) DESC
            LIMIT ?1
            "#,
        )
        .map_err(|e| e.to_string())?;
    let store_rows = store_stmt
        .query_map(params![i64::from(limit)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)? as u32,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    for row in store_rows {
        let (review_key, workspace, repo, pr_id, store_json, created_at, updated_at) =
            row.map_err(|e| e.to_string())?;
        let Ok(store) = serde_json::from_str::<StoredReviewStore>(&store_json) else {
            continue;
        };
        let StoredReviewStore {
            threads,
            review_runs,
        } = store;
        for thread in threads {
            if existing_thread_ids.contains(&thread.id) {
                continue;
            }
            let run = review_runs
                .iter()
                .rev()
                .find(|run| run.thread_id.as_deref() == Some(thread.id.as_str()));
            let status = run
                .map(|run| review_job_status_from_run(&run.status))
                .unwrap_or_else(|| {
                    if thread
                        .messages
                        .iter()
                        .any(|message| message.role == "assistant")
                    {
                        ReviewJobStatus::Succeeded
                    } else {
                        ReviewJobStatus::Failed
                    }
                });
            let terminal = matches!(
                status,
                ReviewJobStatus::Succeeded | ReviewJobStatus::Failed | ReviewJobStatus::Cancelled
            );
            let (source_branch, destination_branch) = run
                .map(|run| (run.source_branch.clone(), run.destination_branch.clone()))
                .unwrap_or_else(|| (String::new(), String::new()));
            jobs.push(ReviewJob {
                id: format!("store:{review_key}:{}", thread.id),
                workspace: workspace.clone(),
                repo: repo.clone(),
                pr_id,
                pr_title: if thread.title.trim().is_empty() {
                    format!("PR #{pr_id}")
                } else {
                    thread.title.clone()
                },
                source_branch,
                destination_branch,
                status,
                trigger: "manual".to_string(),
                thread_id: Some(thread.id),
                error: if status == ReviewJobStatus::Failed {
                    Some("Review thread has no assistant response captured.".to_string())
                } else {
                    None
                },
                created_at: run.map(|run| run.created_at.clone()).unwrap_or_else(|| {
                    if thread.created_at.is_empty() {
                        created_at.clone()
                    } else {
                        thread.created_at.clone()
                    }
                }),
                started_at: Some(created_at.clone()),
                finished_at: if terminal {
                    run.and_then(|run| run.finished_at.clone())
                        .or(Some(updated_at.clone()))
                } else {
                    None
                },
            });
        }
    }
    jobs.sort_by(|a, b| {
        parse_ms(&b.created_at)
            .cmp(&parse_ms(&a.created_at))
            .then_with(|| b.id.cmp(&a.id))
    });
    jobs.truncate(limit as usize);
    Ok(jobs)
}

fn review_job_status_from_run(status: &str) -> ReviewJobStatus {
    match status {
        "succeeded" => ReviewJobStatus::Succeeded,
        "failed" => ReviewJobStatus::Failed,
        "cancelled" => ReviewJobStatus::Cancelled,
        "queued" => ReviewJobStatus::Queued,
        _ => ReviewJobStatus::Running,
    }
}

fn parse_ms(value: &str) -> u128 {
    value.parse::<u128>().unwrap_or(0)
}

fn row_to_review_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReviewJob> {
    let status: String = row.get(7)?;
    Ok(ReviewJob {
        id: row.get(0)?,
        workspace: row.get(1)?,
        repo: row.get(2)?,
        pr_id: row.get::<_, i64>(3)? as u32,
        pr_title: row.get(4)?,
        source_branch: row.get(5)?,
        destination_branch: row.get(6)?,
        status: ReviewJobStatus::from_str(&status),
        trigger: row.get(8)?,
        thread_id: row.get(9)?,
        error: row.get(10)?,
        created_at: row.get(11)?,
        started_at: row.get(12)?,
        finished_at: row.get(13)?,
    })
}

fn metric_key(workspace: &str, repo: &str, pr_id: u32) -> String {
    format!("{workspace}_{repo}_{pr_id}")
}

fn counts_json(counts: &[ClosedPrCount]) -> Result<String, String> {
    serde_json::to_string(counts).map_err(|e| e.to_string())
}

fn parse_counts_json(value: String) -> Vec<ClosedPrCount> {
    serde_json::from_str(&value).unwrap_or_default()
}

fn sorted_counts(map: HashMap<String, u32>) -> Vec<ClosedPrCount> {
    let mut counts: Vec<ClosedPrCount> = map
        .into_iter()
        .map(|(key, count)| ClosedPrCount { key, count })
        .collect();
    counts.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));
    counts
}

fn impact_for_metrics(
    additions: u32,
    deletions: u32,
    files_changed: u32,
    total_findings: u32,
    high_or_critical_findings: u32,
) -> String {
    let churn = additions.saturating_add(deletions);
    if high_or_critical_findings > 0 || churn >= 500 || files_changed >= 20 {
        "high".to_string()
    } else if total_findings > 0 || churn >= 120 || files_changed >= 8 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

pub fn review_risk_summary(
    workspace: &str,
    repo: &str,
    pr_id: u32,
    additions: u32,
    deletions: u32,
    files_changed: u32,
) -> ClosedPrRiskSummary {
    let Ok(Some(json)) = load_review_json(workspace, repo, pr_id) else {
        return ClosedPrRiskSummary {
            impact: impact_for_metrics(additions, deletions, files_changed, 0, 0),
            ..ClosedPrRiskSummary::default()
        };
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
        return ClosedPrRiskSummary {
            has_ai_review: true,
            impact: impact_for_metrics(additions, deletions, files_changed, 0, 0),
            ..ClosedPrRiskSummary::default()
        };
    };

    let has_threads = value
        .get("threads")
        .and_then(|threads| threads.as_array())
        .is_some_and(|threads| !threads.is_empty());
    let review_runs = value
        .get("reviewRuns")
        .and_then(|runs| runs.as_array())
        .cloned()
        .unwrap_or_default();
    let has_ai_review = has_threads || !review_runs.is_empty();
    let latest_findings = review_runs
        .iter()
        .rev()
        .filter_map(|run| run.get("findings").and_then(|findings| findings.as_array()))
        .find(|findings| !findings.is_empty());

    let mut severity_counts = HashMap::new();
    let mut category_counts = HashMap::new();
    let mut total_findings = 0;
    let mut high_or_critical_findings = 0;

    if let Some(findings) = latest_findings {
        for finding in findings {
            total_findings += 1;
            let severity = finding
                .get("severity")
                .and_then(|severity| severity.as_str())
                .unwrap_or("unknown")
                .to_string();
            if severity == "high" || severity == "critical" {
                high_or_critical_findings += 1;
            }
            *severity_counts.entry(severity).or_insert(0) += 1;

            let category = finding
                .get("category")
                .and_then(|category| category.as_str())
                .unwrap_or("other")
                .to_string();
            *category_counts.entry(category).or_insert(0) += 1;
        }
    }

    ClosedPrRiskSummary {
        has_ai_review,
        impact: impact_for_metrics(
            additions,
            deletions,
            files_changed,
            total_findings,
            high_or_critical_findings,
        ),
        total_findings,
        high_or_critical_findings,
        severity_counts: sorted_counts(severity_counts),
        category_counts: sorted_counts(category_counts),
    }
}

pub fn upsert_closed_pr_metric(metric: &ClosedPrMetric) -> Result<(), String> {
    let conn = open()?;
    let severity_counts_json = counts_json(&metric.risk.severity_counts)?;
    let category_counts_json = counts_json(&metric.risk.category_counts)?;
    conn.execute(
        r#"
        INSERT INTO closed_pr_metrics (
          metric_key, workspace, repo, pr_id, title, author_display_name, author_account_id,
          state, source_branch, destination_branch, created_on, updated_on, additions, deletions,
          files_changed, diffstat_cached, has_ai_review, impact, total_findings,
          high_or_critical_findings, severity_counts_json, category_counts_json, synced_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
          ?19, ?20, ?21, ?22, ?23)
        ON CONFLICT(metric_key) DO UPDATE SET
          title = excluded.title,
          author_display_name = excluded.author_display_name,
          author_account_id = excluded.author_account_id,
          state = excluded.state,
          source_branch = excluded.source_branch,
          destination_branch = excluded.destination_branch,
          created_on = excluded.created_on,
          updated_on = excluded.updated_on,
          additions = excluded.additions,
          deletions = excluded.deletions,
          files_changed = excluded.files_changed,
          diffstat_cached = excluded.diffstat_cached,
          has_ai_review = excluded.has_ai_review,
          impact = excluded.impact,
          total_findings = excluded.total_findings,
          high_or_critical_findings = excluded.high_or_critical_findings,
          severity_counts_json = excluded.severity_counts_json,
          category_counts_json = excluded.category_counts_json,
          synced_at = excluded.synced_at
        "#,
        params![
            metric_key(&metric.workspace, &metric.repo, metric.pr_id),
            &metric.workspace,
            &metric.repo,
            i64::from(metric.pr_id),
            &metric.title,
            &metric.author_display_name,
            &metric.author_account_id,
            &metric.state,
            &metric.source_branch,
            &metric.destination_branch,
            &metric.created_on,
            &metric.updated_on,
            i64::from(metric.additions),
            i64::from(metric.deletions),
            i64::from(metric.files_changed),
            if metric.diffstat_cached { 1 } else { 0 },
            if metric.risk.has_ai_review { 1 } else { 0 },
            &metric.risk.impact,
            i64::from(metric.risk.total_findings),
            i64::from(metric.risk.high_or_critical_findings),
            severity_counts_json,
            category_counts_json,
            &metric.synced_at
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn list_closed_pr_metrics() -> Result<Vec<ClosedPrMetric>, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT workspace, repo, pr_id, title, author_display_name, author_account_id, state,
              source_branch, destination_branch, created_on, updated_on, additions, deletions,
              files_changed, diffstat_cached, has_ai_review, impact, total_findings,
              high_or_critical_findings, severity_counts_json, category_counts_json, synced_at
            FROM closed_pr_metrics
            ORDER BY updated_on DESC, workspace ASC, repo ASC, pr_id DESC
            "#,
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            let severity_counts_json: String = row.get(19)?;
            let category_counts_json: String = row.get(20)?;
            Ok(ClosedPrMetric {
                workspace: row.get(0)?,
                repo: row.get(1)?,
                pr_id: row.get::<_, i64>(2)? as u32,
                title: row.get(3)?,
                author_display_name: row.get(4)?,
                author_account_id: row.get(5)?,
                state: row.get(6)?,
                source_branch: row.get(7)?,
                destination_branch: row.get(8)?,
                created_on: row.get(9)?,
                updated_on: row.get(10)?,
                additions: row.get::<_, i64>(11)? as u32,
                deletions: row.get::<_, i64>(12)? as u32,
                files_changed: row.get::<_, i64>(13)? as u32,
                diffstat_cached: row.get::<_, i64>(14)? != 0,
                risk: ClosedPrRiskSummary {
                    has_ai_review: row.get::<_, i64>(15)? != 0,
                    impact: row.get(16)?,
                    total_findings: row.get::<_, i64>(17)? as u32,
                    high_or_critical_findings: row.get::<_, i64>(18)? as u32,
                    severity_counts: parse_counts_json(severity_counts_json),
                    category_counts: parse_counts_json(category_counts_json),
                },
                synced_at: row.get(21)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut metrics = Vec::new();
    for row in rows {
        metrics.push(row.map_err(|e| e.to_string())?);
    }
    Ok(metrics)
}

fn cleanup_legacy_review_files(keep_keys: &[String]) -> Result<(), String> {
    let dir = legacy_reviews_dir()?;
    let entries = fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if !keep_keys.contains(&stem) {
            let _ = fs::remove_file(&path);
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn database_path_for_diagnostics() -> Result<PathBuf, String> {
    db_path()
}

#[allow(dead_code)]
fn _assert_path_send_sync(_: &Path) {}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::administrative_audit::{
        AdministrativeAuditAction, AdministrativeAuditActor, AdministrativeAuditActorKind,
        AdministrativeAuditOutcome, AdministrativeAuditRepositoryScope,
        AdministrativeAuditSchemaVersion, AdministrativeAuditTarget, AdministrativeAuditTargetKind,
        REDACTED_AUDIT_VALUE,
    };
    use crate::finding_publication::{
        FindingAnchorSide, FindingLineRange, FindingPublicationSchemaVersion, FindingSeverity,
    };
    use crate::review_event::{
        PullRequestClosedOutcome, PullRequestEventActor, PullRequestReviewEvent,
        PullRequestReviewEventKind, PullRequestReviewEventSchemaVersion, PullRequestRevision,
    };

    use super::*;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn test_dir(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("lachesi-{name}-{nanos}"))
    }

    fn with_test_data_dir<T>(name: &str, f: impl FnOnce(&Path) -> T) -> T {
        let _guard = ENV_LOCK.lock().expect("test env lock");
        let dir = test_dir(name);
        std::env::set_var("LACHESI_DATA_DIR", &dir);
        let result = f(&dir);
        std::env::remove_var("LACHESI_DATA_DIR");
        let _ = fs::remove_dir_all(&dir);
        result
    }

    fn cursor_identity() -> ReviewCursorIdentity {
        ReviewCursorIdentity {
            tenant_id: "tenant-acme".to_string(),
            provider: PullRequestReviewEventProvider::Github,
            workspace: "acme".to_string(),
            repo: "payments".to_string(),
            pr_id: 42,
        }
    }

    fn finding_publication_request() -> FindingPublicationRequest {
        FindingPublicationRequest {
            schema_version: FindingPublicationSchemaVersion::V1,
            tenant_id: "tenant-acme".to_string(),
            provider: PullRequestReviewEventProvider::Github,
            workspace: "acme".to_string(),
            repository: "payments".to_string(),
            pull_request_id: 42,
            base_sha: "1111111111111111111111111111111111111111".to_string(),
            head_sha: "2222222222222222222222222222222222222222".to_string(),
            finding_fingerprint: "finding:src/lib.rs:12".to_string(),
            anchor: FindingLineRange {
                path: "src/lib.rs".to_string(),
                start_line: 12,
                end_line: 14,
                side: FindingAnchorSide::New,
            },
            title: "Guard the nullable value".to_string(),
            body: "The error path can omit this value.".to_string(),
            severity: FindingSeverity::High,
            suggested_fix: None,
        }
    }

    #[test]
    fn finding_publication_reservations_are_durable_idempotent_and_fenced() {
        with_test_data_dir("finding-publication-reservation", |_| {
            let request = finding_publication_request();
            let marker = crate::finding_publication::finding_marker(&request);
            let first = reserve_finding_publication(&request, &marker, "lease-1")
                .expect("first reservation");
            let FindingPublicationReservation::Acquired(first_lease) = first else {
                panic!("first publisher should acquire the marker");
            };

            assert_eq!(
                reserve_finding_publication(&request, &marker, "lease-2")
                    .expect("concurrent reservation"),
                FindingPublicationReservation::InProgress
            );

            let conn = open().expect("open publication database");
            conn.execute(
                "UPDATE shared_finding_publications SET lease_expires_at_ms = 0 WHERE marker = ?1",
                params![marker],
            )
            .expect("expire first lease");
            let reclaimed = reserve_finding_publication(&request, &marker, "lease-2")
                .expect("reclaim expired reservation");
            let FindingPublicationReservation::Acquired(second_lease) = reclaimed else {
                panic!("expired reservation should be reclaimed");
            };
            assert!(complete_finding_publication(
                &first_lease,
                &ProviderCommentIdentity {
                    comment_id: "stale-comment".to_string()
                }
            )
            .is_err());

            let identity = ProviderCommentIdentity {
                comment_id: "9223372036854775000".to_string(),
            };
            complete_finding_publication(&second_lease, &identity).expect("complete current lease");
            assert_eq!(
                reserve_finding_publication(&request, &marker, "lease-3")
                    .expect("idempotent published reservation"),
                FindingPublicationReservation::Published {
                    identity,
                    marker: marker.clone(),
                }
            );
            release_finding_publication(&first_lease).expect("stale release is harmless");
            assert_eq!(
                reserve_finding_publication(&request, &marker, "lease-4")
                    .expect("published state survives stale release"),
                FindingPublicationReservation::Published {
                    identity: ProviderCommentIdentity {
                        comment_id: "9223372036854775000".to_string()
                    },
                    marker,
                }
            );
        });
    }

    fn completion(
        outcome: ReviewRunOutcome,
        head_sha: &str,
        expected_previous_head_sha: Option<&str>,
        run_id: &str,
        completed_at: &str,
    ) -> ReviewRunCompletion {
        ReviewRunCompletion {
            identity: cursor_identity(),
            reviewed_base_sha: None,
            reviewed_head_sha: head_sha.to_string(),
            current_head_sha: head_sha.to_string(),
            expected_previous_head_sha: expected_previous_head_sha.map(str::to_string),
            run_id: run_id.to_string(),
            completed_at: completed_at.to_string(),
            outcome,
        }
    }

    fn shared_event(
        tenant_id: &str,
        workspace: &str,
        repo: &str,
        pr_id: u64,
        head_sha: &str,
        delivery_id: &str,
    ) -> PullRequestReviewEvent {
        PullRequestReviewEvent {
            schema_version: PullRequestReviewEventSchemaVersion::V1,
            kind: PullRequestReviewEventKind::Synchronized,
            provider: PullRequestReviewEventProvider::Github,
            tenant_id: tenant_id.to_string(),
            workspace: workspace.to_string(),
            repository: repo.to_string(),
            pull_request_id: pr_id,
            base: PullRequestRevision {
                ref_name: "main".to_string(),
                sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            },
            head: PullRequestRevision {
                ref_name: format!("feature/{pr_id}"),
                sha: head_sha.to_string(),
            },
            provider_updated_at_ms: Some(
                head_sha
                    .as_bytes()
                    .first()
                    .and_then(|byte| char::from(*byte).to_digit(16))
                    .map(i64::from)
                    .unwrap_or_default()
                    * 1_000,
            ),
            draft: false,
            closed_outcome: None,
            actor: PullRequestEventActor {
                id: "user-7".to_string(),
                login: "reviewer".to_string(),
                display_name: None,
            },
            delivery_id: delivery_id.to_string(),
        }
    }

    fn queued_shared_job(outcome: ReviewJobEnqueueOutcome) -> ReviewJobRecord {
        let ReviewJobEnqueueOutcome::Queued(job) = outcome else {
            panic!("expected a queued shared review job");
        };
        *job
    }

    fn feedback_event(
        event_id: &str,
        action: ReviewFindingFeedbackAction,
        occurred_at: &str,
    ) -> ReviewFindingFeedbackEvent {
        ReviewFindingFeedbackEvent {
            event_id: event_id.to_string(),
            identity: ReviewFindingFeedbackIdentity {
                tenant_id: "tenant-acme".to_string(),
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repo: "payments".to_string(),
                pr_id: 42,
            },
            review_run_id: "run-1".to_string(),
            finding_fingerprint: "finding-abc".to_string(),
            action,
            occurred_at: occurred_at.to_string(),
            actor_id: "reviewer-1".to_string(),
            reason: Some(format!("reason for {}", action.as_str())),
        }
    }

    fn audit_event(delivery_id: &str, occurred_at: &str) -> AdministrativeAuditEvent {
        AdministrativeAuditEvent {
            schema_version: AdministrativeAuditSchemaVersion::V1,
            delivery_id: delivery_id.to_string(),
            tenant_id: "tenant-acme".to_string(),
            occurred_at: occurred_at.to_string(),
            actor: AdministrativeAuditActor {
                kind: AdministrativeAuditActorKind::User,
                id: "user:reviewer-1".to_string(),
            },
            repository: AdministrativeAuditRepositoryScope {
                provider: PullRequestReviewEventProvider::Github,
                workspace: "acme".to_string(),
                repo: "payments".to_string(),
                pr_id: Some(42),
            },
            action: AdministrativeAuditAction::AutomatedReviewTriggered,
            target: AdministrativeAuditTarget {
                kind: AdministrativeAuditTargetKind::ReviewRun,
                id: "run:1".to_string(),
            },
            outcome: AdministrativeAuditOutcome::Succeeded,
            correlation_id: "correlation:1".to_string(),
        }
    }

    #[test]
    fn dedicated_review_data_dir_is_created() {
        let _guard = ENV_LOCK.lock().expect("test env lock");
        let dir = test_dir("dedicated-review-data");
        std::env::set_var("LACHESI_REVIEW_DATA_DIR", &dir);

        let resolved = local_data_dir().expect("resolve dedicated review data dir");

        std::env::remove_var("LACHESI_REVIEW_DATA_DIR");
        assert_eq!(resolved, dir);
        assert!(dir.is_dir());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saves_and_loads_review_json_from_sqlite() {
        with_test_data_dir("roundtrip", |dir| {
            save_review_json("workspace", "repo", 123, r#"{"threads":[]}"#).expect("save review");

            assert!(dir.join(DB_FILE).exists());
            let loaded = load_review_json("workspace", "repo", 123).expect("load review");
            assert_eq!(loaded.as_deref(), Some(r#"{"threads":[]}"#));
        });
    }

    #[test]
    fn administrative_audit_append_is_idempotent_and_immutable() {
        with_test_data_dir("administrative-audit-immutable", |dir| {
            let event = audit_event("delivery-1", "1000");

            assert_eq!(
                append_administrative_audit_event(&event).expect("append event"),
                AdministrativeAuditAppendResult::Appended
            );
            assert_eq!(
                append_administrative_audit_event(&event).expect("duplicate event"),
                AdministrativeAuditAppendResult::Duplicate
            );

            let mut conflicting = event;
            conflicting.outcome = AdministrativeAuditOutcome::Failed;
            assert_eq!(
                append_administrative_audit_event(&conflicting).expect_err("conflicting delivery"),
                "`deliveryId` is already associated with a different audit event"
            );

            let conn = Connection::open(dir.join(DB_FILE)).expect("open audit database");
            conn.execute(
                r#"
                UPDATE administrative_audit_events
                SET event_json = '{}'
                WHERE tenant_id = 'tenant-acme' AND delivery_id = 'delivery-1'
                "#,
                [],
            )
            .expect_err("audit row is immutable");
            conn.execute(
                r#"
                DELETE FROM administrative_audit_events
                WHERE tenant_id = 'tenant-acme' AND delivery_id = 'delivery-1'
                "#,
                [],
            )
            .expect_err("audit row is append-only");
        });
    }

    #[test]
    fn administrative_audit_retention_purge_is_tenant_scoped_and_controlled() {
        with_test_data_dir("administrative-audit-purge", |_| {
            append_administrative_audit_event(&audit_event("delivery-old", "1000"))
                .expect("append old event");
            append_administrative_audit_event(&audit_event("delivery-current", "2000"))
                .expect("append current event");
            let mut other_tenant = audit_event("delivery-other", "500");
            other_tenant.tenant_id = "tenant-other".to_string();
            append_administrative_audit_event(&other_tenant).expect("append other tenant event");

            assert_eq!(
                purge_administrative_audit_events_before("tenant-acme", 2000)
                    .expect("purge expired events"),
                1
            );
            let tenant_export =
                export_administrative_audit_jsonl("tenant-acme").expect("export tenant");
            assert!(!tenant_export.contains("delivery-old"));
            assert!(tenant_export.contains("delivery-current"));
            assert!(export_administrative_audit_jsonl("tenant-other")
                .expect("export other tenant")
                .contains("delivery-other"));
            assert_eq!(
                append_administrative_audit_event(&audit_event("delivery-old", "3000"))
                    .expect("reject purged event resurrection"),
                AdministrativeAuditAppendResult::Duplicate
            );
            assert!(!export_administrative_audit_jsonl("tenant-acme")
                .expect("export after retry")
                .contains("delivery-old"));
        });
    }

    #[test]
    fn administrative_audit_jsonl_export_is_stable_ordered_and_tenant_scoped() {
        with_test_data_dir("administrative-audit-export", |_| {
            let mut later = audit_event("delivery-a", "2000");
            later.action = AdministrativeAuditAction::ReviewPublished;
            later.target = AdministrativeAuditTarget {
                kind: AdministrativeAuditTargetKind::Publication,
                id: "publication:remote-comment-1".to_string(),
            };
            let earlier = audit_event("delivery-b", "1000");
            let mut other_tenant = audit_event("delivery-other", "500");
            other_tenant.tenant_id = "tenant-other".to_string();

            append_administrative_audit_event(&later).expect("append later event");
            append_administrative_audit_event(&other_tenant).expect("append other tenant event");
            append_administrative_audit_event(&earlier).expect("append earlier event");

            let expected = concat!(
                "{\"schemaVersion\":\"v1\",\"deliveryId\":\"delivery-b\",\"tenantId\":\"tenant-acme\",\"occurredAt\":\"1000\",\"actor\":{\"kind\":\"user\",\"id\":\"user:reviewer-1\"},\"repository\":{\"provider\":\"github\",\"workspace\":\"acme\",\"repo\":\"payments\",\"prId\":42},\"action\":\"automated_review_triggered\",\"target\":{\"kind\":\"review_run\",\"id\":\"run:1\"},\"outcome\":\"succeeded\",\"correlationId\":\"correlation:1\"}\n",
                "{\"schemaVersion\":\"v1\",\"deliveryId\":\"delivery-a\",\"tenantId\":\"tenant-acme\",\"occurredAt\":\"2000\",\"actor\":{\"kind\":\"user\",\"id\":\"user:reviewer-1\"},\"repository\":{\"provider\":\"github\",\"workspace\":\"acme\",\"repo\":\"payments\",\"prId\":42},\"action\":\"review_published\",\"target\":{\"kind\":\"publication\",\"id\":\"publication:remote-comment-1\"},\"outcome\":\"succeeded\",\"correlationId\":\"correlation:1\"}\n"
            );
            assert_eq!(
                export_administrative_audit_jsonl("tenant-acme").expect("export JSONL"),
                expected
            );
            let mut streamed = Vec::new();
            write_administrative_audit_jsonl("tenant-acme", &mut streamed).expect("stream JSONL");
            assert_eq!(streamed, expected.as_bytes());
        });
    }

    #[test]
    fn administrative_audit_redacts_sensitive_values_in_storage_and_export() {
        with_test_data_dir("administrative-audit-redaction", |dir| {
            let mut event = audit_event("delivery-1", "1000");
            event.actor.id = "Bearer secret-token".to_string();
            event.target.id = "/Users/alice/private/repo".to_string();
            event.correlation_id = "diff --git a/secret.rs b/secret.rs".to_string();

            append_administrative_audit_event(&event).expect("append redacted event");

            let conn = Connection::open(dir.join(DB_FILE)).expect("open audit database");
            let stored: String = conn
                .query_row(
                    "SELECT event_json FROM administrative_audit_events",
                    [],
                    |row| row.get(0),
                )
                .expect("read stored event");
            let exported =
                export_administrative_audit_jsonl("tenant-acme").expect("export audit log");
            for output in [&stored, &exported] {
                assert!(output.contains(REDACTED_AUDIT_VALUE));
                for sensitive in ["secret-token", "/Users/alice", "secret.rs", "diff --git"] {
                    assert!(!output.contains(sensitive));
                }
                assert!(!output.contains("sourceCode"));
                assert!(!output.contains("promptContents"));
            }
        });
    }

    #[test]
    fn disabling_team_audit_does_not_disable_local_review_history() {
        with_test_data_dir("administrative-audit-disabled", |dir| {
            set_team_audit_collection_enabled("tenant-acme", false).expect("disable audit");
            assert!(!team_audit_collection_enabled("tenant-acme").expect("read setting"));
            assert_eq!(
                append_administrative_audit_event(&audit_event("delivery-1", "1000"))
                    .expect("skip disabled audit"),
                AdministrativeAuditAppendResult::CollectionDisabled
            );
            assert_eq!(
                export_administrative_audit_jsonl("tenant-acme").expect("empty export"),
                ""
            );
            let conn = Connection::open(dir.join(DB_FILE)).expect("open audit database");
            let receipt_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM administrative_audit_delivery_receipts",
                    [],
                    |row| row.get(0),
                )
                .expect("count delivery receipts");
            let event_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM administrative_audit_events",
                    [],
                    |row| row.get(0),
                )
                .expect("count audit events");
            assert_eq!(receipt_count, 1);
            assert_eq!(event_count, 0);

            save_review_json("acme", "payments", 42, r#"{"reviewRuns":[]}"#)
                .expect("save ordinary local review history");
            assert_eq!(
                load_review_json("acme", "payments", 42).expect("load local review history"),
                Some(r#"{"reviewRuns":[]}"#.to_string())
            );

            set_team_audit_collection_enabled("tenant-acme", true).expect("enable audit");
            assert_eq!(
                append_administrative_audit_event(&audit_event("delivery-1", "1000"))
                    .expect("deduplicate previously skipped delivery"),
                AdministrativeAuditAppendResult::Duplicate
            );
            assert_eq!(
                append_administrative_audit_event(&audit_event("delivery-2", "2000"))
                    .expect("collect new delivery"),
                AdministrativeAuditAppendResult::Appended
            );
        });
    }

    #[test]
    fn migrates_legacy_json_on_first_load() {
        with_test_data_dir("migration", |dir| {
            let legacy_dir = dir.join(LEGACY_REVIEWS_DIR);
            fs::create_dir_all(&legacy_dir).expect("legacy dir");
            fs::write(
                legacy_dir.join(legacy_review_file_name("workspace", "repo", 456)),
                r#"{"content":"old review","generatedAt":"1"}"#,
            )
            .expect("legacy review file");

            let loaded = load_review_json("workspace", "repo", 456).expect("load review");
            assert_eq!(
                loaded.as_deref(),
                Some(r#"{"content":"old review","generatedAt":"1"}"#)
            );
            assert!(dir.join(DB_FILE).exists());
        });
    }

    #[test]
    fn first_review_has_an_explicit_no_cursor_state() {
        with_test_data_dir("review-cursor-empty", |_| {
            let state = get_review_cursor(&cursor_identity()).expect("load empty cursor");

            assert_eq!(state, ReviewCursorState::NotReviewed);
        });
    }

    #[test]
    fn successful_review_atomically_advances_only_from_the_expected_cursor() {
        const FIRST_SHA: &str = "1111111111111111111111111111111111111111";
        const SECOND_SHA: &str = "2222222222222222222222222222222222222222";
        const THIRD_SHA: &str = "3333333333333333333333333333333333333333";

        with_test_data_dir("review-cursor-success", |_| {
            let first = record_review_completion(&completion(
                ReviewRunOutcome::Succeeded,
                FIRST_SHA,
                None,
                "run-1",
                "1000",
            ))
            .expect("record first completion");
            assert_eq!(
                first,
                ReviewCursorState::Reviewed(ReviewCursor {
                    identity: cursor_identity(),
                    reviewed_base_sha: None,
                    reviewed_head_sha: FIRST_SHA.to_string(),
                    run_id: "run-1".to_string(),
                    completed_at: "1000".to_string(),
                })
            );

            let latest = record_review_completion(&completion(
                ReviewRunOutcome::Succeeded,
                SECOND_SHA,
                Some(FIRST_SHA),
                "run-2",
                "2000",
            ))
            .expect("record latest completion");
            let stale = record_review_completion(&completion(
                ReviewRunOutcome::Succeeded,
                THIRD_SHA,
                Some(FIRST_SHA),
                "run-stale",
                "3000",
            ))
            .expect("reject stale compare-and-swap");
            let mut obsolete = completion(
                ReviewRunOutcome::Succeeded,
                FIRST_SHA,
                Some(SECOND_SHA),
                "run-obsolete",
                "3000",
            );
            obsolete.current_head_sha = SECOND_SHA.to_string();
            let obsolete = record_review_completion(&obsolete)
                .expect("ignore completion for an obsolete pull-request head");
            let advanced = record_review_completion(&completion(
                ReviewRunOutcome::Succeeded,
                THIRD_SHA,
                Some(SECOND_SHA),
                "run-3",
                "500",
            ))
            .expect("advance regardless of caller clock ordering");

            assert_eq!(stale, latest);
            assert_eq!(obsolete, latest);
            assert_eq!(
                advanced,
                ReviewCursorState::Reviewed(ReviewCursor {
                    identity: cursor_identity(),
                    reviewed_base_sha: None,
                    reviewed_head_sha: THIRD_SHA.to_string(),
                    run_id: "run-3".to_string(),
                    completed_at: "500".to_string(),
                })
            );
            assert_eq!(
                get_review_cursor(&cursor_identity()).expect("reload cursor"),
                advanced
            );
        });
    }

    #[test]
    fn failed_and_cancelled_reviews_do_not_advance_the_cursor() {
        const HEAD_SHA: &str = "1111111111111111111111111111111111111111";
        const OTHER_SHA: &str = "2222222222222222222222222222222222222222";

        with_test_data_dir("review-cursor-unsuccessful", |_| {
            let successful = record_review_completion(&completion(
                ReviewRunOutcome::Succeeded,
                HEAD_SHA,
                None,
                "run-successful",
                "1000",
            ))
            .expect("record successful completion");

            for outcome in [ReviewRunOutcome::Failed, ReviewRunOutcome::Cancelled] {
                let state = record_review_completion(&completion(
                    outcome,
                    OTHER_SHA,
                    Some(HEAD_SHA),
                    "run-unsuccessful",
                    "2000",
                ))
                .expect("record unsuccessful completion");
                assert_eq!(state, successful);
            }
        });
    }

    #[test]
    fn records_every_feedback_action_without_mutating_the_original_finding() {
        const REVIEW_JSON: &str = r#"{"reviewRuns":[{"id":"run-1","findings":[{"fingerprint":"finding-abc","status":"new"}]}]}"#;
        with_test_data_dir("finding-feedback-actions", |_| {
            save_review_json("acme", "payments", 42, REVIEW_JSON).expect("save original finding");

            let mut latest = None;
            for (index, action) in ReviewFindingFeedbackAction::ALL.into_iter().enumerate() {
                latest = Some(
                    record_finding_feedback(&feedback_event(
                        &format!("event-{index}"),
                        action,
                        &format!("{}", 1000 + index),
                    ))
                    .expect("record feedback"),
                );
            }

            let latest = latest.expect("feedback state");
            assert_eq!(latest.events.len(), ReviewFindingFeedbackAction::ALL.len());
            assert_eq!(
                latest.disposition,
                crate::review_feedback::ReviewFindingDisposition::Open
            );
            assert_eq!(
                load_review_json("acme", "payments", 42).expect("reload original finding"),
                Some(REVIEW_JSON.to_string())
            );
        });
    }

    #[test]
    fn duplicate_feedback_delivery_is_idempotent_and_conflicts_fail_closed() {
        with_test_data_dir("finding-feedback-idempotency", |dir| {
            let mut event =
                feedback_event("delivery-1", ReviewFindingFeedbackAction::Accepted, "1000");
            event.reason = None;

            let first = record_finding_feedback(&event).expect("first delivery");
            let duplicate = record_finding_feedback(&event).expect("duplicate delivery");
            assert_eq!(duplicate, first);
            assert_eq!(duplicate.events.len(), 1);

            let conn =
                Connection::open(dir.join(DB_FILE)).expect("open feedback database directly");
            conn.execute(
                r#"
                UPDATE review_finding_feedback_events
                SET reason = ''
                WHERE tenant_id = 'tenant-acme' AND event_id = 'delivery-1'
                "#,
                [],
            )
            .expect("simulate a non-canonical imported reason");
            drop(conn);
            let normalized_duplicate =
                record_finding_feedback(&event).expect("duplicate after reason normalization");
            assert_eq!(normalized_duplicate, first);

            let mut conflicting = event;
            conflicting.action = ReviewFindingFeedbackAction::Fixed;
            assert_eq!(
                record_finding_feedback(&conflicting).expect_err("conflicting delivery"),
                "`eventId` is already associated with different reviewer feedback"
            );
            assert_eq!(
                get_finding_feedback_state(&conflicting.target())
                    .expect("state after conflict")
                    .events
                    .len(),
                1
            );
        });
    }

    #[test]
    fn feedback_state_is_deterministic_for_out_of_order_and_equal_timestamps() {
        with_test_data_dir("finding-feedback-ordering", |_| {
            let reopened = feedback_event(
                "event-middle",
                ReviewFindingFeedbackAction::Reopened,
                "3000",
            );
            let target = reopened.target();
            record_finding_feedback(&reopened).expect("newest first");
            record_finding_feedback(&feedback_event(
                "event-old",
                ReviewFindingFeedbackAction::Fixed,
                "2000",
            ))
            .expect("older second");
            record_finding_feedback(&feedback_event(
                "event-z",
                ReviewFindingFeedbackAction::Fixed,
                "4000",
            ))
            .expect("equal timestamp lexically later");
            let state = record_finding_feedback(&feedback_event(
                "event-a",
                ReviewFindingFeedbackAction::Accepted,
                "4000",
            ))
            .expect("equal timestamp lexically earlier");

            assert_eq!(
                state.disposition,
                crate::review_feedback::ReviewFindingDisposition::Fixed
            );
            assert_eq!(
                state
                    .latest_event
                    .as_ref()
                    .map(|event| event.event_id.as_str()),
                Some("event-z")
            );
            assert_eq!(
                get_finding_feedback_state(&target).expect("reload state"),
                state
            );
        });
    }

    #[test]
    fn feedback_storage_is_tenant_scoped() {
        with_test_data_dir("finding-feedback-tenant-isolation", |_| {
            let tenant_acme = feedback_event(
                "shared-delivery",
                ReviewFindingFeedbackAction::Accepted,
                "1000",
            );
            let mut tenant_other = tenant_acme.clone();
            tenant_other.identity.tenant_id = "tenant-other".to_string();
            tenant_other.action = ReviewFindingFeedbackAction::Dismissed;

            record_finding_feedback(&tenant_acme).expect("acme feedback");
            record_finding_feedback(&tenant_other).expect("other tenant feedback");

            assert_eq!(
                get_finding_feedback_state(&tenant_acme.target())
                    .expect("acme state")
                    .disposition,
                crate::review_feedback::ReviewFindingDisposition::Accepted
            );
            assert_eq!(
                get_finding_feedback_state(&tenant_other.target())
                    .expect("other state")
                    .disposition,
                crate::review_feedback::ReviewFindingDisposition::Dismissed
            );
        });
    }

    #[test]
    fn v1_database_migrates_without_losing_review_jobs_or_findings() {
        with_test_data_dir("review-cursor-v1-migration", |dir| {
            fs::create_dir_all(dir).expect("create v1 data directory");
            let db = dir.join(DB_FILE);
            let conn = Connection::open(&db).expect("open v1 database");
            conn.execute_batch(
                r#"
                CREATE TABLE schema_migrations (
                  version INTEGER PRIMARY KEY,
                  applied_at TEXT NOT NULL DEFAULT (strftime('%s','now') || '000')
                );
                INSERT INTO schema_migrations(version) VALUES (1);

                CREATE TABLE ai_review_stores (
                  review_key TEXT PRIMARY KEY,
                  workspace TEXT NOT NULL,
                  repo TEXT NOT NULL,
                  pr_id INTEGER NOT NULL,
                  store_json TEXT NOT NULL,
                  migrated_from_json INTEGER NOT NULL DEFAULT 0,
                  created_at TEXT NOT NULL,
                  updated_at TEXT NOT NULL
                );
                INSERT INTO ai_review_stores (
                  review_key, workspace, repo, pr_id, store_json,
                  migrated_from_json, created_at, updated_at
                ) VALUES (
                  'acme_payments_42', 'acme', 'payments', 42,
                  '{"reviewRuns":[{"findings":[{"title":"preserved"}]}]}',
                  0, '1000', '2000'
                );

                CREATE TABLE ai_review_jobs (
                  id TEXT PRIMARY KEY,
                  workspace TEXT NOT NULL,
                  repo TEXT NOT NULL,
                  pr_id INTEGER NOT NULL,
                  pr_title TEXT NOT NULL,
                  source_branch TEXT NOT NULL,
                  destination_branch TEXT NOT NULL,
                  status TEXT NOT NULL,
                  trigger TEXT NOT NULL,
                  thread_id TEXT,
                  error TEXT,
                  created_at TEXT NOT NULL,
                  started_at TEXT,
                  finished_at TEXT
                );
                INSERT INTO ai_review_jobs (
                  id, workspace, repo, pr_id, pr_title, source_branch,
                  destination_branch, status, trigger, thread_id, error,
                  created_at, started_at, finished_at
                ) VALUES (
                  'job-existing', 'acme', 'payments', 42, 'Existing review',
                  'feature', 'main', 'succeeded', 'manual', 'thread-existing',
                  NULL, '1000', '1000', '2000'
                );
                "#,
            )
            .expect("create v1 schema");
            drop(conn);

            assert_eq!(
                get_review_cursor(&cursor_identity()).expect("migrate and load cursor"),
                ReviewCursorState::NotReviewed
            );
            assert_eq!(
                load_review_json("acme", "payments", 42).expect("load preserved findings"),
                Some(r#"{"reviewRuns":[{"findings":[{"title":"preserved"}]}]}"#.to_string())
            );
            assert_eq!(
                get_review_job("job-existing")
                    .expect("load preserved job")
                    .expect("existing job")
                    .status,
                ReviewJobStatus::Succeeded
            );

            let conn = Connection::open(db).expect("reopen migrated database");
            let versions: Vec<i64> = conn
                .prepare("SELECT version FROM schema_migrations ORDER BY version")
                .expect("prepare migration query")
                .query_map([], |row| row.get(0))
                .expect("query migrations")
                .collect::<rusqlite::Result<_>>()
                .expect("read migration versions");
            assert_eq!(versions, vec![1, 2, 3, 4, 5, 6, 7]);
        });
    }

    #[test]
    fn v6_publication_database_adds_base_sha_without_losing_rows() {
        with_test_data_dir("finding-publication-v6-migration", |dir| {
            let request = finding_publication_request();
            let legacy_marker = crate::finding_publication::legacy_finding_marker(&request);
            let current_marker = crate::finding_publication::finding_marker(&request);
            fs::create_dir_all(dir).expect("create v6 data directory");
            let db = dir.join(DB_FILE);
            let conn = Connection::open(&db).expect("open v6 database");
            conn.execute_batch(
                r#"
                CREATE TABLE schema_migrations (
                  version INTEGER PRIMARY KEY,
                  applied_at TEXT NOT NULL DEFAULT (strftime('%s','now') || '000')
                );
                INSERT INTO schema_migrations(version)
                VALUES (1), (2), (3), (4), (5), (6);

                CREATE TABLE shared_finding_publications (
                  marker TEXT PRIMARY KEY,
                  tenant_id TEXT NOT NULL,
                  provider TEXT NOT NULL,
                  workspace TEXT NOT NULL,
                  repo TEXT NOT NULL,
                  pr_id INTEGER NOT NULL,
                  head_sha TEXT NOT NULL,
                  finding_fingerprint TEXT NOT NULL,
                  status TEXT NOT NULL,
                  lease_token TEXT,
                  lease_expires_at_ms INTEGER,
                  comment_id TEXT,
                  created_at_ms INTEGER NOT NULL,
                  updated_at_ms INTEGER NOT NULL
                );
                CREATE INDEX idx_shared_finding_publications_target
                  ON shared_finding_publications(
                    tenant_id, provider, workspace, repo, pr_id, head_sha
                  );
                "#,
            )
            .expect("create v6 publication schema");
            conn.execute(
                r#"
                INSERT INTO shared_finding_publications (
                  marker, tenant_id, provider, workspace, repo, pr_id, head_sha,
                  finding_fingerprint, status, lease_token, lease_expires_at_ms,
                  comment_id, created_at_ms, updated_at_ms
                ) VALUES (
                  ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'published',
                  NULL, NULL, 'comment-1', 1000, 1000
                )
                "#,
                params![
                    legacy_marker,
                    request.tenant_id,
                    request.provider.as_str(),
                    request.workspace,
                    request.repository,
                    request.pull_request_id,
                    request.head_sha,
                    request.finding_fingerprint,
                ],
            )
            .expect("insert v6 publication");
            drop(conn);

            let migrated = open().expect("migrate v6 publication schema");
            let (base_sha, comment_id): (String, String) = migrated
                .query_row(
                    r#"
                    SELECT base_sha, comment_id
                    FROM shared_finding_publications
                    WHERE marker = ?1
                    "#,
                    params![legacy_marker],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("load migrated publication");
            assert_eq!(base_sha, "");
            assert_eq!(comment_id, "comment-1");
            let index_sql: String = migrated
                .query_row(
                    r#"
                    SELECT sql FROM sqlite_master
                    WHERE type = 'index'
                      AND name = 'idx_shared_finding_publications_target'
                    "#,
                    [],
                    |row| row.get(0),
                )
                .expect("load migrated publication index");
            assert!(index_sql.contains("base_sha"));
            let version: i64 = migrated
                .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                    row.get(0)
                })
                .expect("load latest migration");
            assert_eq!(version, 7);
            drop(migrated);

            assert_eq!(
                reserve_finding_publication(&request, &current_marker, "lease-after-migration")
                    .expect("reserve migrated publication"),
                FindingPublicationReservation::Published {
                    identity: ProviderCommentIdentity {
                        comment_id: "comment-1".to_string(),
                    },
                    marker: legacy_marker,
                }
            );
        });
    }

    #[test]
    fn feedback_migration_repairs_a_missing_v3_table_atomically() {
        with_test_data_dir("finding-feedback-v3-repair", |dir| {
            fs::create_dir_all(dir).expect("create data directory");
            let db = dir.join(DB_FILE);
            let conn = Connection::open(&db).expect("open incomplete v3 database");
            conn.execute_batch(
                r#"
                CREATE TABLE schema_migrations (
                  version INTEGER PRIMARY KEY,
                  applied_at TEXT NOT NULL DEFAULT (strftime('%s','now') || '000')
                );
                INSERT INTO schema_migrations(version) VALUES (1), (2), (3);
                "#,
            )
            .expect("create incomplete v3 schema");
            drop(conn);

            let target =
                feedback_event("event-1", ReviewFindingFeedbackAction::Accepted, "1000").target();
            assert!(get_finding_feedback_state(&target)
                .expect("repair and load feedback state")
                .events
                .is_empty());

            let conn = Connection::open(db).expect("reopen repaired database");
            let table_exists: bool = conn
                .query_row(
                    r#"
                    SELECT EXISTS(
                      SELECT 1 FROM sqlite_master
                      WHERE type = 'table' AND name = 'review_finding_feedback_events'
                    )
                    "#,
                    [],
                    |row| row.get(0),
                )
                .expect("query repaired table");
            assert!(table_exists);
        });
    }

    #[test]
    fn cleanup_removes_stale_db_rows_and_legacy_files() {
        with_test_data_dir("cleanup", |dir| {
            save_review_json("workspace", "repo", 1, r#"{"one":true}"#).expect("save one");
            save_review_json("workspace", "repo", 2, r#"{"two":true}"#).expect("save two");
            let legacy_dir = dir.join(LEGACY_REVIEWS_DIR);
            fs::create_dir_all(&legacy_dir).expect("legacy dir");
            let stale_legacy = legacy_dir.join(legacy_review_file_name("workspace", "repo", 2));
            fs::write(&stale_legacy, "{}").expect("legacy review file");

            cleanup_stale_reviews(&["workspace_repo_1".to_string()]).expect("cleanup");

            assert!(load_review_json("workspace", "repo", 1)
                .expect("load kept")
                .is_some());
            assert!(load_review_json("workspace", "repo", 2)
                .expect("load removed")
                .is_none());
            assert!(!stale_legacy.exists());
        });
    }

    #[test]
    fn tracks_review_job_lifecycle() {
        with_test_data_dir("jobs", |_| {
            let job = create_review_job(
                "workspace",
                "repo",
                9,
                "Add menu review",
                "feature/menu-review",
                "main",
                "menuBar",
            )
            .expect("create job");
            assert_eq!(job.status, ReviewJobStatus::Queued);

            let running =
                update_review_job_status(&job.id, ReviewJobStatus::Running, Some("thread-1"), None)
                    .expect("mark running");
            assert_eq!(running.status, ReviewJobStatus::Running);
            assert_eq!(running.thread_id.as_deref(), Some("thread-1"));
            assert!(running.started_at.is_some());

            let finished = update_review_job_status(
                &job.id,
                ReviewJobStatus::Succeeded,
                Some("thread-1"),
                None,
            )
            .expect("mark succeeded");
            assert_eq!(finished.status, ReviewJobStatus::Succeeded);
            assert!(finished.finished_at.is_some());

            let jobs = list_recent_review_jobs(10).expect("list jobs");
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].id, job.id);
        });
    }

    #[test]
    fn shared_review_jobs_use_full_then_incremental_scope_and_advance_on_success_only() {
        const FIRST_SHA: &str = "1111111111111111111111111111111111111111";
        const SECOND_SHA: &str = "2222222222222222222222222222222222222222";
        const THIRD_SHA: &str = "3333333333333333333333333333333333333333";

        with_test_data_dir("shared-review-scope-cursor", |_| {
            let first = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    FIRST_SHA,
                    "delivery-1",
                ))
                .expect("enqueue first review"),
            );
            assert_eq!(
                first.request.scope,
                ReviewJobScope::FullBranch {
                    base_sha: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
                    head_sha: FIRST_SHA.to_string(),
                }
            );
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim first review")
                .expect("first review job");
            assert_eq!(running.request.id, first.request.id);
            let completed = finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-1".to_string(),
                },
            )
            .expect("finish first review");
            assert_eq!(completed.status, SharedReviewJobStatus::Completed);
            assert_eq!(
                finish_shared_review_job(
                    &running.request.id,
                    running.attempt_count,
                    &ReviewJobExecution::Completed {
                        run_id: "run-1".to_string(),
                    },
                )
                .expect("repeat matching completion"),
                completed
            );
            assert!(finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Failed {
                    run_id: Some("run-1".to_string()),
                    error_code: "conflicting_outcome".to_string(),
                },
            )
            .is_err());
            assert_eq!(
                get_review_cursor(&cursor_identity()).expect("cursor after success"),
                ReviewCursorState::Reviewed(ReviewCursor {
                    identity: cursor_identity(),
                    reviewed_base_sha: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),),
                    reviewed_head_sha: FIRST_SHA.to_string(),
                    run_id: "run-1".to_string(),
                    completed_at: completed.finished_at.clone().expect("completion time"),
                })
            );

            let second = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    SECOND_SHA,
                    "delivery-2",
                ))
                .expect("enqueue incremental review"),
            );
            assert_eq!(
                second.request.scope,
                ReviewJobScope::Incremental {
                    previous_head_sha: FIRST_SHA.to_string(),
                    current_head_sha: SECOND_SHA.to_string(),
                }
            );
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim incremental review")
                .expect("incremental review job");
            let failed = finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Failed {
                    run_id: Some("run-2".to_string()),
                    error_code: "provider_unavailable".to_string(),
                },
            )
            .expect("fail incremental review");
            assert_eq!(failed.status, SharedReviewJobStatus::Failed);
            assert_eq!(
                get_review_cursor(&cursor_identity()).expect("cursor after failure"),
                ReviewCursorState::Reviewed(ReviewCursor {
                    identity: cursor_identity(),
                    reviewed_base_sha: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),),
                    reviewed_head_sha: FIRST_SHA.to_string(),
                    run_id: "run-1".to_string(),
                    completed_at: completed.finished_at.expect("completion time"),
                })
            );

            let third = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    THIRD_SHA,
                    "delivery-3",
                ))
                .expect("enqueue after failed review"),
            );
            assert_eq!(third.request.scope.previous_head_sha(), Some(FIRST_SHA));
        });
    }

    #[test]
    fn shared_review_jobs_deduplicate_delivery_and_head_per_tenant() {
        const HEAD_SHA: &str = "1111111111111111111111111111111111111111";
        const OTHER_SHA: &str = "2222222222222222222222222222222222222222";

        with_test_data_dir("shared-review-dedup", |_| {
            let event = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                HEAD_SHA,
                "delivery-1",
            );
            let queued =
                queued_shared_job(enqueue_shared_review_job(&event).expect("enqueue review"));
            assert!(matches!(
                enqueue_shared_review_job(&event).expect("deduplicate delivery"),
                ReviewJobEnqueueOutcome::DuplicateDelivery(_)
            ));

            let mut same_head = event.clone();
            same_head.delivery_id = "delivery-2".to_string();
            assert!(matches!(
                enqueue_shared_review_job(&same_head).expect("deduplicate head"),
                ReviewJobEnqueueOutcome::DuplicateHead(Some(_))
            ));

            let mut conflicting_delivery = event.clone();
            conflicting_delivery.head.sha = OTHER_SHA.to_string();
            assert!(enqueue_shared_review_job(&conflicting_delivery).is_err());

            let mut other_tenant = event;
            other_tenant.tenant_id = "tenant-other".to_string();
            assert!(matches!(
                enqueue_shared_review_job(&other_tenant).expect("other tenant review"),
                ReviewJobEnqueueOutcome::Queued(_)
            ));
            assert_eq!(
                get_shared_review_job(&queued.request.id)
                    .expect("load original job")
                    .expect("original job")
                    .request
                    .tenant_id,
                "tenant-acme"
            );
        });
    }

    #[test]
    fn a_newer_base_revision_with_the_same_head_creates_a_full_review() {
        const HEAD_SHA: &str = "1111111111111111111111111111111111111111";
        const NEW_BASE_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        with_test_data_dir("shared-review-base-retarget", |_| {
            let original = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                HEAD_SHA,
                "delivery-original-base",
            );
            enqueue_shared_review_job(&original).expect("enqueue original base");
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim original base")
                .expect("original base job");
            finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-original-base".to_string(),
                },
            )
            .expect("complete original base");

            let mut retargeted = original;
            retargeted.delivery_id = "delivery-new-base".to_string();
            retargeted.base.ref_name = "release".to_string();
            retargeted.base.sha = NEW_BASE_SHA.to_string();
            retargeted.provider_updated_at_ms =
                retargeted.provider_updated_at_ms.map(|value| value + 1);
            let queued = queued_shared_job(
                enqueue_shared_review_job(&retargeted).expect("enqueue retargeted base"),
            );
            assert_eq!(
                queued.request.scope,
                ReviewJobScope::FullBranch {
                    base_sha: NEW_BASE_SHA.to_string(),
                    head_sha: HEAD_SHA.to_string(),
                }
            );
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim retargeted base")
                .expect("retargeted base job");
            let completed = finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-new-base".to_string(),
                },
            )
            .expect("complete retargeted base");
            assert_eq!(completed.status, SharedReviewJobStatus::Completed);
        });
    }

    #[test]
    fn combined_base_and_head_changes_create_a_full_review() {
        const FIRST_HEAD_SHA: &str = "1111111111111111111111111111111111111111";
        const SECOND_HEAD_SHA: &str = "2222222222222222222222222222222222222222";
        const NEW_BASE_SHA: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        with_test_data_dir("shared-review-base-and-head-change", |_| {
            enqueue_shared_review_job(&shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                FIRST_HEAD_SHA,
                "delivery-original-revision",
            ))
            .expect("enqueue original revision");
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim original revision")
                .expect("original revision job");
            finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-original-revision".to_string(),
                },
            )
            .expect("complete original revision");

            let mut changed = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                SECOND_HEAD_SHA,
                "delivery-changed-revision",
            );
            changed.base.ref_name = "release".to_string();
            changed.base.sha = NEW_BASE_SHA.to_string();
            let queued = queued_shared_job(
                enqueue_shared_review_job(&changed).expect("enqueue changed base and head"),
            );

            assert_eq!(
                queued.request.scope,
                ReviewJobScope::FullBranch {
                    base_sha: NEW_BASE_SHA.to_string(),
                    head_sha: SECOND_HEAD_SHA.to_string(),
                }
            );
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim changed base and head")
                .expect("changed base and head job");
            let completed = finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-changed-revision".to_string(),
                },
            )
            .expect("complete changed base and head");
            assert_eq!(completed.status, SharedReviewJobStatus::Completed);
        });
    }

    #[test]
    fn stale_head_events_are_ignored_without_cancelling_current_queued_work() {
        const OLD_SHA: &str = "1111111111111111111111111111111111111111";
        const NEW_SHA: &str = "3333333333333333333333333333333333333333";
        with_test_data_dir("shared-review-stale-enqueue", |_| {
            let current = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    NEW_SHA,
                    "delivery-current",
                ))
                .expect("enqueue current head"),
            );
            let stale = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                OLD_SHA,
                "delivery-stale",
            );

            assert!(matches!(
                enqueue_shared_review_job(&stale).expect("ignore stale head"),
                ReviewJobEnqueueOutcome::Ignored {
                    reason: ReviewJobIgnoredReason::Stale,
                    cancelled_queued_jobs: 0,
                }
            ));
            assert_eq!(
                get_shared_review_job(&current.request.id)
                    .expect("load current job")
                    .expect("current job")
                    .status,
                SharedReviewJobStatus::Queued
            );
        });
    }

    #[test]
    fn equal_timestamp_revision_changes_are_enqueued_instead_of_dropped() {
        const FIRST_SHA: &str = "1111111111111111111111111111111111111111";
        const SECOND_SHA: &str = "2222222222222222222222222222222222222222";
        with_test_data_dir("shared-review-equal-timestamp", |_| {
            let first_event = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                FIRST_SHA,
                "delivery-first",
            );
            let provider_updated_at_ms = first_event.provider_updated_at_ms;
            let first = queued_shared_job(
                enqueue_shared_review_job(&first_event).expect("enqueue first revision"),
            );
            let mut same_timestamp = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                SECOND_SHA,
                "delivery-second",
            );
            same_timestamp.provider_updated_at_ms = provider_updated_at_ms;

            let second = queued_shared_job(
                enqueue_shared_review_job(&same_timestamp)
                    .expect("enqueue ambiguous same-timestamp revision"),
            );
            assert_ne!(first.request.id, second.request.id);
            assert_eq!(
                get_shared_review_job(&first.request.id)
                    .expect("load first ambiguous job")
                    .expect("first ambiguous job")
                    .status,
                SharedReviewJobStatus::Queued
            );
            let running_first = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim first ambiguous job")
                .expect("first ambiguous job");
            let cancelled_first = finish_shared_review_job(
                &running_first.request.id,
                running_first.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-ambiguous-first".to_string(),
                },
            )
            .expect("finish first ambiguous job");
            assert_eq!(cancelled_first.status, SharedReviewJobStatus::Cancelled);
            let running_second = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim second ambiguous job")
                .expect("second ambiguous job");
            let cancelled_second = finish_shared_review_job(
                &running_second.request.id,
                running_second.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-ambiguous-second".to_string(),
                },
            )
            .expect("finish second ambiguous job");
            assert_eq!(cancelled_second.status, SharedReviewJobStatus::Cancelled);
            assert_eq!(
                get_review_cursor(&cursor_identity()).expect("ambiguous cursor"),
                ReviewCursorState::NotReviewed
            );

            same_timestamp.delivery_id = "delivery-confirmed".to_string();
            same_timestamp.provider_updated_at_ms =
                same_timestamp.provider_updated_at_ms.map(|value| value + 1);
            enqueue_shared_review_job(&same_timestamp).expect("enqueue confirmed revision");
            let confirmed = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim confirmed revision")
                .expect("confirmed revision");
            let completed = finish_shared_review_job(
                &confirmed.request.id,
                confirmed.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-confirmed".to_string(),
                },
            )
            .expect("finish confirmed revision");
            assert_eq!(completed.status, SharedReviewJobStatus::Completed);
            assert!(matches!(
                get_review_cursor(&cursor_identity()).expect("confirmed cursor"),
                ReviewCursorState::Reviewed(ReviewCursor {
                    reviewed_head_sha,
                    ..
                }) if reviewed_head_sha == SECOND_SHA
            ));
        });
    }

    #[test]
    fn newer_head_supersedes_only_queued_work_for_the_same_pull_request() {
        const OLD_SHA: &str = "1111111111111111111111111111111111111111";
        const NEW_SHA: &str = "2222222222222222222222222222222222222222";
        const OTHER_SHA: &str = "3333333333333333333333333333333333333333";

        with_test_data_dir("shared-review-supersede", |_| {
            let stale = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    OLD_SHA,
                    "delivery-stale",
                ))
                .expect("enqueue stale job"),
            );
            let unrelated = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    43,
                    OTHER_SHA,
                    "delivery-unrelated",
                ))
                .expect("enqueue unrelated job"),
            );
            let current = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    NEW_SHA,
                    "delivery-current",
                ))
                .expect("enqueue current job"),
            );

            let stale = get_shared_review_job(&stale.request.id)
                .expect("load stale job")
                .expect("stale job");
            assert_eq!(stale.status, SharedReviewJobStatus::Cancelled);
            assert_eq!(stale.error_code.as_deref(), Some("superseded"));
            assert_eq!(
                get_shared_review_job(&unrelated.request.id)
                    .expect("load unrelated job")
                    .expect("unrelated job")
                    .status,
                SharedReviewJobStatus::Queued
            );
            assert_eq!(current.status, SharedReviewJobStatus::Queued);
        });
    }

    #[test]
    fn shared_review_claims_respect_repository_and_pull_request_limits() {
        const FIRST_SHA: &str = "1111111111111111111111111111111111111111";
        const SECOND_SHA: &str = "2222222222222222222222222222222222222222";
        const OTHER_SHA: &str = "3333333333333333333333333333333333333333";

        with_test_data_dir("shared-review-concurrency", |_| {
            enqueue_shared_review_job(&shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                FIRST_SHA,
                "delivery-running",
            ))
            .expect("enqueue running job");
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits {
                per_repository: 1,
                per_pull_request: 1,
            })
            .expect("claim first job")
            .expect("running job");
            enqueue_shared_review_job(&shared_event(
                "tenant-acme",
                "acme",
                "payments",
                43,
                OTHER_SHA,
                "delivery-same-repo",
            ))
            .expect("enqueue same repository");
            enqueue_shared_review_job(&shared_event(
                "tenant-acme",
                "acme",
                "ledger",
                42,
                OTHER_SHA,
                "delivery-other-repo",
            ))
            .expect("enqueue other repository");

            let other_repository = claim_next_shared_review_job(ReviewConcurrencyLimits {
                per_repository: 1,
                per_pull_request: 1,
            })
            .expect("claim other repository")
            .expect("other repository job");
            assert_eq!(other_repository.request.repository, "ledger");
            assert!(claim_next_shared_review_job(ReviewConcurrencyLimits {
                per_repository: 1,
                per_pull_request: 1,
            })
            .expect("repository limit")
            .is_none());

            finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Failed {
                    run_id: None,
                    error_code: "synthetic_failure".to_string(),
                },
            )
            .expect("release repository slot");
            let same_repository = claim_next_shared_review_job(ReviewConcurrencyLimits {
                per_repository: 1,
                per_pull_request: 1,
            })
            .expect("claim same repository")
            .expect("same repository job");
            assert_eq!(same_repository.request.pull_request_id, 43);

            let mut newer = shared_event(
                "tenant-acme",
                "acme",
                "ledger",
                42,
                SECOND_SHA,
                "delivery-same-pr-new-head",
            );
            newer.head.ref_name = "feature/new-head".to_string();
            enqueue_shared_review_job(&newer).expect("enqueue same PR new head");
            assert!(claim_next_shared_review_job(ReviewConcurrencyLimits {
                per_repository: 2,
                per_pull_request: 1,
            })
            .expect("pull-request limit")
            .is_none());
        });
    }

    #[test]
    fn draft_or_closed_state_cancels_queued_jobs_and_blocks_cursor_advancement() {
        const HEAD_SHA: &str = "1111111111111111111111111111111111111111";
        with_test_data_dir("shared-review-suppression", |_| {
            enqueue_shared_review_job(&shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                HEAD_SHA,
                "delivery-running",
            ))
            .expect("enqueue running job");
            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim running job")
                .expect("running job");

            let mut closed = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                HEAD_SHA,
                "delivery-closed",
            );
            closed.kind = PullRequestReviewEventKind::Closed;
            closed.closed_outcome = Some(PullRequestClosedOutcome::ClosedWithoutMerge);
            assert_eq!(
                suppress_shared_review_jobs(&closed, ReviewJobIgnoredReason::Closed)
                    .expect("suppress closed PR"),
                0
            );
            let stale = finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-stale".to_string(),
                },
            )
            .expect("finish now-closed review");
            assert_eq!(stale.status, SharedReviewJobStatus::Cancelled);
            assert_eq!(
                stale.error_code.as_deref(),
                Some("stale_pull_request_state")
            );
            assert_eq!(
                get_review_cursor(&cursor_identity()).expect("closed PR cursor"),
                ReviewCursorState::NotReviewed
            );
            closed.kind = PullRequestReviewEventKind::Reopened;
            closed.closed_outcome = None;
            closed.delivery_id = "delivery-reopened".to_string();
            let reopened = queued_shared_job(
                enqueue_shared_review_job(&closed).expect("enqueue reopened PR at the same head"),
            );
            assert_eq!(reopened.request.head.sha, HEAD_SHA);

            let mut draft = shared_event(
                "tenant-acme",
                "acme",
                "other",
                7,
                HEAD_SHA,
                "delivery-draft",
            );
            let queued =
                queued_shared_job(enqueue_shared_review_job(&draft).expect("enqueue before draft"));
            draft.draft = true;
            assert_eq!(
                suppress_shared_review_jobs(&draft, ReviewJobIgnoredReason::Draft)
                    .expect("suppress draft PR"),
                1
            );
            assert_eq!(
                get_shared_review_job(&queued.request.id)
                    .expect("load cancelled draft job")
                    .expect("cancelled draft job")
                    .status,
                SharedReviewJobStatus::Cancelled
            );
            draft.draft = false;
            draft.kind = PullRequestReviewEventKind::ReadyForReview;
            draft.delivery_id = "delivery-ready".to_string();
            let ready = queued_shared_job(
                enqueue_shared_review_job(&draft)
                    .expect("enqueue ready-for-review PR at the same head"),
            );
            assert_ne!(ready.request.id, queued.request.id);
        });
    }

    #[test]
    fn expired_review_job_leases_are_reclaimed_and_fence_stale_workers() {
        const HEAD_SHA: &str = "1111111111111111111111111111111111111111";
        with_test_data_dir("shared-review-lease-recovery", |dir| {
            enqueue_shared_review_job(&shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                HEAD_SHA,
                "delivery-lease",
            ))
            .expect("enqueue leased job");
            let first_attempt = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim first attempt")
                .expect("first attempt");
            assert_eq!(first_attempt.attempt_count, 1);

            let conn =
                Connection::open(dir.join(DB_FILE)).expect("open review job database directly");
            conn.execute(
                "UPDATE shared_review_jobs SET lease_expires_at_ms = 0 WHERE id = ?1",
                params![first_attempt.request.id],
            )
            .expect("expire first attempt lease");
            drop(conn);
            assert!(finish_shared_review_job(
                &first_attempt.request.id,
                first_attempt.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-expired-worker".to_string(),
                },
            )
            .is_err());

            let second_attempt = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("reclaim expired job")
                .expect("second attempt");
            assert_eq!(second_attempt.request.id, first_attempt.request.id);
            assert_eq!(second_attempt.attempt_count, 2);
            assert!(renew_shared_review_job_lease(
                &second_attempt.request.id,
                first_attempt.attempt_count
            )
            .is_err());
            let renewed = renew_shared_review_job_lease(
                &second_attempt.request.id,
                second_attempt.attempt_count,
            )
            .expect("renew current attempt");
            assert!(renewed.lease_expires_at.is_some());
            assert!(finish_shared_review_job(
                &first_attempt.request.id,
                first_attempt.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-stale-worker".to_string(),
                },
            )
            .is_err());

            let completed = finish_shared_review_job(
                &second_attempt.request.id,
                second_attempt.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-current-worker".to_string(),
                },
            )
            .expect("complete current attempt");
            assert_eq!(completed.status, SharedReviewJobStatus::Completed);
        });
    }

    #[test]
    fn expired_review_job_leases_stop_requeueing_after_the_attempt_limit() {
        const HEAD_SHA: &str = "1111111111111111111111111111111111111111";
        with_test_data_dir("shared-review-lease-exhaustion", |dir| {
            let queued = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    HEAD_SHA,
                    "delivery-exhaustion",
                ))
                .expect("enqueue recoverable job"),
            );

            for expected_attempt in 1..=MAX_SHARED_REVIEW_JOB_ATTEMPTS {
                let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                    .expect("claim recoverable job")
                    .expect("running attempt");
                assert_eq!(running.attempt_count, expected_attempt);
                let conn =
                    Connection::open(dir.join(DB_FILE)).expect("open review database directly");
                conn.execute(
                    "UPDATE shared_review_jobs SET lease_expires_at_ms = 0 WHERE id = ?1",
                    params![running.request.id],
                )
                .expect("expire running attempt");
            }

            assert!(
                claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                    .expect("process exhausted lease")
                    .is_none()
            );
            let exhausted = get_shared_review_job(&queued.request.id)
                .expect("load exhausted job")
                .expect("exhausted job");
            assert_eq!(exhausted.status, SharedReviewJobStatus::Failed);
            assert_eq!(
                exhausted.error_code.as_deref(),
                Some("worker_lease_exhausted")
            );
            assert_eq!(exhausted.attempt_count, MAX_SHARED_REVIEW_JOB_ATTEMPTS);
        });
    }

    #[test]
    fn stale_suppression_events_do_not_cancel_or_invalidate_newer_head_work() {
        const OLD_SHA: &str = "1111111111111111111111111111111111111111";
        const NEW_SHA: &str = "2222222222222222222222222222222222222222";
        with_test_data_dir("shared-review-stale-suppression", |_| {
            let queued = queued_shared_job(
                enqueue_shared_review_job(&shared_event(
                    "tenant-acme",
                    "acme",
                    "payments",
                    42,
                    NEW_SHA,
                    "delivery-new-head",
                ))
                .expect("enqueue current head"),
            );
            let mut stale_closed = shared_event(
                "tenant-acme",
                "acme",
                "payments",
                42,
                OLD_SHA,
                "delivery-stale-closed",
            );
            stale_closed.kind = PullRequestReviewEventKind::Closed;
            stale_closed.closed_outcome = Some(PullRequestClosedOutcome::ClosedWithoutMerge);

            assert_eq!(
                suppress_shared_review_jobs(&stale_closed, ReviewJobIgnoredReason::Closed)
                    .expect("ignore stale close"),
                0
            );
            assert_eq!(
                get_shared_review_job(&queued.request.id)
                    .expect("load current job")
                    .expect("current job")
                    .status,
                SharedReviewJobStatus::Queued
            );

            let running = claim_next_shared_review_job(ReviewConcurrencyLimits::default())
                .expect("claim current job")
                .expect("current job");
            let completed = finish_shared_review_job(
                &running.request.id,
                running.attempt_count,
                &ReviewJobExecution::Completed {
                    run_id: "run-current-head".to_string(),
                },
            )
            .expect("finish current head");
            assert_eq!(completed.status, SharedReviewJobStatus::Completed);
            assert!(matches!(
                get_review_cursor(&cursor_identity()).expect("current cursor"),
                ReviewCursorState::Reviewed(ReviewCursor {
                    reviewed_head_sha,
                    ..
                }) if reviewed_head_sha == NEW_SHA
            ));
        });
    }

    #[test]
    fn lists_saved_review_threads_as_synthetic_history_jobs() {
        with_test_data_dir("synthetic-history", |_| {
            save_review_json(
                "workspace",
                "repo",
                42,
                r#"{
                  "activeThreadId": "thread-1",
                  "threads": [{
                    "id": "thread-1",
                    "title": "Review",
                    "createdAt": "1000",
                    "updatedAt": "2000",
                    "claudeSessionId": "session-1",
                    "messages": [{
                      "id": "msg-1",
                      "role": "assistant",
                      "content": "Looks good",
                      "createdAt": "2000"
                    }]
                  }],
                  "reviewRuns": []
                }"#,
            )
            .expect("save review store");

            let jobs = list_recent_review_jobs(10).expect("list jobs");

            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].workspace, "workspace");
            assert_eq!(jobs[0].repo, "repo");
            assert_eq!(jobs[0].pr_id, 42);
            assert_eq!(jobs[0].status, ReviewJobStatus::Succeeded);
            assert_eq!(jobs[0].trigger, "manual");
            assert_eq!(jobs[0].thread_id.as_deref(), Some("thread-1"));
        });
    }

    #[test]
    fn upserts_and_lists_closed_pr_metrics() {
        with_test_data_dir("closed-pr-metrics", |_| {
            let metric = ClosedPrMetric {
                workspace: "workspace".to_string(),
                repo: "repo".to_string(),
                pr_id: 7,
                title: "Close old endpoint".to_string(),
                author_display_name: "Sam Author".to_string(),
                author_account_id: Some("sam".to_string()),
                state: "MERGED".to_string(),
                source_branch: "feature/endpoint".to_string(),
                destination_branch: "main".to_string(),
                created_on: "2026-06-01T10:00:00.000Z".to_string(),
                updated_on: "2026-06-02T10:00:00.000Z".to_string(),
                additions: 42,
                deletions: 8,
                files_changed: 3,
                diffstat_cached: true,
                risk: ClosedPrRiskSummary {
                    has_ai_review: true,
                    impact: "medium".to_string(),
                    total_findings: 1,
                    high_or_critical_findings: 0,
                    severity_counts: vec![ClosedPrCount {
                        key: "medium".to_string(),
                        count: 1,
                    }],
                    category_counts: vec![ClosedPrCount {
                        key: "test".to_string(),
                        count: 1,
                    }],
                },
                synced_at: "1000".to_string(),
            };

            upsert_closed_pr_metric(&metric).expect("upsert metric");
            let metrics = list_closed_pr_metrics().expect("list metrics");

            assert_eq!(metrics.len(), 1);
            assert_eq!(metrics[0], metric);
        });
    }

    #[test]
    fn derives_closed_pr_risk_summary_from_latest_review_run() {
        with_test_data_dir("closed-pr-risk", |_| {
            save_review_json(
                "workspace",
                "repo",
                9,
                r#"{
                  "threads": [{ "id": "thread-1" }],
                  "reviewRuns": [{
                    "id": "run-1",
                    "findings": [{
                      "severity": "low",
                      "category": "docs"
                    }]
                  }, {
                    "id": "run-2",
                    "findings": [{
                      "severity": "critical",
                      "category": "security"
                    }, {
                      "severity": "medium",
                      "category": "test"
                    }]
                  }]
                }"#,
            )
            .expect("save review json");

            let risk = review_risk_summary("workspace", "repo", 9, 10, 4, 2);

            assert!(risk.has_ai_review);
            assert_eq!(risk.impact, "high");
            assert_eq!(risk.total_findings, 2);
            assert_eq!(risk.high_or_critical_findings, 1);
            assert_eq!(
                risk.severity_counts,
                vec![
                    ClosedPrCount {
                        key: "critical".to_string(),
                        count: 1,
                    },
                    ClosedPrCount {
                        key: "medium".to_string(),
                        count: 1,
                    },
                ]
            );
            assert_eq!(
                risk.category_counts,
                vec![
                    ClosedPrCount {
                        key: "security".to_string(),
                        count: 1,
                    },
                    ClosedPrCount {
                        key: "test".to_string(),
                        count: 1,
                    },
                ]
            );
        });
    }
}
