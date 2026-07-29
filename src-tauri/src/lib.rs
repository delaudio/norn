pub mod administrative_audit;
pub mod bitbucket_oauth_onboarding;
pub mod cli;
mod commands;
mod config;
mod credentials;
pub mod finding_publication;
pub mod finding_reconciliation;
pub mod github_app_onboarding;
mod headless_review;
pub mod incremental_review;
mod launch;
mod local_repo;
pub mod operational_telemetry;
pub mod organization_policy;
pub mod repo_config;
pub mod review_evaluation;
pub mod review_event;
pub mod review_feedback;
pub mod review_job;
pub mod review_metrics;
pub mod review_retry;
mod review_storage;
pub mod self_hosted_backup;
pub mod self_hosted_service;
mod services;
pub mod team_authorization;
pub mod team_credentials;
pub mod team_oidc;
pub mod tui;
pub mod webhook_ingress;

pub use review_storage::{
    append_administrative_audit_event, apply_repository_retention, delete_disconnected_repository,
    get_finding_feedback_state, get_review_cursor, purge_administrative_audit_events_before,
    record_finding_feedback, record_review_completion, review_effectiveness_metrics,
    set_team_audit_collection_enabled, team_audit_collection_enabled,
    write_administrative_audit_jsonl, RetentionCount, RetentionMode, RetentionReport,
    RetentionScope, ReviewCursor, ReviewCursorIdentity, ReviewCursorState, ReviewRunCompletion,
    ReviewRunOutcome, AUDIT_METADATA_RETENTION_DAYS, FINDING_ACTIVITY_RETENTION_DAYS,
    REVIEW_CONTENT_RETENTION_DAYS, REVIEW_JOB_RETENTION_DAYS,
};
pub use services::bitbucket::{publish_review_finding_native, reconcile_review_findings_native};

use commands::{bitbucket, context, repositories, review};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager,
};

const TRAY_ID: &str = "lachesi-main";

