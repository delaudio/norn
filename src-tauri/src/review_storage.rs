use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::review_event::PullRequestReviewEventProvider;
use crate::review_feedback::{
    derive_finding_feedback_state, ReviewFindingFeedbackAction, ReviewFindingFeedbackEvent,
    ReviewFindingFeedbackIdentity, ReviewFindingFeedbackState, ReviewFindingFeedbackTarget,
};

const APP_DIR: &str = "lachesi";
const DB_FILE: &str = "lachesi.sqlite3";
const LEGACY_REVIEWS_DIR: &str = "reviews";

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
    Ok(())
}

fn now_ms() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|_| "0".to_string())
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
        _ => return Err(rusqlite::Error::InvalidQuery),
    };
    let action = ReviewFindingFeedbackAction::from_str(&row.get::<_, String>(8)?)
        .ok_or(rusqlite::Error::InvalidQuery)?;
    let pr_id = u64::try_from(row.get::<_, i64>(5)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
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
        reason: row.get(11)?,
    })
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
              reviewed_head_sha, run_id, completed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(tenant_id, provider, workspace, repo, pr_id) DO UPDATE SET
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
            SELECT reviewed_head_sha, run_id, completed_at
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
                    reviewed_head_sha: row.get(0)?,
                    run_id: row.get(1)?,
                    completed_at: row.get(2)?,
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

    fn completion(
        outcome: ReviewRunOutcome,
        head_sha: &str,
        expected_previous_head_sha: Option<&str>,
        run_id: &str,
        completed_at: &str,
    ) -> ReviewRunCompletion {
        ReviewRunCompletion {
            identity: cursor_identity(),
            reviewed_head_sha: head_sha.to_string(),
            current_head_sha: head_sha.to_string(),
            expected_previous_head_sha: expected_previous_head_sha.map(str::to_string),
            run_id: run_id.to_string(),
            completed_at: completed_at.to_string(),
            outcome,
        }
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
        with_test_data_dir("finding-feedback-idempotency", |_| {
            let event = feedback_event("delivery-1", ReviewFindingFeedbackAction::Accepted, "1000");

            let first = record_finding_feedback(&event).expect("first delivery");
            let duplicate = record_finding_feedback(&event).expect("duplicate delivery");
            assert_eq!(duplicate, first);
            assert_eq!(duplicate.events.len(), 1);

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
            assert_eq!(versions, vec![1, 2, 3]);
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