fn setup_menu_bar(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let status = MenuItem::with_id(app, "status", "● Starting Lachesi...", false, None::<&str>)?;
    let separator_top = PredefinedMenuItem::separator(app)?;
    let open = MenuItem::with_id(app, "open", "Open Lachesi", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "↻ Sync pull requests", true, None::<&str>)?;
    let separator_pull_requests = PredefinedMenuItem::separator(app)?;
    let loading = MenuItem::with_id(
        app,
        "latest-heading",
        "Pull requests loading...",
        false,
        None::<&str>,
    )?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &separator_top,
            &open,
            &sync,
            &separator_pull_requests,
            &loading,
        ],
    )?;
    let icon = app.default_window_icon().cloned();
    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("Lachesi")
        .icon_as_template(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => {
                let _ = app.emit("lachesi-menu-open", ());
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "sync" => {
                let _ = app.emit("lachesi-menu-sync", ());
            }
            id if id.starts_with("pr-") => {
                let _ = app.emit("lachesi-menu-open-pr", id.trim_start_matches("pr-"));
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                let _ = app.emit("lachesi-menu-open", ());
            }
        });
    if let Some(icon) = icon {
        tray = tray.icon(icon);
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(review::AiReviewRunStore::default())
        .manage(review::AiReviewFixStore::default())
        .setup(|app| {
            setup_menu_bar(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bitbucket::load_config,
            bitbucket::validate_repo_review_config,
            bitbucket::save_config,
            bitbucket::save_credentials,
            bitbucket::save_github_token,
            bitbucket::has_credentials,
            bitbucket::clear_credentials,
            bitbucket::save_jira_token,
            bitbucket::save_notion_token,
            bitbucket::test_connection,
            bitbucket::get_current_user,
            bitbucket::list_pull_requests,
            bitbucket::list_closed_pr_metrics,
            bitbucket::sync_closed_pr_metrics,
            review_storage::get_review_effectiveness_metrics,
            bitbucket::get_pull_request,
            bitbucket::approve_pull_request,
            bitbucket::get_branch_status,
            bitbucket::get_diffstat,
            bitbucket::get_pr_diff,
            bitbucket::get_pr_file_preview,
            bitbucket::list_comments,
            bitbucket::publish_review_finding,
            bitbucket::reconcile_review_findings,
            bitbucket::create_inline_comment,
            bitbucket::create_general_comment,
            bitbucket::delete_comment,
            launch::list_review_terminals,
            launch::launch_claude_review,
            context::get_jira_issue,
            context::get_notion_page,
            repositories::list_repository_worktrees,
            repositories::list_repository_files,
            repositories::get_repository_file_diff,
            repositories::read_repository_file,
            repositories::get_repository_file_blame,
            repositories::open_repository_file_external,
            repositories::checkout_repository_branch,
            repositories::fetch_repository,
            repositories::pull_repository,
            repositories::stash_repository,
            review::get_ai_review_run_state,
            review::load_ai_review_store,
            review::create_ai_review_thread,
            review::set_active_ai_review_thread,
            review::delete_ai_review_thread,
            review::record_ai_review_finding_publication,
            review::start_inline_review,
            review::reply_inline_review,
            review::cancel_inline_review,
            review::run_inline_review,
            review::draft_ai_review_comments,
            review::create_ai_review_job,
            review::update_ai_review_job_status,
            review::list_ai_review_jobs,
            review::load_saved_review,
            review::delete_saved_review,
            review::cleanup_stale_reviews,
            review::get_ai_review_fix_state,
            review::start_ai_review_fix,
            review::start_ai_review_commit,
            review::start_ai_review_push,
            review::start_ai_conflict_resolution,
            review::sync_pr_branch,
            review::reset_ai_review_fix_state,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tauri_ipc_smoke {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;
    use tauri::{
        ipc::{CallbackFn, InvokeBody},
        test::{mock_builder, mock_context, noop_assets, INVOKE_KEY},
        webview::InvokeRequest,
    };

    use super::{commands::bitbucket, review_storage};

    fn temp_repo_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after UNIX epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lachesi-tauri-ipc-smoke-{nonce}"));
        fs::create_dir_all(&dir).expect("test repo directory should be created");
        dir
    }

    fn ipc_request(command: &str, body: serde_json::Value) -> InvokeRequest {
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost"
                .parse()
                .expect("local Tauri URL should parse"),
            body: InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        }
    }

    #[test]
    fn validate_repo_review_config_runs_through_tauri_ipc() {
        let repo_dir = temp_repo_dir();
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                bitbucket::validate_repo_review_config
            ])
            .build(mock_context(noop_assets()))
            .expect("mock Tauri app should build");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview should build");

        let response = tauri::test::get_ipc_response(
            &webview,
            ipc_request(
                "validate_repo_review_config",
                json!({ "repoPath": repo_dir.to_string_lossy() }),
            ),
        )
        .expect("validate_repo_review_config should return an IPC response")
        .deserialize::<serde_json::Value>()
        .expect("IPC response should be valid JSON");

        assert_eq!(response["repoPath"], repo_dir.to_string_lossy().as_ref());
        assert_eq!(
            response["configPath"],
            repo_dir.join(".lachesi.yaml").to_string_lossy().as_ref()
        );
        assert_eq!(response["exists"], false);
        assert_eq!(response["config"], serde_json::Value::Null);
        assert_eq!(response["warnings"].as_array().map(Vec::len), Some(0));
        assert_eq!(response["errors"].as_array().map(Vec::len), Some(0));

        fs::remove_dir_all(repo_dir).expect("test repo directory should be removed");
    }

    #[test]
    fn review_effectiveness_metrics_runs_through_tauri_ipc() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                review_storage::get_review_effectiveness_metrics
            ])
            .build(mock_context(noop_assets()))
            .expect("mock Tauri app should build");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview should build");

        let error = tauri::test::get_ipc_response(
            &webview,
            ipc_request(
                "get_review_effectiveness_metrics",
                json!({ "filter": { "tenantId": "" } }),
            ),
        )
        .expect_err("invalid metrics filter should use the command error channel");

        assert!(error
            .as_str()
            .expect("metrics command errors should cross IPC as strings")
            .contains("tenantId"));
    }

    #[test]
    fn publish_review_finding_runs_through_tauri_ipc() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![bitbucket::publish_review_finding])
            .build(mock_context(noop_assets()))
            .expect("mock Tauri app should build");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview should build");

        let error = tauri::test::get_ipc_response(
            &webview,
            ipc_request(
                "publish_review_finding",
                json!({
                    "request": {
                        "schemaVersion": "v1",
                        "tenantId": "",
                        "provider": "github",
                        "workspace": "acme",
                        "repository": "payments",
                        "pullRequestId": 42,
                        "baseSha": "1111111111111111111111111111111111111111",
                        "headSha": "2222222222222222222222222222222222222222",
                        "findingFingerprint": "finding-1",
                        "anchor": {
                            "path": "src/lib.rs",
                            "startLine": 12,
                            "endLine": 12,
                            "side": "new"
                        },
                        "title": "Validate the value",
                        "body": "The value may be absent.",
                        "severity": "high"
                    }
                }),
            ),
        )
        .expect_err("invalid publication request should use the command error channel");

        let serialized = error
            .as_str()
            .expect("publication command errors should cross IPC as strings");
        let publication_error: serde_json::Value =
            serde_json::from_str(serialized).expect("publication error should contain JSON");
        assert_eq!(publication_error["code"], "invalid_request");
        assert_eq!(publication_error["retryable"], false);
    }

    #[test]
    fn reconcile_review_findings_runs_through_tauri_ipc() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                bitbucket::reconcile_review_findings
            ])
            .build(mock_context(noop_assets()))
            .expect("mock Tauri app should build");
        let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("mock webview should build");

        let error = tauri::test::get_ipc_response(
            &webview,
            ipc_request(
                "reconcile_review_findings",
                json!({
                    "request": {
                        "schemaVersion": "v1",
                        "tenantId": "",
                        "provider": "github",
                        "workspace": "acme",
                        "repository": "payments",
                        "pullRequestId": 42,
                        "baseSha": "1111111111111111111111111111111111111111",
                        "headSha": "2222222222222222222222222222222222222222",
                        "trackedComments": [],
                        "currentFindings": []
                    }
                }),
            ),
        )
        .expect_err("invalid reconciliation request should use the command error channel");

        let serialized = error
            .as_str()
            .expect("reconciliation command errors should cross IPC as strings");
        let reconciliation_error: serde_json::Value =
            serde_json::from_str(serialized).expect("reconciliation error should contain JSON");
        assert_eq!(reconciliation_error["code"], "invalid_request");
        assert_eq!(reconciliation_error["retryable"], false);
    }
}
