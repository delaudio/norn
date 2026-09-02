mod diff_server;
mod image_diff;
mod loading;
mod render;
mod terminal;

use std::{
    collections::{hash_map::DefaultHasher, HashMap},
    hash::{Hash, Hasher},
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, RwLock},
    time::Duration,
};

use diff_server::{open_browser_url, WebDiffServer, WebDiffState};

use crossterm::event::{self, Event, KeyCode, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Frame, Line, Modifier, Span},
    widgets::{Clear, Paragraph},
};
use zeroize::Zeroizing;

use crate::config::{self, AiProvider, AppConfig, RepoRef};
use crate::credentials::{self, CredentialProvider, CredentialSource, CredentialStatus};
use crate::readiness::{self, ReadinessIssueSeverity, ReadinessStatus};
use crate::repo_config;
use crate::services::bitbucket::{
    create_general_comment_native, get_pr_file_preview_native,
    get_stable_pull_request_review_snapshot_native, validate_repo_review_config_native, PrComment,
    PullRequestDetail, PullRequestSummary,
};
use crate::services::review::{
    start_inline_review_native, AiReviewRunState, AiReviewRunStatus, AiReviewRunStore,
};

use image_diff::{image_candidate_from_patch, ImageDiffState, TerminalImageSupport};
use loading::{LoadEvent, LoadState, Loader};
use render::{
    detail_view_target, diff_content_width_for_area, diff_image_area_for_area, mouse_target,
    render, selected_diff_file_patch, DetailView, DiffViewMode, DraftComment, FocusPane,
    LoadingView, MouseTarget, PrListFilter, TuiState,
};
use terminal::TerminalGuard;

const TUI_SKIP_AI_REVIEW_ANALYZERS: bool = true;
const TICK_RATE: Duration = Duration::from_millis(250);
const DEFAULT_REVIEW_PROMPT: &str = include_str!("../../../src/lib/defaultReviewPrompt.md");
const CLAUDE_MODELS: [&str; 4] = ["", "sonnet", "opus", "fable"];
const CLAUDE_EFFORTS: [&str; 6] = ["", "low", "medium", "high", "xhigh", "max"];
const CODEX_MODELS: [&str; 3] = ["", "gpt-5.4", "gpt-5.5"];
const CODEX_EFFORTS: [&str; 4] = ["", "low", "medium", "high"];
const SETTING_FIELDS: [&str; 7] = [
    "AI provider",
    "Claude model",
    "Claude effort",
    "Codex model",
    "Codex effort",
    "GitHub credential",
    "Bitbucket credential",
];

pub fn run_from_env() -> Result<(), String> {
    let launch_opts = launch_mode_from_args(std::env::args().skip(1))?;
    let mut config = config::load();
    let resolve_current_repo = match launch_opts.mode {
        TuiLaunchMode::Version => {
            println!("norn-tui {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        TuiLaunchMode::Help => {
            println!("{}", tui_usage());
            return Ok(());
        }
        TuiLaunchMode::CurrentRepo => {
            config.repos.clear();
            true
        }
        TuiLaunchMode::Workspace => false,
    };
    if launch_opts.skip_readiness {
        eprintln!("Skipping readiness preflight by request (--skip-readiness).");
    } else {
        run_tui_preflight(resolve_current_repo, Path::new("."))?;
    }
    let mut app = TuiApp::from_config(config);
    if resolve_current_repo {
        app.focus = FocusPane::PullRequests;
    }
    let mut terminal = TerminalGuard::enter().map_err(|error| error.to_string())?;
    if resolve_current_repo {
        app.resolve_current_repo();
    } else {
        app.load_selected_repo();
    }
    let mut detect_image_support = true;

    loop {
        app.advance_loading();
        let area = terminal.area().map_err(|error| error.to_string())?;
        app.prepare_rendered_diff(area);
        terminal
            .draw(|frame| {
                // Settings is a bounded overlay. Drawing the pure workspace view first
                // preserves useful context behind it without changing application state.
                render(frame, app.view_state());
                if app.settings_open {
                    render_settings(frame, &app);
                }
            })
            .map_err(|error| error.to_string())?;
        if detect_image_support {
            app.image_support = TerminalImageSupport::detect();
            detect_image_support = false;
        }

        if app.should_quit || terminal.interrupted() {
            break;
        }

        if event::poll(TICK_RATE).map_err(|error| error.to_string())? {
            match event::read().map_err(|error| error.to_string())? {
                Event::Key(key) => app.handle_key(key.code),
                Event::Paste(value) => app.handle_paste(&value),
                Event::Mouse(mouse) => {
                    if app.settings_open {
                        continue;
                    }
                    let area = terminal.area().map_err(|error| error.to_string())?;
                    app.handle_mouse(mouse, area);
                }
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
    }

    Ok(())
}

fn run_tui_preflight(resolve_current_repo: bool, cwd: &Path) -> Result<(), String> {
    let report = readiness::collect_report(cwd, !resolve_current_repo);
    let status = readiness::derive_status(&report.issues);
    let has_machine_issues = report
        .issues
        .iter()
        .any(|issue| issue.scope == crate::readiness::ReadinessIssueScope::Machine);
    let has_repository_issues = report
        .issues
        .iter()
        .any(|issue| issue.scope == crate::readiness::ReadinessIssueScope::Repository);
    let has_repository_config_issues = report.issues.iter().any(|issue| {
        issue.scope == crate::readiness::ReadinessIssueScope::Repository
            && (issue.code.starts_with("repository.config")
                || issue.code == "repository.analyzerUnavailable")
    });
    let has_existing_repo_config = report
        .repository
        .config
        .as_ref()
        .is_some_and(|config| config.exists);

    if let ReadinessStatus::Fail = status {
        eprintln!("norn-tui cannot start:");
        for issue in report
            .issues
            .iter()
            .filter(|issue| issue.severity == ReadinessIssueSeverity::Error)
        {
            eprintln!("  - {}: {}", issue.code, issue.message);
            eprintln!("    remediation: {}", issue.remediation);
        }
        if has_machine_issues {
            eprintln!("Machine readiness issues detected; resolve first with `norn setup`.");
            eprintln!("You can rerun `norn-tui` after setup is complete.");
        }
        if has_repository_issues {
            eprintln!("Run `norn doctor --repo-path .` to inspect repository issues.");
        }
        if has_repository_config_issues && has_existing_repo_config {
            eprintln!(
                "Repository config issues detected; run `norn doctor --repo-path .` to repair first."
            );
        }
        eprintln!("Use `norn-tui --skip-readiness` only if setup must be deferred.");
        return Err("Readiness preflight failed; complete machine/repository setup before starting the TUI.".to_string());
    }

    if resolve_current_repo {
        if let Some(git_root) = &report.repository.gitRoot {
            let repo_root = Path::new(git_root);
            if let Ok(Some(_action)) = repo_config::default_init_action_if_needed(repo_root) {
                match repo_config::write_default_repo_config_if_missing(repo_root) {
                    Ok(was_written) => {
                        if was_written {
                            eprintln!(
                                "norn-tui created a default .norn.yaml for first-time onboarding."
                            );
                        }
                    }
                    Err(error) => {
                        eprintln!("norn-tui could not create default .norn.yaml yet: {error}");
                    }
                }
            }
        }
    }

    let warning_count = report
        .issues
        .iter()
        .filter(|issue| issue.severity == ReadinessIssueSeverity::Warning)
        .count();
    if warning_count > 0 {
        eprintln!("norn-tui starting with readiness warnings ({warning_count}):");
        for issue in report
            .issues
            .iter()
            .filter(|issue| issue.severity == ReadinessIssueSeverity::Warning)
        {
            eprintln!("  - {}: {}", issue.code, issue.message);
            eprintln!("    remediation: {}", issue.remediation);
        }
        if has_machine_issues {
            eprintln!(
                "Run `norn setup` to repair machine readiness before entering a strict flow."
            );
        }
        if has_repository_issues {
            if has_existing_repo_config {
                eprintln!(
                    "Run `norn doctor --repo-path .` if repository checks should be repaired."
                );
            } else if has_repository_config_issues {
                eprintln!("Run `norn init --yes` to seed defaults for this repository.");
            }
        }
        eprintln!("Run `norn doctor --repo-path .` to confirm a safe degraded mode.");
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiLaunchMode {
    Help,
    Version,
    CurrentRepo,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TuiLaunchOptions {
    mode: TuiLaunchMode,
    skip_readiness: bool,
}

fn tui_usage() -> &'static str {
    "Usage: norn-tui [--current-repo] [--workspace] [--skip-readiness] [--version]\n\nBy default, norn-tui opens pull requests for the git repository in the current directory."
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsField {
    AiProvider,
    ClaudeModel,
    ClaudeEffort,
    CodexModel,
    CodexEffort,
    GithubCredential,
    BitbucketCredential,
}

impl SettingsField {
    fn as_index(self) -> usize {
        match self {
            Self::AiProvider => 0,
            Self::ClaudeModel => 1,
            Self::ClaudeEffort => 2,
            Self::CodexModel => 3,
            Self::CodexEffort => 4,
            Self::GithubCredential => 5,
            Self::BitbucketCredential => 6,
        }
    }

    fn from_index(index: usize) -> Self {
        match index % SETTING_FIELDS.len() {
            0 => Self::AiProvider,
            1 => Self::ClaudeModel,
            2 => Self::ClaudeEffort,
            3 => Self::CodexModel,
            4 => Self::CodexEffort,
            5 => Self::GithubCredential,
            _ => Self::BitbucketCredential,
        }
    }
}

// Intentionally omit Debug and Clone: token-bearing variants must not be
// printable or duplicated through derived trait implementations.
#[derive(PartialEq, Eq)]
enum SettingsEditor {
    Text {
        field: SettingsField,
        value: String,
    },
    GithubToken {
        token: Zeroizing<String>,
    },
    BitbucketUsername {
        username: String,
    },
    BitbucketToken {
        username: String,
        token: Zeroizing<String>,
    },
}

impl SettingsEditor {
    fn value_mut(&mut self) -> &mut String {
        match self {
            Self::Text { value, .. } => value,
            Self::GithubToken { token } => token,
            Self::BitbucketUsername { username } => username,
            Self::BitbucketToken { token, .. } => token,
        }
    }

    fn is_secret(&self) -> bool {
        matches!(self, Self::GithubToken { .. } | Self::BitbucketToken { .. })
    }

    fn prompt(&self) -> &'static str {
        match self {
            Self::Text { .. } => "Enter a custom value; empty restores the provider default.",
            Self::GithubToken { .. } => {
                "Paste or type a GitHub personal access token. Input is masked."
            }
            Self::BitbucketUsername { .. } => {
                "Enter the username associated with your Bitbucket Cloud token."
            }
            Self::BitbucketToken { .. } => {
                "Paste or type a Bitbucket Cloud API token. Input is masked."
            }
        }
    }

    fn panel_title(&self) -> &'static str {
        match self {
            Self::Text { .. } => " Norn settings · Edit AI review ",
            Self::GithubToken { .. } => " Norn settings · Configure GitHub ",
            Self::BitbucketUsername { .. } | Self::BitbucketToken { .. } => {
                " Norn settings · Configure Bitbucket "
            }
        }
    }

    fn step_label(&self) -> &'static str {
        match self {
            Self::Text { .. } => "Custom value",
            Self::GithubToken { .. } => "GitHub · Secure credential",
            Self::BitbucketUsername { .. } => "Bitbucket · Step 1 of 2 · Account",
            Self::BitbucketToken { .. } => "Bitbucket · Step 2 of 2 · API token",
        }
    }

    fn field_label(&self) -> &'static str {
        match self {
            Self::Text { field, .. } => SETTING_FIELDS[field.as_index()],
            Self::GithubToken { .. } => "Personal access token",
            Self::BitbucketUsername { .. } => "Username",
            Self::BitbucketToken { .. } => "API token",
        }
    }

    fn guidance_lines(&self) -> Vec<Line<'static>> {
        match self {
            Self::Text { .. } => vec![Line::from(Span::styled(
                "The value is saved only when you return to Settings and press s.",
                render::panel_muted_style(),
            ))],
            Self::GithubToken { .. } => vec![
                Line::from(Span::styled(
                    "Storage: OS keychain",
                    render::panel_success_style(),
                )),
                Line::from(Span::styled(
                    "Norn will not display or write the token to its settings file.",
                    render::panel_muted_style(),
                )),
            ],
            Self::BitbucketUsername { .. } => vec![Line::from(Span::styled(
                "Next, Norn asks for the API token and stores both in the OS keychain.",
                render::panel_muted_style(),
            ))],
            Self::BitbucketToken { username, .. } => vec![
                Line::from(vec![
                    Span::styled("Account  ", render::panel_accent_style()),
                    Span::styled(username.clone(), render::panel_text_style()),
                ]),
                Line::from(Span::styled(
                    "The token stays masked and is stored only in the OS keychain.",
                    render::panel_muted_style(),
                )),
            ],
        }
    }

    fn footer_lines(&self) -> Vec<Line<'static>> {
        let action = match self {
            Self::Text { .. } => " apply",
            Self::GithubToken { .. } | Self::BitbucketToken { .. } => " store securely",
            Self::BitbucketUsername { .. } => " continue",
        };
        let escape = if matches!(self, Self::BitbucketToken { .. }) {
            " back"
        } else {
            " cancel"
        };
        vec![
            Line::from(vec![
                Span::styled("Enter", render::panel_accent_style()),
                Span::styled(action, render::panel_muted_style()),
                Span::styled("  Esc", render::panel_accent_style()),
                Span::styled(escape, render::panel_muted_style()),
                Span::styled("  Backspace", render::panel_accent_style()),
                Span::styled(" delete", render::panel_muted_style()),
            ]),
            Line::from(Span::styled(
                if self.is_secret() {
                    "Paste is supported; pasted secrets remain masked."
                } else {
                    "Your other Settings changes remain pending while you edit."
                },
                render::panel_muted_style(),
            )),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditorInputResult {
    appended: usize,
    rejected_for_size: bool,
}

fn append_editor_input(editor: &mut SettingsEditor, input: &str) -> EditorInputResult {
    let current_bytes = editor.value().len();
    let remaining = credentials::MAX_PROVIDER_TOKEN_BYTES.saturating_sub(current_bytes);
    let sanitized = Zeroizing::new(
        input
            .chars()
            .filter(|character| !character.is_control())
            .collect::<String>(),
    );
    if sanitized.len() > remaining {
        return EditorInputResult {
            appended: 0,
            rejected_for_size: true,
        };
    }

    let appended = sanitized.chars().count();
    editor.value_mut().push_str(&sanitized);
    EditorInputResult {
        appended,
        rejected_for_size: false,
    }
}

fn launch_mode_from_args<I, S>(args: I) -> Result<TuiLaunchOptions, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut options = TuiLaunchOptions {
        mode: TuiLaunchMode::CurrentRepo,
        skip_readiness: false,
    };
    for arg in args {
        match arg.as_ref() {
            "--workspace" | "--global" => options.mode = TuiLaunchMode::Workspace,
            "--current-repo" => options.mode = TuiLaunchMode::CurrentRepo,
            "--version" | "-V" => {
                options.mode = TuiLaunchMode::Version;
            }
            "--skip-readiness" => options.skip_readiness = true,
            "-h" | "--help" => {
                return Ok(TuiLaunchOptions {
                    mode: TuiLaunchMode::Help,
                    skip_readiness: false,
                })
            }
            unknown => {
                return Err(format!(
                    "Unknown option `{unknown}`. Use `norn-tui --workspace` for the configured repository picker."
                ));
            }
        }
    }
    Ok(options)
}

fn render_settings(frame: &mut Frame<'_>, app: &TuiApp) {
    let area = settings_overlay_area(frame.area());
    frame.render_widget(Clear, area);

    let title = app
        .settings_editor
        .as_ref()
        .map_or(" Norn settings ", SettingsEditor::panel_title);
    let panel = render::panel_block(title, true);
    let inner = panel.inner(area);
    frame.render_widget(panel, area);

    if let Some(editor) = app.settings_editor.as_ref() {
        render_settings_editor(frame, inner, app, editor);
    } else {
        render_settings_overview(frame, inner, app);
    }
}

fn settings_overlay_area(area: Rect) -> Rect {
    let width = area.width.saturating_sub(4).min(112);
    let height = area.height.saturating_sub(2).min(26);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn render_settings_overview(frame: &mut Frame<'_>, area: Rect, app: &TuiApp) {
    let [content, context, footer] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .split(area)
    else {
        return;
    };

    let lines = vec![
        settings_section_line("AI review"),
        app.settings_field_line(SettingsField::AiProvider),
        app.settings_field_line(SettingsField::ClaudeModel),
        app.settings_field_line(SettingsField::ClaudeEffort),
        app.settings_field_line(SettingsField::CodexModel),
        app.settings_field_line(SettingsField::CodexEffort),
        settings_section_line("Provider credentials"),
        app.settings_field_line(SettingsField::GithubCredential),
        app.settings_field_line(SettingsField::BitbucketCredential),
        settings_section_line("CLI readiness"),
        settings_readiness_line("Claude Code CLI", app.claude_cli_available),
        settings_readiness_line("Codex CLI", app.codex_cli_available),
    ];
    frame.render_widget(Paragraph::new(lines).style(render::panel_style()), content);

    let context_lines = vec![
        Line::from(vec![
            Span::styled("Hint  ", render::panel_accent_style()),
            Span::styled(app.settings_context_help(), render::panel_muted_style()),
        ]),
        Line::from(vec![
            Span::styled("Status  ", render::panel_accent_style()),
            Span::styled(app.status.clone(), render::panel_info_style()),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(context_lines).style(render::panel_style()),
        context,
    );

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("↑/↓", render::panel_accent_style()),
                Span::styled(" navigate  ", render::panel_muted_style()),
                Span::styled("←/→", render::panel_accent_style()),
                Span::styled(" preset  ", render::panel_muted_style()),
                Span::styled("Enter", render::panel_accent_style()),
                Span::styled(" action  ", render::panel_muted_style()),
                Span::styled("e", render::panel_accent_style()),
                Span::styled(" custom", render::panel_muted_style()),
            ]),
            Line::from(vec![
                Span::styled("d", render::panel_accent_style()),
                Span::styled(" remove  ", render::panel_muted_style()),
                Span::styled("v", render::panel_accent_style()),
                Span::styled(" validate  ", render::panel_muted_style()),
                Span::styled("s", render::panel_accent_style()),
                Span::styled(" save  ", render::panel_muted_style()),
                Span::styled("Esc", render::panel_accent_style()),
                Span::styled(" close", render::panel_muted_style()),
            ]),
        ])
        .style(render::panel_style()),
        footer,
    );
}

fn settings_section_line(title: &'static str) -> Line<'static> {
    Line::from(Span::styled(
        title,
        render::panel_accent_style().add_modifier(Modifier::BOLD),
    ))
}

fn settings_readiness_line(label: &'static str, available: bool) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {label:<20}"), render::panel_text_style()),
        Span::styled(
            if available { "Available" } else { "Missing" },
            if available {
                render::panel_success_style()
            } else {
                render::panel_error_style()
            },
        ),
    ])
}

fn render_settings_editor(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &TuiApp,
    editor: &SettingsEditor,
) {
    let [heading, input, guidance, status, footer] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Min(2),
            Constraint::Length(1),
            Constraint::Length(2),
        ])
        .split(area)
    else {
        return;
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                editor.step_label(),
                render::panel_accent_style().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(editor.prompt(), render::panel_muted_style())),
        ])
        .style(render::panel_style()),
        heading,
    );

    let value = editor.value();
    let displayed = if value.is_empty() {
        if editor.is_secret() {
            "Start typing or paste; input stays hidden".to_string()
        } else {
            "Type a value".to_string()
        }
    } else if editor.is_secret() {
        mask_secret(value)
    } else {
        value.to_string()
    };
    let input_style = if value.is_empty() {
        render::panel_muted_style()
    } else {
        render::panel_text_style()
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("> ", render::panel_accent_style()),
            Span::styled(displayed, input_style),
        ]))
        .style(render::panel_style())
        .block(render::panel_block(editor.field_label(), false)),
        input,
    );

    frame.render_widget(
        Paragraph::new(editor.guidance_lines()).style(render::panel_style()),
        guidance,
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Status  ", render::panel_accent_style()),
            Span::styled(app.status.clone(), render::panel_info_style()),
        ]))
        .style(render::panel_style()),
        status,
    );
    frame.render_widget(
        Paragraph::new(editor.footer_lines()).style(render::panel_style()),
        footer,
    );
}

fn mask_secret(value: &str) -> String {
    "•".repeat(value.chars().count())
}

fn user_cli_available_in_path(name: &str) -> bool {
    let executable_names: &[&str] = match name {
        "claude" => &["claude", "claude.exe"],
        "codex" => &["codex", "codex.exe"],
        _ => return false,
    };
    std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path).any(|directory| {
                executable_names
                    .iter()
                    .any(|name| directory.join(name).is_file())
            })
        })
        .unwrap_or(false)
}

fn user_cli_available(name: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        let command = match name {
            "claude" => "command -v claude >/dev/null 2>&1",
            "codex" => "command -v codex >/dev/null 2>&1",
            _ => return false,
        };
        Command::new("/bin/zsh")
            .arg("-lc")
            .arg(command)
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(not(target_os = "macos"))]
    {
        user_cli_available_in_path(name)
    }
}

fn credential_status_label(status: CredentialStatus) -> String {
    if !status.available {
        return "Not configured".to_string();
    }
    match status.source {
        CredentialSource::Keychain => "Configured · OS keychain".to_string(),
        CredentialSource::Environment => "Available · environment".to_string(),
        CredentialSource::None => "Not configured".to_string(),
    }
}

fn credential_action_label(status: CredentialStatus) -> &'static str {
    match (status.available, status.source) {
        (true, CredentialSource::Keychain) => "Enter replace · d remove",
        (true, CredentialSource::Environment) => "Enter configure keychain",
        _ => "Enter configure",
    }
}

impl SettingsEditor {
    fn value(&self) -> &str {
        match self {
            Self::Text { value, .. } => value,
            Self::GithubToken { token } => token,
            Self::BitbucketUsername { username } => username,
            Self::BitbucketToken { token, .. } => token,
        }
    }
}

struct TuiApp {
    repos: Vec<RepoRef>,
    selected_repo: usize,
    focus: FocusPane,
    pull_requests: Vec<PullRequestSummary>,
    pr_filter: PrListFilter,
    selected_pr: usize,
    detail: Option<PullRequestDetail>,
    comments: Vec<PrComment>,
    ai_reviewed_pr_ids: Vec<u32>,
    ai_review_running_pr_ids: Vec<u32>,
    diff: Option<String>,
    drafts: Vec<DraftComment>,
    composer: Option<String>,
    next_draft_id: u64,
    ai_provider: AiProvider,
    claude_model: Option<String>,
    claude_effort: Option<String>,
    codex_model: Option<String>,
    codex_effort: Option<String>,
    settings_open: bool,
    settings_field: SettingsField,
    settings_ai_provider: AiProvider,
    settings_claude_model: Option<String>,
    settings_claude_effort: Option<String>,
    settings_codex_model: Option<String>,
    settings_codex_effort: Option<String>,
    settings_editor: Option<SettingsEditor>,
    github_credential_status: CredentialStatus,
    bitbucket_credential_status: CredentialStatus,
    claude_cli_available: bool,
    codex_cli_available: bool,
    ai_review_store: AiReviewRunStore,
    active_ai_target: Option<(String, String, u32)>,
    ai_review_state: Option<AiReviewRunState>,
    ai_review_output: Option<String>,
    detail_view: DetailView,
    detail_scroll: usize,
    ai_review_scroll: usize,
    diff_scroll: usize,
    selected_diff_file: usize,
    diff_view_mode: DiffViewMode,
    rendered_diff: Option<RenderedDiffCache>,
    image_diff: Option<ImageDiffState>,
    image_support: TerminalImageSupport,
    loader: Loader,
    next_request_id: u64,
    repo_request_id: u64,
    pr_request_id: u64,
    ai_request_id: u64,
    marker_generation: u64,
    marker_mutations: HashMap<u32, u64>,
    repo_load: LoadState,
    pr_list_load: LoadState,
    detail_load: LoadState,
    comments_load: LoadState,
    diff_load: LoadState,
    ai_review_load: LoadState,
    spinner_tick: usize,
    ai_poll_tick: usize,
    error: Option<String>,
    status: String,
    diff_prompt_open: bool,
    web_diff_server: Option<WebDiffServer>,
    web_diff_state: Arc<RwLock<WebDiffState>>,
    should_quit: bool,
}

struct RenderedDiffCache {
    selected_file: usize,
    mode: DiffViewMode,
    width: usize,
    patch_hash: u64,
    output: Option<String>,
}

impl TuiApp {
    fn from_config(config: AppConfig) -> Self {
        let claude_model = config.claude_model.clone();
        let claude_effort = config.claude_effort.clone();
        let codex_model = config.codex_model.clone();
        let codex_effort = config.codex_effort.clone();
        Self {
            ai_provider: config.ai_provider,
            claude_model: claude_model.clone(),
            claude_effort: claude_effort.clone(),
            codex_model: codex_model.clone(),
            codex_effort: codex_effort.clone(),
            settings_ai_provider: config.ai_provider,
            settings_claude_model: claude_model,
            settings_claude_effort: claude_effort,
            settings_codex_model: codex_model,
            settings_codex_effort: codex_effort,
            github_credential_status: credentials::credential_status(CredentialProvider::Github),
            bitbucket_credential_status: credentials::credential_status(
                CredentialProvider::Bitbucket,
            ),
            claude_cli_available: user_cli_available_in_path("claude"),
            codex_cli_available: user_cli_available_in_path("codex"),
            ..Self::from_repos(config.repos)
        }
    }

    fn from_repos(repos: Vec<RepoRef>) -> Self {
        Self {
            repos,
            selected_repo: 0,
            settings_open: false,
            settings_field: SettingsField::AiProvider,
            settings_ai_provider: AiProvider::default(),
            settings_claude_model: None,
            settings_claude_effort: None,
            settings_codex_model: None,
            settings_codex_effort: None,
            settings_editor: None,
            github_credential_status: CredentialStatus {
                provider: CredentialProvider::Github,
                available: false,
                source: CredentialSource::None,
            },
            bitbucket_credential_status: CredentialStatus {
                provider: CredentialProvider::Bitbucket,
                available: false,
                source: CredentialSource::None,
            },
            claude_cli_available: false,
            codex_cli_available: false,
            focus: FocusPane::Repositories,
            pull_requests: Vec::new(),
            pr_filter: PrListFilter::Open,
            selected_pr: 0,
            detail: None,
            comments: Vec::new(),
            ai_reviewed_pr_ids: Vec::new(),
            ai_review_running_pr_ids: Vec::new(),
            diff: None,
            drafts: Vec::new(),
            composer: None,
            next_draft_id: 1,
            ai_provider: AiProvider::default(),
            claude_model: None,
            claude_effort: None,
            codex_model: None,
            codex_effort: None,
            ai_review_store: AiReviewRunStore::default(),
            active_ai_target: None,
            ai_review_state: None,
            ai_review_output: None,
            detail_view: DetailView::PullRequest,
            detail_scroll: 0,
            ai_review_scroll: 0,
            diff_scroll: 0,
            selected_diff_file: 0,
            diff_view_mode: DiffViewMode::Unified,
            rendered_diff: None,
            image_diff: None,
            image_support: TerminalImageSupport::metadata_only(),
            loader: Loader::new(),
            next_request_id: 1,
            repo_request_id: 0,
            pr_request_id: 0,
            ai_request_id: 0,
            marker_generation: 0,
            marker_mutations: HashMap::new(),
            repo_load: LoadState::Idle,
            pr_list_load: LoadState::Idle,
            detail_load: LoadState::Idle,
            comments_load: LoadState::Idle,
            diff_load: LoadState::Idle,
            ai_review_load: LoadState::Idle,
            spinner_tick: 0,
            ai_poll_tick: 0,
            error: None,
            status: "Ready".to_string(),
            diff_prompt_open: false,
            web_diff_server: None,
            web_diff_state: Arc::new(RwLock::new(WebDiffState::default())),
            should_quit: false,
        }
    }

    fn next_request(&mut self) -> u64 {
        let request_id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1).max(1);
        request_id
    }

    fn resolve_current_repo(&mut self) {
        let request_id = self.next_request();
        self.repo_request_id = request_id;
        self.repo_load = LoadState::Loading;
        self.status = "Resolving repository...".to_string();
        self.loader.resolve_current_repo(request_id);
    }

    fn advance_loading(&mut self) {
        self.spinner_tick = self.spinner_tick.wrapping_add(1);
        while let Some(event) = self.loader.try_recv() {
            self.apply_load_event(event);
        }
        self.ai_poll_tick = self.ai_poll_tick.wrapping_add(1);
        if self.ai_poll_tick.is_multiple_of(4)
            && !self.ai_review_load.is_loading()
            && matches!(
                self.ai_review_state.as_ref().map(|state| state.status),
                Some(AiReviewRunStatus::Running)
            )
        {
            if let Some((workspace, repo, pr_id)) = self.active_ai_target.clone() {
                let request_id = self.next_request();
                self.ai_request_id = request_id;
                self.ai_review_load = LoadState::Loading;
                self.loader.ai_review(
                    request_id,
                    workspace,
                    repo,
                    pr_id,
                    self.ai_review_store.clone(),
                );
            }
        }
    }

    fn apply_load_event(&mut self, event: LoadEvent) {
        match event {
            LoadEvent::CurrentRepo { request_id, result } if request_id == self.repo_request_id => {
                match result {
                    Ok(repo) => {
                        self.repos = vec![repo];
                        self.selected_repo = 0;
                        self.repo_load = LoadState::Ready;
                        self.load_selected_repo();
                    }
                    Err(error) => {
                        self.repo_load = LoadState::Failed(error.clone());
                        self.status = "Failed to resolve repository".to_string();
                    }
                }
            }
            LoadEvent::PullRequests { request_id, result }
                if request_id == self.repo_request_id =>
            {
                match result {
                    Ok(pull_requests) => {
                        self.pull_requests = pull_requests
                            .into_iter()
                            .filter(|pr| self.pr_filter.includes(pr))
                            .collect();
                        self.selected_pr = 0;
                        self.detail = None;
                        self.comments.clear();
                        self.diff = None;
                        self.drafts.clear();
                        self.composer = None;
                        self.active_ai_target = None;
                        self.ai_review_state = None;
                        self.ai_review_output = None;
                        self.detail_view = DetailView::PullRequest;
                        self.reset_detail_scrolls();
                        self.reset_diff_state();
                        self.pr_list_load = LoadState::Ready;
                        self.error = None;
                        self.status = format!(
                            "Loaded {} {} PRs",
                            self.pull_requests.len(),
                            self.pr_filter.label()
                        );
                        if let Some(repo) = self.repos.get(self.selected_repo) {
                            self.loader.review_markers(
                                request_id,
                                repo.workspace.clone(),
                                repo.repo.clone(),
                                self.pull_requests.iter().map(|pr| pr.id).collect(),
                                self.ai_review_store.clone(),
                                self.marker_generation,
                            );
                        }
                        if !self.pull_requests.is_empty() {
                            self.load_selected_pr();
                        } else {
                            self.detail_load = LoadState::Idle;
                            self.comments_load = LoadState::Idle;
                            self.diff_load = LoadState::Idle;
                            self.ai_review_load = LoadState::Idle;
                        }
                    }
                    Err(error) => {
                        self.pr_list_load = LoadState::Failed(error.clone());
                        self.status = "Failed to load PRs".to_string();
                    }
                }
            }
            LoadEvent::Detail { request_id, result } if request_id == self.pr_request_id => {
                match result {
                    Ok(detail) => {
                        self.detail = Some(detail);
                        self.detail_load = LoadState::Ready;
                        self.drafts.clear();
                        self.composer = None;
                    }
                    Err(error) => {
                        self.detail_load = LoadState::Failed(error.clone());
                    }
                }
                self.finish_pr_load_status();
            }
            LoadEvent::Comments { request_id, result } if request_id == self.pr_request_id => {
                match result {
                    Ok(comments) => {
                        self.comments = comments;
                        self.comments_load = LoadState::Ready;
                    }
                    Err(error) => {
                        self.comments_load = LoadState::Failed(error.clone());
                    }
                }
                self.finish_pr_load_status();
            }
            LoadEvent::Diff { request_id, result } if request_id == self.pr_request_id => {
                match result {
                    Ok(diff) => {
                        self.diff = Some(diff);
                        self.diff_load = LoadState::Ready;
                        self.reset_diff_state();
                    }
                    Err(error) => {
                        self.diff_load = LoadState::Failed(error.clone());
                    }
                }
                self.finish_pr_load_status();
            }
            LoadEvent::AiReview {
                request_id,
                pr_id,
                state,
                output,
            } if request_id == self.ai_request_id => {
                self.ai_review_state = state;
                match output {
                    Ok(output) => {
                        self.ai_review_output = output;
                        self.ai_review_load = LoadState::Ready;
                    }
                    Err(error) => {
                        self.ai_review_load = LoadState::Failed(error.clone());
                    }
                }
                self.update_ai_review_markers(pr_id);
                self.finish_pr_load_status();
            }
            LoadEvent::ReviewMarkers {
                request_id,
                marker_generation,
                reviewed,
                running,
            } if request_id == self.repo_request_id => {
                self.apply_marker_snapshot(marker_generation, reviewed, running);
            }
            _ => {}
        }
    }

    fn update_ai_review_markers(&mut self, pr_id: u32) {
        match self.ai_review_state.as_ref().map(|state| state.status) {
            Some(AiReviewRunStatus::Running) => self.mark_ai_review_running(pr_id),
            Some(AiReviewRunStatus::Succeeded) => {
                self.unmark_ai_review_running(pr_id);
                self.mark_ai_reviewed(pr_id);
            }
            Some(AiReviewRunStatus::Failed | AiReviewRunStatus::Cancelled) => {
                self.unmark_ai_review_running(pr_id);
            }
            _ => {}
        }
    }

    fn finish_pr_load_status(&mut self) {
        if [
            &self.detail_load,
            &self.comments_load,
            &self.diff_load,
            &self.ai_review_load,
        ]
        .iter()
        .any(|state| state.is_loading())
        {
            return;
        }
        if let Some((_, _, pr_id)) = self.active_ai_target.as_ref() {
            self.status = match self.ai_review_state.as_ref() {
                Some(state) if state.status == AiReviewRunStatus::Running => format!(
                    "AI review running: {}",
                    state.logs.last().map(String::as_str).unwrap_or("started")
                ),
                _ => format!("Loaded PR #{pr_id}"),
            };
        }
    }

    fn open_settings(&mut self) {
        self.settings_open = true;
        self.settings_field = SettingsField::AiProvider;
        self.settings_ai_provider = self.ai_provider;
        self.settings_claude_model = self.claude_model.clone();
        self.settings_claude_effort = self.claude_effort.clone();
        self.settings_codex_model = self.codex_model.clone();
        self.settings_codex_effort = self.codex_effort.clone();
        self.settings_editor = None;
        self.refresh_credential_statuses();
        self.refresh_cli_readiness();
        self.status = "Settings opened: use arrows to navigate, e to edit, s to save".to_string();
    }

    fn close_settings(&mut self) {
        self.settings_open = false;
        self.settings_field = SettingsField::AiProvider;
        self.settings_editor = None;
        self.status = "Ready".to_string();
    }

    fn discard_settings(&mut self) {
        self.close_settings();
        self.status = "Settings cancelled".to_string();
    }

    fn cycle_or_value(options: &[&str], current: &Option<String>, forward: bool) -> Option<String> {
        let current_value = current.as_deref().unwrap_or("");
        let mut index = options
            .iter()
            .position(|value| value == &current_value)
            .unwrap_or(0);
        if forward {
            index = (index + 1).rem_euclid(options.len());
        } else {
            index = index.saturating_sub(1);
            index = index.rem_euclid(options.len());
        }
        match options[index] {
            "" => None,
            value => Some(value.to_string()),
        }
    }

    fn cycle_settings_value(&mut self, forward: bool) {
        match self.settings_field {
            SettingsField::AiProvider => {
                self.settings_ai_provider =
                    if matches!(self.settings_ai_provider, AiProvider::Claude) {
                        AiProvider::Codex
                    } else {
                        AiProvider::Claude
                    };
            }
            SettingsField::ClaudeModel => {
                self.settings_claude_model =
                    Self::cycle_or_value(&CLAUDE_MODELS, &self.settings_claude_model, forward);
            }
            SettingsField::ClaudeEffort => {
                self.settings_claude_effort =
                    Self::cycle_or_value(&CLAUDE_EFFORTS, &self.settings_claude_effort, forward);
            }
            SettingsField::CodexModel => {
                self.settings_codex_model =
                    Self::cycle_or_value(&CODEX_MODELS, &self.settings_codex_model, forward);
            }
            SettingsField::CodexEffort => {
                self.settings_codex_effort =
                    Self::cycle_or_value(&CODEX_EFFORTS, &self.settings_codex_effort, forward);
            }
            SettingsField::GithubCredential | SettingsField::BitbucketCredential => {}
        }
    }

    fn refresh_credential_statuses(&mut self) {
        self.github_credential_status = credentials::credential_status(CredentialProvider::Github);
        self.bitbucket_credential_status =
            credentials::credential_status(CredentialProvider::Bitbucket);
    }

    fn refresh_settings_readiness(&mut self) {
        self.refresh_credential_statuses();
        self.refresh_cli_readiness();
        self.status = "Provider readiness refreshed".to_string();
    }

    fn refresh_cli_readiness(&mut self) {
        // Keep the startup scan PATH-only and fast. This explicit settings action
        // may use a login shell on macOS to discover shell-managed tool paths.
        self.claude_cli_available = user_cli_available("claude");
        self.codex_cli_available = user_cli_available("codex");
    }

    fn open_settings_editor(&mut self) {
        self.error = None;
        self.settings_editor = match self.settings_field {
            SettingsField::ClaudeModel => Some(SettingsEditor::Text {
                field: self.settings_field,
                value: self.settings_claude_model.clone().unwrap_or_default(),
            }),
            SettingsField::ClaudeEffort => Some(SettingsEditor::Text {
                field: self.settings_field,
                value: self.settings_claude_effort.clone().unwrap_or_default(),
            }),
            SettingsField::CodexModel => Some(SettingsEditor::Text {
                field: self.settings_field,
                value: self.settings_codex_model.clone().unwrap_or_default(),
            }),
            SettingsField::CodexEffort => Some(SettingsEditor::Text {
                field: self.settings_field,
                value: self.settings_codex_effort.clone().unwrap_or_default(),
            }),
            SettingsField::GithubCredential => Some(SettingsEditor::GithubToken {
                token: Zeroizing::new(String::new()),
            }),
            SettingsField::BitbucketCredential => Some(SettingsEditor::BitbucketUsername {
                username: String::new(),
            }),
            SettingsField::AiProvider => None,
        };
        if let Some(editor) = self.settings_editor.as_ref() {
            self.status = editor.prompt().to_string();
        }
    }

    fn cancel_settings_editor(&mut self) {
        let Some(editor) = self.settings_editor.take() else {
            return;
        };
        match editor {
            SettingsEditor::BitbucketToken { username, .. } => {
                self.settings_editor = Some(SettingsEditor::BitbucketUsername { username });
                self.status = "Back to Bitbucket username".to_string();
            }
            _ => {
                self.status = "Setting edit cancelled".to_string();
            }
        }
    }

    fn apply_settings_editor(&mut self) {
        let Some(editor) = self.settings_editor.take() else {
            return;
        };
        match editor {
            SettingsEditor::Text { field, value } => {
                let value = (!value.trim().is_empty()).then(|| value.trim().to_string());
                match field {
                    SettingsField::ClaudeModel => self.settings_claude_model = value,
                    SettingsField::ClaudeEffort => self.settings_claude_effort = value,
                    SettingsField::CodexModel => self.settings_codex_model = value,
                    SettingsField::CodexEffort => self.settings_codex_effort = value,
                    _ => {}
                }
                self.status = "Custom setting updated; press s to save".to_string();
            }
            SettingsEditor::GithubToken { token } => {
                match credentials::store_provider_credential(
                    CredentialProvider::Github,
                    None,
                    &token,
                ) {
                    Ok(()) => {
                        self.error = None;
                        self.refresh_credential_statuses();
                        self.status = "GitHub credential stored in the OS keychain".to_string();
                    }
                    Err(error) => {
                        self.error = Some(error);
                        self.status = "Failed to store GitHub credential".to_string();
                        self.settings_editor = Some(SettingsEditor::GithubToken { token });
                    }
                }
            }
            SettingsEditor::BitbucketUsername { username } => {
                if username.trim().is_empty() {
                    self.status = "Bitbucket username cannot be empty".to_string();
                    self.settings_editor = Some(SettingsEditor::BitbucketUsername { username });
                } else {
                    self.settings_editor = Some(SettingsEditor::BitbucketToken {
                        username: username.trim().to_string(),
                        token: Zeroizing::new(String::new()),
                    });
                    self.status = "Enter the Bitbucket token; input is masked".to_string();
                }
            }
            SettingsEditor::BitbucketToken { username, token } => {
                match credentials::store_provider_credential(
                    CredentialProvider::Bitbucket,
                    Some(&username),
                    &token,
                ) {
                    Ok(()) => {
                        self.error = None;
                        self.refresh_credential_statuses();
                        self.status = "Bitbucket credential stored in the OS keychain".to_string();
                    }
                    Err(error) => {
                        self.error = Some(error);
                        self.status = "Failed to store Bitbucket credential".to_string();
                        self.settings_editor =
                            Some(SettingsEditor::BitbucketToken { username, token });
                    }
                }
            }
        }
    }

    fn remove_selected_credential(&mut self) {
        let provider = match self.settings_field {
            SettingsField::GithubCredential => CredentialProvider::Github,
            SettingsField::BitbucketCredential => CredentialProvider::Bitbucket,
            _ => {
                self.status =
                    "Select a credential field before removing a keychain entry".to_string();
                return;
            }
        };
        let status = match provider {
            CredentialProvider::Github => self.github_credential_status,
            CredentialProvider::Bitbucket => self.bitbucket_credential_status,
        };
        if status.source != CredentialSource::Keychain || !status.available {
            self.status = match status.source {
                CredentialSource::Environment if status.available => {
                    "Environment credentials cannot be removed from Norn".to_string()
                }
                _ => "No OS keychain credential to remove".to_string(),
            };
            return;
        }
        match credentials::clear_provider_credential(provider) {
            Ok(()) => {
                self.refresh_credential_statuses();
                self.status = format!("Removed {} OS keychain credential", provider.label());
            }
            Err(error) => {
                self.error = Some(error);
                self.status = format!("Failed to remove {} credential", provider.label());
            }
        }
    }

    fn next_settings_field(&mut self) {
        let index = self.settings_field.as_index();
        self.settings_field = SettingsField::from_index((index + 1) % SETTING_FIELDS.len());
    }

    fn previous_settings_field(&mut self) {
        let index = self.settings_field.as_index();
        self.settings_field =
            SettingsField::from_index((index + SETTING_FIELDS.len() - 1) % SETTING_FIELDS.len());
    }

    fn persist_settings(&mut self) -> Result<(), String> {
        let mut cfg = config::load();
        cfg.ai_provider = self.settings_ai_provider;
        cfg.claude_model = self.settings_claude_model.clone();
        cfg.claude_effort = self.settings_claude_effort.clone();
        cfg.codex_model = self.settings_codex_model.clone();
        cfg.codex_effort = self.settings_codex_effort.clone();
        config::save(&cfg)?;
        self.ai_provider = self.settings_ai_provider;
        self.claude_model = self.settings_claude_model.clone();
        self.claude_effort = self.settings_claude_effort.clone();
        self.codex_model = self.settings_codex_model.clone();
        self.codex_effort = self.settings_codex_effort.clone();
        Ok(())
    }

    fn save_settings(&mut self) {
        match self.persist_settings() {
            Ok(()) => {
                self.close_settings();
                self.status = "Settings saved.".to_string();
            }
            Err(error) => {
                self.error = Some(error.clone());
                self.status = "Failed to save settings".to_string();
            }
        }
    }

    fn option_label(value: &Option<String>) -> String {
        value.clone().unwrap_or_else(|| "default".to_string())
    }

    fn render_settings_field_value(&self, field: SettingsField) -> String {
        match field {
            SettingsField::AiProvider => match self.settings_ai_provider {
                AiProvider::Claude => "Claude".to_string(),
                AiProvider::Codex => "Codex".to_string(),
            },
            SettingsField::ClaudeModel => Self::option_label(&self.settings_claude_model),
            SettingsField::ClaudeEffort => Self::option_label(&self.settings_claude_effort),
            SettingsField::CodexModel => Self::option_label(&self.settings_codex_model),
            SettingsField::CodexEffort => Self::option_label(&self.settings_codex_effort),
            SettingsField::GithubCredential => {
                credential_status_label(self.github_credential_status)
            }
            SettingsField::BitbucketCredential => {
                credential_status_label(self.bitbucket_credential_status)
            }
        }
    }

    fn settings_field_line(&self, field: SettingsField) -> Line<'static> {
        let selected = self.settings_field == field;
        let mut spans = vec![
            Span::styled(
                if selected { "› " } else { "  " },
                if selected {
                    render::panel_accent_style()
                } else {
                    render::panel_muted_style()
                },
            ),
            Span::styled(
                format!("{:<22}", SETTING_FIELDS[field.as_index()]),
                if selected {
                    render::panel_text_style().add_modifier(Modifier::BOLD)
                } else {
                    render::panel_text_style()
                },
            ),
            Span::styled(
                self.render_settings_field_value(field),
                if matches!(
                    field,
                    SettingsField::GithubCredential | SettingsField::BitbucketCredential
                ) {
                    let status = if field == SettingsField::GithubCredential {
                        self.github_credential_status
                    } else {
                        self.bitbucket_credential_status
                    };
                    if status.available {
                        render::panel_success_style()
                    } else {
                        render::panel_muted_style()
                    }
                } else {
                    render::panel_info_style()
                },
            ),
        ];
        if matches!(
            field,
            SettingsField::GithubCredential | SettingsField::BitbucketCredential
        ) {
            let status = if field == SettingsField::GithubCredential {
                self.github_credential_status
            } else {
                self.bitbucket_credential_status
            };
            spans.push(Span::styled("  ", render::panel_text_style()));
            spans.push(Span::styled(
                credential_action_label(status),
                if selected {
                    render::panel_accent_style()
                } else {
                    render::panel_muted_style()
                },
            ));
        }
        Line::from(spans)
    }

    fn settings_context_help(&self) -> String {
        match self.settings_field {
            SettingsField::AiProvider => {
                "Choose the assistant used for new TUI reviews.".to_string()
            }
            SettingsField::ClaudeModel
            | SettingsField::ClaudeEffort
            | SettingsField::CodexModel
            | SettingsField::CodexEffort => {
                "Use ←/→ for presets or e to enter a custom value.".to_string()
            }
            SettingsField::GithubCredential | SettingsField::BitbucketCredential => {
                let status = if self.settings_field == SettingsField::GithubCredential {
                    self.github_credential_status
                } else {
                    self.bitbucket_credential_status
                };
                match status.source {
                    CredentialSource::Keychain if status.available => {
                        "Enter replaces the keychain credential; d removes it.".to_string()
                    }
                    CredentialSource::Environment if status.available => {
                        "Environment credential is active; Enter configures the keychain."
                            .to_string()
                    }
                    _ => "Enter starts secure setup; token input stays masked.".to_string(),
                }
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode) {
        if self.settings_open {
            if self.settings_editor.is_some() {
                match code {
                    KeyCode::Esc => self.cancel_settings_editor(),
                    KeyCode::Enter => self.apply_settings_editor(),
                    KeyCode::Backspace => {
                        if let Some(editor) = self.settings_editor.as_mut() {
                            editor.value_mut().pop();
                        }
                    }
                    KeyCode::Char(character) => {
                        if let Some(editor) = self.settings_editor.as_mut() {
                            let field = editor.field_label();
                            let result = append_editor_input(editor, &character.to_string());
                            if result.rejected_for_size {
                                self.status = format!("{field} reached the input size limit");
                            }
                        }
                    }
                    _ => {}
                }
                return;
            }
            match code {
                KeyCode::Esc => {
                    self.discard_settings();
                }
                KeyCode::Enter => self.open_settings_editor(),
                KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => self.next_settings_field(),
                KeyCode::Up | KeyCode::Char('k') => self.previous_settings_field(),
                KeyCode::Left => self.cycle_settings_value(false),
                KeyCode::Right => self.cycle_settings_value(true),
                KeyCode::Char('e') => self.open_settings_editor(),
                KeyCode::Char('d') => self.remove_selected_credential(),
                KeyCode::Char('v') => self.refresh_settings_readiness(),
                KeyCode::Char('s') => self.save_settings(),
                _ => {}
            }
            return;
        }
        if self.diff_prompt_open {
            match code {
                KeyCode::Char('g') => {
                    self.diff_prompt_open = false;
                    self.open_diff_view();
                }
                KeyCode::Char('b') => {
                    self.diff_prompt_open = false;
                    self.open_browser_diff();
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.diff_prompt_open = false;
                    self.status = "Diff view cancelled".to_string();
                }
                _ => {}
            }
            return;
        }
        if self.composer.is_some() {
            self.handle_composer_key(code);
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Tab => self.toggle_focus(),
            KeyCode::Enter => self.load_selected_pr(),
            KeyCode::Char('c') => self.start_comment_composer(),
            KeyCode::Char('p') => self.publish_drafts(),
            KeyCode::Char('x') => self.discard_drafts(),
            KeyCode::Char('a') => self.start_ai_review(),
            KeyCode::Char('f') => self.cycle_pr_filter(),
            KeyCode::Char('g') => self.prompt_or_toggle_diff(),
            KeyCode::Char('b') => self.open_browser_diff(),
            KeyCode::Char('u') => self.toggle_diff_view_mode(),
            KeyCode::Char('i') => self.toggle_image_side(),
            KeyCode::Char('v') => self.toggle_detail_view(),
            KeyCode::Char('y') => self.copy_ai_review_output(),
            KeyCode::Char('r') => self.refresh_active_view(),
            KeyCode::Char('s') => self.open_settings(),
            KeyCode::PageUp => self.scroll_active_detail(-10),
            KeyCode::PageDown => self.scroll_active_detail(10),
            KeyCode::Home => self.reset_active_detail_scroll(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            _ => {}
        }
    }

    fn handle_paste(&mut self, value: &str) {
        if !self.settings_open {
            return;
        }
        let Some(editor) = self.settings_editor.as_mut() else {
            return;
        };
        let is_secret = editor.is_secret();
        let field = editor.field_label();
        let result = append_editor_input(editor, value);
        self.status = if result.rejected_for_size {
            format!("{field} was not pasted because it exceeds the input size limit")
        } else if result.appended == 0 {
            format!("Nothing pasted into {field}")
        } else if is_secret {
            format!("Pasted into {field}; secret remains masked")
        } else {
            format!("Pasted into {field}")
        };
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, area: ratatui::layout::Rect) {
        if self.composer.is_some() {
            return;
        }
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if self.detail_view == DetailView::Diff {
                    self.scroll_active_detail(-3);
                    return;
                }
                self.scroll_detail_at(area, mouse.column, mouse.row, -3);
            }
            MouseEventKind::ScrollDown => {
                if self.detail_view == DetailView::Diff {
                    self.scroll_active_detail(3);
                    return;
                }
                self.scroll_detail_at(area, mouse.column, mouse.row, 3);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                match mouse_target(area, mouse.column, mouse.row, self.view_state()) {
                    Some(MouseTarget::Repository(index)) => self.select_repo(index),
                    Some(MouseTarget::PullRequest(index)) => self.select_pr(index),
                    Some(MouseTarget::PrFilter(filter)) => self.set_pr_filter(filter),
                    Some(MouseTarget::DiffFile(index)) => self.select_diff_file(index),
                    None => {
                        if let Some(view) =
                            detail_view_target(area, mouse.column, mouse.row, self.detail_view)
                        {
                            self.detail_view = view;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_composer_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.composer = None;
                self.status = "Comment draft cancelled".to_string();
            }
            KeyCode::Enter => self.stage_composer_comment(),
            KeyCode::Backspace => {
                if let Some(composer) = self.composer.as_mut() {
                    composer.pop();
                }
            }
            KeyCode::Char(character) => {
                if let Some(composer) = self.composer.as_mut() {
                    composer.push(character);
                }
            }
            _ => {}
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            FocusPane::Repositories => FocusPane::PullRequests,
            FocusPane::PullRequests if self.detail_view == DetailView::Diff => FocusPane::Diff,
            FocusPane::PullRequests => FocusPane::Repositories,
            FocusPane::Diff => FocusPane::Repositories,
        };
    }

    fn select_next(&mut self) {
        match self.focus {
            FocusPane::Repositories => self.select_next_repo(),
            FocusPane::PullRequests => self.select_next_pr(),
            FocusPane::Diff => self.select_next_diff_file(),
        }
    }

    fn select_previous(&mut self) {
        match self.focus {
            FocusPane::Repositories => self.select_previous_repo(),
            FocusPane::PullRequests => self.select_previous_pr(),
            FocusPane::Diff => self.select_previous_diff_file(),
        }
    }

    fn select_next_repo(&mut self) {
        if self.repos.is_empty() {
            self.selected_repo = 0;
            return;
        }
        let previous = self.selected_repo;
        self.selected_repo = (self.selected_repo + 1).min(self.repos.len() - 1);
        if self.selected_repo != previous {
            self.load_selected_repo();
        }
    }

    fn select_previous_repo(&mut self) {
        let previous = self.selected_repo;
        self.selected_repo = self.selected_repo.saturating_sub(1);
        if self.selected_repo != previous {
            self.load_selected_repo();
        }
    }

    fn select_repo(&mut self, index: usize) {
        if index >= self.repos.len() {
            return;
        }
        self.focus = FocusPane::Repositories;
        if self.selected_repo != index {
            self.selected_repo = index;
            self.load_selected_repo();
        }
    }

    fn select_next_pr(&mut self) {
        if self.pull_requests.is_empty() {
            self.selected_pr = 0;
            return;
        }
        self.selected_pr = (self.selected_pr + 1).min(self.pull_requests.len() - 1);
    }

    fn select_previous_pr(&mut self) {
        self.selected_pr = self.selected_pr.saturating_sub(1);
    }

    fn select_pr(&mut self, index: usize) {
        if index >= self.pull_requests.len() {
            return;
        }
        self.focus = FocusPane::PullRequests;
        self.selected_pr = index;
        if let Some(pr) = self.pull_requests.get(index) {
            self.status = format!("Selected PR #{}; press enter to load", pr.id);
        }
    }

    fn select_next_diff_file(&mut self) {
        let file_count = diff_file_count(self.diff.as_deref());
        if file_count == 0 {
            self.selected_diff_file = 0;
            return;
        }
        let previous = self.selected_diff_file;
        self.selected_diff_file = (self.selected_diff_file + 1).min(file_count - 1);
        if self.selected_diff_file != previous {
            self.diff_scroll = 0;
            self.rendered_diff = None;
            self.image_diff = None;
            self.status = "Selected next diff file".to_string();
        }
    }

    fn select_previous_diff_file(&mut self) {
        let previous = self.selected_diff_file;
        self.selected_diff_file = self.selected_diff_file.saturating_sub(1);
        if self.selected_diff_file != previous {
            self.diff_scroll = 0;
            self.rendered_diff = None;
            self.image_diff = None;
            self.status = "Selected previous diff file".to_string();
        }
    }

    fn select_diff_file(&mut self, index: usize) {
        if index >= diff_file_count(self.diff.as_deref()) {
            return;
        }
        self.focus = FocusPane::Diff;
        self.selected_diff_file = index;
        self.diff_scroll = 0;
        self.rendered_diff = None;
        self.image_diff = None;
        self.status = "Selected diff file".to_string();
    }

    fn cycle_pr_filter(&mut self) {
        self.set_pr_filter(self.pr_filter.next());
    }

    fn set_pr_filter(&mut self, filter: PrListFilter) {
        if self.pr_filter == filter {
            self.status = format!("Showing {} PRs", self.pr_filter.label());
            return;
        }
        self.pr_filter = filter;
        self.selected_pr = 0;
        self.load_selected_repo();
    }

    fn load_selected_repo(&mut self) {
        let Some(repo) = self.repos.get(self.selected_repo).cloned() else {
            self.pr_list_load = LoadState::Idle;
            self.status = "No repositories configured".to_string();
            return;
        };
        self.clear_pr_context_for_repo_load();
        let provider = repo.provider;
        let workspace = repo.workspace.clone();
        let repo_name = repo.repo.clone();
        let request_id = self.next_request();
        self.repo_request_id = request_id;
        self.pr_list_load = LoadState::Loading;
        self.status = format!(
            "Loading {} PRs for {workspace}/{repo_name}...",
            self.pr_filter.label()
        );
        self.error = None;
        self.loader.pull_requests(
            request_id,
            provider,
            workspace,
            repo_name,
            self.pr_filter.provider_state().to_string(),
        );
    }

    fn clear_pr_context_for_repo_load(&mut self) {
        self.pr_request_id = 0;
        self.ai_request_id = 0;
        self.pull_requests.clear();
        self.selected_pr = 0;
        self.ai_reviewed_pr_ids.clear();
        self.ai_review_running_pr_ids.clear();
        self.marker_generation = self.marker_generation.wrapping_add(1);
        self.marker_mutations.clear();
        self.detail = None;
        self.comments.clear();
        self.diff = None;
        self.active_ai_target = None;
        self.ai_review_state = None;
        self.ai_review_output = None;
        self.drafts.clear();
        self.composer = None;
        self.detail_view = DetailView::PullRequest;
        self.detail_load = LoadState::Idle;
        self.comments_load = LoadState::Idle;
        self.diff_load = LoadState::Idle;
        self.ai_review_load = LoadState::Idle;
        self.reset_detail_scrolls();
        self.reset_diff_state();
    }

    fn load_selected_pr(&mut self) {
        self.load_selected_pr_for_view(DetailView::PullRequest);
    }

    fn load_selected_pr_for_view(&mut self, target_view: DetailView) {
        if !matches!(self.pr_list_load, LoadState::Ready) {
            self.status = "Wait for the pull request list to load".to_string();
            return;
        }
        let Some(repo) = self.repos.get(self.selected_repo) else {
            return;
        };
        let Some(pr) = self.pull_requests.get(self.selected_pr) else {
            self.detail = None;
            self.comments.clear();
            self.diff = None;
            self.drafts.clear();
            self.composer = None;
            return;
        };
        let provider = repo.provider;
        let workspace = repo.workspace.clone();
        let repo_name = repo.repo.clone();
        let pr_id = pr.id;
        let target = (workspace.clone(), repo_name.clone(), pr_id);
        if self.pr_load_in_flight_for(&target) {
            self.status = "The selected pull request is already loading".to_string();
            return;
        }
        let target_changed = self.active_ai_target.as_ref() != Some(&target);
        if target_changed {
            self.detail = None;
            self.comments.clear();
            self.diff = None;
            self.ai_review_state = None;
            self.ai_review_output = None;
            self.reset_diff_state();
        }
        let request_id = self.next_request();
        let ai_request_id = self.next_request();
        self.pr_request_id = request_id;
        self.ai_request_id = ai_request_id;
        self.detail_load = LoadState::Loading;
        self.comments_load = LoadState::Loading;
        self.diff_load = LoadState::Loading;
        self.ai_review_load = LoadState::Loading;
        self.status = format!("Loading PR #{pr_id}...");
        self.error = None;
        self.drafts.clear();
        self.composer = None;
        self.active_ai_target = Some((workspace.clone(), repo_name.clone(), pr_id));
        self.detail_view = target_view;
        self.reset_detail_scrolls();
        self.loader.pull_request_resources(
            request_id,
            ai_request_id,
            provider,
            workspace,
            repo_name,
            pr_id,
            self.ai_review_store.clone(),
        );
    }

    fn pr_load_in_flight_for(&self, target: &(String, String, u32)) -> bool {
        self.active_ai_target.as_ref() == Some(target)
            && [&self.detail_load, &self.comments_load, &self.diff_load]
                .into_iter()
                .any(LoadState::is_loading)
    }

    fn start_comment_composer(&mut self) {
        if !matches!(self.pr_list_load, LoadState::Ready)
            || !matches!(self.detail_load, LoadState::Ready)
        {
            self.status = "Wait for the selected pull request to load".to_string();
            return;
        }
        if self.selected_review_target().is_none() {
            self.status = "Load the selected pull request before drafting a comment".to_string();
            return;
        }
        self.composer = Some(String::new());
        self.status = "Composing general review comment".to_string();
    }

    fn stage_composer_comment(&mut self) {
        let Some(raw) = self.composer.take() else {
            return;
        };
        let raw = raw.trim().to_string();
        if raw.is_empty() {
            self.status = "Empty comment discarded".to_string();
            return;
        }
        let id = self.next_draft_id;
        self.next_draft_id += 1;
        self.drafts.push(DraftComment { id, raw });
        self.status = format!("Staged {} draft comment(s)", self.drafts.len());
    }

    fn discard_drafts(&mut self) {
        let count = self.drafts.len();
        self.drafts.clear();
        self.composer = None;
        self.status = format!("Discarded {count} draft comment(s)");
    }

    fn publish_drafts(&mut self) {
        if self.comments_load.is_loading() {
            self.status = "Wait for comments to load before publishing drafts".to_string();
            return;
        }
        let Some((provider, workspace, repo, pr_id)) = self.selected_review_target() else {
            self.status = "Select a pull request before publishing drafts".to_string();
            return;
        };
        if self.drafts.is_empty() {
            self.status = "No draft comments to publish".to_string();
            return;
        }

        self.publish_drafts_with(|raw| {
            create_general_comment_native(
                Some(provider),
                workspace.as_str(),
                repo.as_str(),
                pr_id,
                raw,
                None,
            )
        });
    }

    fn start_ai_review(&mut self) {
        let Some((provider, workspace, repo, pr_id)) = self.selected_review_target() else {
            self.status = "Select a pull request before starting AI review".to_string();
            return;
        };
        let snapshot = match get_stable_pull_request_review_snapshot_native(
            Some(provider),
            &workspace,
            &repo,
            pr_id,
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.error = Some(error);
                self.status = "Failed to load stable PR snapshot for AI review".to_string();
                return;
            }
        };
        let detail = snapshot.detail;
        let diff = snapshot.diff;
        let title = detail.title.clone();
        let source_branch = detail.source_branch.clone();
        let destination_branch = detail.destination_branch.clone();
        let reviewed_base_sha = detail.destination_commit_hash.clone();
        let reviewed_head_sha = detail.source_commit_hash.clone();
        let prompt = match self.review_prompt_for_selected_repo() {
            Ok(prompt) => prompt,
            Err(error) => {
                self.error = Some(error);
                self.status = "Failed to load review prompt".to_string();
                return;
            }
        };
        let payload = build_review_payload(&prompt, &detail, &diff);
        self.detail = Some(detail);
        self.diff = Some(diff);
        match start_inline_review_native(
            self.ai_review_store.clone(),
            workspace.clone(),
            repo.clone(),
            pr_id,
            title,
            source_branch,
            destination_branch,
            reviewed_base_sha,
            reviewed_head_sha,
            payload,
            Some("Review this pull request from the terminal UI.".to_string()),
            None,
            Some("Review".to_string()),
            TUI_SKIP_AI_REVIEW_ANALYZERS,
            self.ai_provider,
            self.claude_model.clone(),
            self.claude_effort.clone(),
            self.codex_model.clone(),
            self.codex_effort.clone(),
            None,
        ) {
            Ok(state) => {
                self.ai_request_id = self.next_request();
                self.active_ai_target = Some((workspace, repo, pr_id));
                self.ai_review_state = Some(state);
                self.ai_review_output = None;
                self.ai_review_load = LoadState::Ready;
                self.mark_ai_review_running(pr_id);
                self.detail_view = DetailView::AiReview;
                self.ai_review_scroll = 0;
                self.error = None;
                self.status = format!("Started {} AI review", ai_provider_label(self.ai_provider));
            }
            Err(error) => {
                self.error = Some(error);
                self.status = "Failed to start AI review".to_string();
            }
        }
    }

    fn toggle_detail_view(&mut self) {
        self.detail_view = match self.detail_view {
            DetailView::PullRequest => DetailView::AiReview,
            DetailView::AiReview => DetailView::PullRequest,
            DetailView::Diff => DetailView::PullRequest,
        };
        if self.detail_view == DetailView::AiReview {
            if let Some((workspace, repo, pr_id)) = self.active_ai_target.clone() {
                let request_id = self.next_request();
                self.ai_request_id = request_id;
                self.ai_review_load = LoadState::Loading;
                self.loader.ai_review(
                    request_id,
                    workspace,
                    repo,
                    pr_id,
                    self.ai_review_store.clone(),
                );
            }
            self.status = "Showing AI review output".to_string();
        } else if self.detail_view == DetailView::Diff {
            self.load_selected_pr_for_view(DetailView::Diff);
        } else {
            self.status = "Showing pull request detail".to_string();
        }
    }

    fn scroll_active_detail(&mut self, delta: isize) {
        match self.detail_view {
            DetailView::PullRequest => {
                self.detail_scroll = self.detail_scroll.saturating_add_signed(delta);
                self.status = "Scrolled pull request detail".to_string();
            }
            DetailView::AiReview => {
                self.ai_review_scroll = self.ai_review_scroll.saturating_add_signed(delta);
                self.status = "Scrolled AI review output".to_string();
            }
            DetailView::Diff => {
                self.diff_scroll = self.diff_scroll.saturating_add_signed(delta);
                self.status = "Scrolled PR diff".to_string();
            }
        }
    }

    fn scroll_detail_at(&mut self, area: ratatui::layout::Rect, x: u16, y: u16, delta: isize) {
        let Some(view) = detail_view_target(area, x, y, self.detail_view) else {
            return;
        };
        self.detail_view = view;
        self.scroll_active_detail(delta);
    }

    fn reset_active_detail_scroll(&mut self) {
        match self.detail_view {
            DetailView::PullRequest => {
                self.detail_scroll = 0;
                self.status = "Reset pull request detail scroll".to_string();
            }
            DetailView::AiReview => {
                self.ai_review_scroll = 0;
                self.status = "Reset AI review scroll".to_string();
            }
            DetailView::Diff => {
                self.diff_scroll = 0;
                self.status = "Reset PR diff scroll".to_string();
            }
        }
    }

    fn reset_detail_scrolls(&mut self) {
        self.detail_scroll = 0;
        self.ai_review_scroll = 0;
        self.diff_scroll = 0;
    }

    fn refresh_active_view(&mut self) {
        if self.selected_pull_request_id().is_some() {
            self.load_selected_pr_for_view(self.detail_view);
        } else {
            self.load_selected_repo();
        }
    }

    fn reset_diff_state(&mut self) {
        self.diff_scroll = 0;
        self.selected_diff_file = 0;
        self.rendered_diff = None;
        self.image_diff = None;
    }

    fn toggle_diff_view_mode(&mut self) {
        self.diff_view_mode = self.diff_view_mode.next();
        self.diff_scroll = 0;
        self.rendered_diff = None;
        self.status = format!("Diff view: {}", self.diff_view_mode.label());
    }

    fn toggle_image_side(&mut self) {
        let Some(image) = self.image_diff.as_mut() else {
            self.status = "Selected diff is not a supported image".to_string();
            return;
        };
        if image.toggle_side() {
            self.diff_scroll = 0;
            self.status = format!("Image version: {}", image.selected_side.label());
        } else {
            self.status = format!(
                "Only the {} image version is available",
                image.selected_side.label()
            );
        }
    }

    fn refresh_selected_image_diff(&mut self) {
        if self
            .image_diff
            .as_ref()
            .is_some_and(|image| image.selected_file == self.selected_diff_file)
        {
            return;
        }
        let Some(patch) = selected_diff_file_patch(self.diff.as_deref(), self.selected_diff_file)
        else {
            self.image_diff = None;
            return;
        };
        let Some(candidate) = image_candidate_from_patch(&patch) else {
            self.image_diff = None;
            return;
        };
        let Some((provider, workspace, repo, pr_id)) = self.selected_review_target() else {
            self.image_diff = None;
            return;
        };
        self.image_diff = Some(ImageDiffState::load(
            self.selected_diff_file,
            candidate,
            |side, path| {
                get_pr_file_preview_native(
                    Some(provider),
                    &workspace,
                    &repo,
                    pr_id,
                    path,
                    side.provider_value(),
                )
            },
        ));
    }

    fn prepare_rendered_diff(&mut self, area: ratatui::layout::Rect) {
        if self.detail_view != DetailView::Diff {
            return;
        }
        self.refresh_selected_image_diff();
        if let Some(image) = self.image_diff.as_mut() {
            self.rendered_diff = None;
            let image_area = diff_image_area_for_area(area, self.detail.is_some());
            if let Err(error) = image.prepare_protocol(&self.image_support, image_area) {
                self.error = Some(error);
            }
            return;
        }
        let width = diff_content_width_for_area(area);
        let Some(patch) = selected_diff_file_patch(self.diff.as_deref(), self.selected_diff_file)
        else {
            self.rendered_diff = None;
            return;
        };
        let patch_hash = stable_hash(patch.as_str());
        if self.rendered_diff.as_ref().is_some_and(|cache| {
            cache.selected_file == self.selected_diff_file
                && cache.mode == self.diff_view_mode
                && cache.width == width
                && cache.patch_hash == patch_hash
        }) {
            return;
        }

        self.rendered_diff = Some(RenderedDiffCache {
            selected_file: self.selected_diff_file,
            mode: self.diff_view_mode,
            width,
            patch_hash,
            output: render_diff_with_delta(patch.as_str(), width, self.diff_view_mode),
        });
    }

    fn prompt_or_toggle_diff(&mut self) {
        if self.detail_view == DetailView::Diff {
            self.detail_view = DetailView::PullRequest;
            self.focus = FocusPane::PullRequests;
            self.status = "Closed PR diff".to_string();
            return;
        }
        if self.repos.is_empty() || self.pull_requests.is_empty() {
            self.status = "Select a pull request first".to_string();
            return;
        }
        self.diff_prompt_open = true;
        self.status = "Open diff: [g] Native TUI · [b] Browser · [Esc] Cancel".to_string();
    }

    fn open_browser_diff(&mut self) {
        let Some(web_state) = self.selected_web_diff_state() else {
            self.status = "Select a pull request first".to_string();
            return;
        };
        let pr_id = web_state.pr_id;

        if self.web_diff_server.is_none() {
            match WebDiffServer::start(Arc::clone(&self.web_diff_state)) {
                Ok(server) => self.web_diff_server = Some(server),
                Err(error) => {
                    self.status = format!("Failed to start diff server: {error}");
                    return;
                }
            }
        }

        if let Some(server) = self.web_diff_server.as_ref() {
            server.update_pr(web_state);
            let url = server.url();
            match open_browser_url(&url) {
                Ok(()) => {
                    self.status = format!("Opened PR #{pr_id} diff in browser");
                }
                Err(error) => {
                    self.status =
                        format!("Browser open failed: {error}. Open the diff manually at {url}");
                }
            }
        }
    }

    fn selected_web_diff_state(&self) -> Option<WebDiffState> {
        let repo = self.repos.get(self.selected_repo)?;
        let pr = self.pull_requests.get(self.selected_pr)?;
        let target = (repo.workspace.clone(), repo.repo.clone(), pr.id);
        let selected_pr_is_loaded = self.active_ai_target.as_ref() == Some(&target);
        let detail = if selected_pr_is_loaded {
            self.detail.as_ref()
        } else {
            None
        };

        Some(WebDiffState {
            version: 0,
            provider: Some(repo.provider),
            workspace: repo.workspace.clone(),
            repo: repo.repo.clone(),
            pr_id: pr.id,
            pr_title: detail
                .map(|loaded| loaded.title.clone())
                .unwrap_or_else(|| pr.title.clone()),
            pr_author: detail
                .map(|loaded| loaded.author_display_name.clone())
                .unwrap_or_else(|| pr.author_display_name.clone()),
            source_branch: detail
                .map(|loaded| loaded.source_branch.clone())
                .unwrap_or_default(),
            target_branch: detail
                .map(|loaded| loaded.destination_branch.clone())
                .unwrap_or_default(),
            diff: if selected_pr_is_loaded {
                self.diff.clone()
            } else {
                None
            },
            diffstat: None,
            population_failed: false,
        })
    }

    fn open_diff_view(&mut self) {
        if self.detail_view == DetailView::Diff {
            self.detail_view = DetailView::PullRequest;
            self.focus = FocusPane::PullRequests;
            self.status = "Closed PR diff".to_string();
            return;
        }
        self.detail_view = DetailView::Diff;
        self.focus = FocusPane::Diff;
        self.load_selected_pr_for_view(DetailView::Diff);
    }

    fn copy_ai_review_output(&mut self) {
        self.copy_loaded_ai_review_output_with(|output| {
            terminal::copy_to_clipboard(output).map_err(|error| error.to_string())
        });
    }

    fn copy_loaded_ai_review_output_with(
        &mut self,
        copier: impl FnOnce(&str) -> Result<(), String>,
    ) {
        if !matches!(self.ai_review_load, LoadState::Ready) {
            self.status = "Wait for the selected AI review to load".to_string();
            return;
        }
        let Some(output) = self
            .ai_review_output
            .as_deref()
            .map(str::trim)
            .filter(|output| !output.is_empty())
        else {
            self.status = "No AI review output to copy".to_string();
            return;
        };
        match copier(output) {
            Ok(()) => {
                self.status = "Copied AI review output".to_string();
                self.error = None;
            }
            Err(error) => {
                self.status = "Failed to copy AI review output".to_string();
                self.error = Some(error);
            }
        }
    }

    fn review_prompt_for_selected_repo(&self) -> Result<String, String> {
        let Some(repo) = self.repos.get(self.selected_repo) else {
            return Ok(default_review_prompt());
        };
        let Some(local_path) = repo
            .local_path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
        else {
            return Ok(default_review_prompt());
        };
        let result = validate_repo_review_config_native(std::path::Path::new(local_path), None)?;
        if !result.errors.is_empty() {
            return Err(result
                .errors
                .into_iter()
                .map(|error| error.message)
                .collect::<Vec<_>>()
                .join("\n"));
        }
        let prompt = result
            .config
            .and_then(|config| config.review)
            .and_then(|review| review.prompt)
            .unwrap_or_default();
        let replacement = prompt
            .replace
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let extension = prompt
            .extend
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let base_prompt = replacement.unwrap_or_else(default_review_prompt);
        Ok(match extension {
            Some(extension) => {
                format!("{base_prompt}\n\n## Repository review policy\n{extension}")
            }
            None => base_prompt,
        })
    }

    fn mark_ai_reviewed(&mut self, pr_id: u32) {
        if !self.ai_reviewed_pr_ids.contains(&pr_id) {
            self.ai_reviewed_pr_ids.push(pr_id);
            self.record_marker_mutation(pr_id);
        }
    }

    fn mark_ai_review_running(&mut self, pr_id: u32) {
        if !self.ai_review_running_pr_ids.contains(&pr_id) {
            self.ai_review_running_pr_ids.push(pr_id);
            self.record_marker_mutation(pr_id);
        }
    }

    fn unmark_ai_review_running(&mut self, pr_id: u32) {
        let previous_len = self.ai_review_running_pr_ids.len();
        self.ai_review_running_pr_ids.retain(|id| *id != pr_id);
        if self.ai_review_running_pr_ids.len() != previous_len {
            self.record_marker_mutation(pr_id);
        }
    }

    fn record_marker_mutation(&mut self, pr_id: u32) {
        self.marker_generation = self.marker_generation.wrapping_add(1);
        self.marker_mutations.insert(pr_id, self.marker_generation);
    }

    fn apply_marker_snapshot(
        &mut self,
        snapshot_generation: u64,
        mut reviewed: Vec<u32>,
        mut running: Vec<u32>,
    ) {
        for (&pr_id, &mutation_generation) in &self.marker_mutations {
            if mutation_generation <= snapshot_generation {
                continue;
            }
            Self::set_marker_membership(
                &mut reviewed,
                pr_id,
                self.ai_reviewed_pr_ids.contains(&pr_id),
            );
            Self::set_marker_membership(
                &mut running,
                pr_id,
                self.ai_review_running_pr_ids.contains(&pr_id),
            );
        }
        self.ai_reviewed_pr_ids = reviewed;
        self.ai_review_running_pr_ids = running;
        self.marker_mutations
            .retain(|_, generation| *generation > snapshot_generation);
    }

    fn set_marker_membership(markers: &mut Vec<u32>, pr_id: u32, present: bool) {
        if present {
            if !markers.contains(&pr_id) {
                markers.push(pr_id);
            }
        } else {
            markers.retain(|id| *id != pr_id);
        }
    }

    fn publish_drafts_with(
        &mut self,
        mut publisher: impl FnMut(String) -> Result<PrComment, String>,
    ) {
        let drafts = std::mem::take(&mut self.drafts);
        let mut unpublished = Vec::new();
        let mut published = 0usize;
        for draft in drafts {
            match publisher(draft.raw.clone()) {
                Ok(comment) => {
                    self.comments.push(comment);
                    published += 1;
                }
                Err(error) => {
                    self.error = Some(format!("Publish failed for draft #{}: {error}", draft.id));
                    unpublished.push(draft);
                }
            }
        }

        self.drafts = unpublished;
        if self.drafts.is_empty() {
            self.status = format!("Published {published} draft comment(s)");
            self.error = None;
        } else {
            self.status = format!(
                "Published {published}; {} draft comment(s) still pending",
                self.drafts.len()
            );
        }
    }

    fn selected_pull_request_id(&self) -> Option<u32> {
        self.pull_requests.get(self.selected_pr).map(|pr| pr.id)
    }

    fn selected_review_target(
        &self,
    ) -> Option<(crate::config::ReviewProvider, String, String, u32)> {
        if !matches!(self.pr_list_load, LoadState::Ready)
            || !matches!(self.detail_load, LoadState::Ready)
        {
            return None;
        }
        let repo = self.repos.get(self.selected_repo)?;
        let pr_id = self.selected_pull_request_id()?;
        if self.detail.as_ref().map(|detail| detail.id) != Some(pr_id) {
            return None;
        }
        Some((
            repo.provider,
            repo.workspace.clone(),
            repo.repo.clone(),
            pr_id,
        ))
    }

    fn view_state(&self) -> TuiState<'_> {
        TuiState {
            repos: &self.repos,
            selected_repo: self.selected_repo,
            focus: self.focus,
            pull_requests: &self.pull_requests,
            pr_filter: self.pr_filter,
            selected_pr: self.selected_pr,
            detail: self.detail.as_ref(),
            comments: &self.comments,
            ai_reviewed_pr_ids: &self.ai_reviewed_pr_ids,
            ai_review_running_pr_ids: &self.ai_review_running_pr_ids,
            diff: self.diff.as_deref(),
            drafts: &self.drafts,
            composer: self.composer.as_deref(),
            ai_review: self.ai_review_state.as_ref(),
            ai_review_output: self.ai_review_output.as_deref(),
            detail_view: self.detail_view,
            detail_scroll: self.detail_scroll,
            ai_review_scroll: self.ai_review_scroll,
            diff_scroll: self.diff_scroll,
            selected_diff_file: self.selected_diff_file,
            diff_view_mode: self.diff_view_mode,
            rendered_diff_output: self.rendered_diff.as_ref().and_then(|cache| {
                if cache.selected_file == self.selected_diff_file
                    && cache.mode == self.diff_view_mode
                    && cache
                        .output
                        .as_deref()
                        .is_some_and(|output| !output.trim().is_empty())
                {
                    cache.output.as_deref()
                } else {
                    None
                }
            }),
            image_diff: self.image_diff.as_ref(),
            image_protocol: self.image_support.label(),
            loading: LoadingView {
                repo: &self.repo_load,
                pull_requests: &self.pr_list_load,
                detail: &self.detail_load,
                comments: &self.comments_load,
                diff: &self.diff_load,
                ai_review: &self.ai_review_load,
                tick: self.spinner_tick,
            },
            error: self.error.as_deref(),
            status: self.status.as_str(),
            diff_prompt_open: self.diff_prompt_open,
        }
    }
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn render_diff_with_delta(patch: &str, width: usize, mode: DiffViewMode) -> Option<String> {
    let mut command = Command::new("delta");
    command
        .arg("--dark")
        .arg("--paging=never")
        .arg("--line-numbers")
        .arg("--file-style=bold brightwhite")
        .arg("--file-decoration-style=brightblack ul")
        .arg("--width")
        .arg(width.max(20).to_string());
    if mode == DiffViewMode::Split {
        command.arg("--side-by-side");
    }
    command.stdin(Stdio::piped()).stdout(Stdio::piped());

    let mut child = command.spawn().ok()?;
    child.stdin.as_mut()?.write_all(patch.as_bytes()).ok()?;
    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }
    let rendered = String::from_utf8_lossy(&output.stdout).to_string();
    (!rendered.trim().is_empty()).then_some(rendered)
}

fn build_review_payload(prompt: &str, detail: &PullRequestDetail, diff: &str) -> String {
    let author = detail.author_display_name.trim();
    let author = if author.is_empty() { "unknown" } else { author };
    let mut lines = vec![
        prompt.trim().to_string(),
        String::new(),
        "## Pull request".to_string(),
        format!("{} (#{})", detail.title, detail.id),
        format!("Author: {author}"),
        format!(
            "Branch: {} -> {}",
            detail.source_branch, detail.destination_branch
        ),
    ];
    if !detail.description_raw.trim().is_empty() {
        lines.extend([
            String::new(),
            "## Description".to_string(),
            detail.description_raw.trim().to_string(),
        ]);
    }
    lines.extend([
        String::new(),
        "## Diff".to_string(),
        "```diff".to_string(),
        diff.trim().to_string(),
        "```".to_string(),
    ]);
    lines.join("\n")
}

fn ai_provider_label(provider: AiProvider) -> &'static str {
    match provider {
        AiProvider::Claude => "Claude",
        AiProvider::Codex => "Codex",
    }
}

fn default_review_prompt() -> String {
    DEFAULT_REVIEW_PROMPT.trim().to_string()
}

fn diff_file_count(diff: Option<&str>) -> usize {
    let count = diff
        .unwrap_or_default()
        .lines()
        .filter(|line| line.starts_with("diff --git "))
        .count();
    if count == 0 && diff.unwrap_or_default().trim().is_empty() {
        0
    } else {
        count.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReviewProvider;
    use ratatui::{backend::TestBackend, Terminal};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn repo(workspace: &str, repo: &str) -> RepoRef {
        RepoRef {
            provider: ReviewProvider::Github,
            workspace: workspace.to_string(),
            repo: repo.to_string(),
            local_path: None,
        }
    }

    fn pr(id: u32, title: &str) -> PullRequestSummary {
        PullRequestSummary {
            id,
            title: title.to_string(),
            author_display_name: String::new(),
            author_account_id: None,
            source_branch: format!("feature/{id}"),
            destination_branch: "main".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            comment_count: 0,
            created_on: String::new(),
            updated_on: String::new(),
            reviewers: Vec::new(),
        }
    }

    fn detail(id: u32, title: &str) -> PullRequestDetail {
        PullRequestDetail {
            id,
            title: title.to_string(),
            description_raw: "Review this.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: String::new(),
            reviewers: Vec::new(),
            source_branch: format!("feature/{id}"),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        }
    }

    #[test]
    fn settings_are_discoverable_and_accept_custom_model_values() {
        let mut app = TuiApp::from_repos(Vec::new());

        app.handle_key(KeyCode::Char('s'));
        assert!(app.settings_open);
        app.settings_field = SettingsField::CodexModel;
        app.handle_key(KeyCode::Char('e'));
        for character in "custom-model".chars() {
            app.handle_key(KeyCode::Char(character));
        }
        app.handle_key(KeyCode::Enter);

        assert_eq!(app.settings_codex_model.as_deref(), Some("custom-model"));
        assert!(app.settings_editor.is_none());
        assert!(app.status.contains("press s to save"));
    }

    #[test]
    fn custom_model_values_survive_open_and_confirm_without_edits() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::ClaudeModel;
        app.settings_claude_model = Some("future-provider-model".to_string());

        app.handle_key(KeyCode::Char('e'));
        assert_eq!(
            app.settings_editor.as_ref().map(SettingsEditor::value),
            Some("future-provider-model")
        );
        app.handle_key(KeyCode::Enter);

        assert_eq!(
            app.settings_claude_model.as_deref(),
            Some("future-provider-model")
        );
    }

    #[test]
    fn settings_render_masks_provider_tokens() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_editor = Some(SettingsEditor::GithubToken {
            token: Zeroizing::new("SECRET_DO_NOT_RENDER".to_string()),
        });
        let backend = TestBackend::new(120, 28);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| render_settings(frame, &app))
            .expect("draw settings");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(!rendered.contains("SECRET_DO_NOT_RENDER"));
        assert!(rendered.contains(&mask_secret("SECRET_DO_NOT_RENDER")));
    }

    #[test]
    fn settings_render_sections_and_contextual_credential_actions() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::GithubCredential;
        app.github_credential_status = CredentialStatus {
            provider: CredentialProvider::Github,
            available: false,
            source: CredentialSource::None,
        };
        let backend = TestBackend::new(120, 28);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| render_settings(frame, &app))
            .expect("draw settings");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("AI review"));
        assert!(rendered.contains("Provider credentials"));
        assert!(rendered.contains("CLI readiness"));
        assert!(rendered.contains("Not configured"));
        assert!(rendered.contains("Enter configure"));
        assert!(rendered.contains("token input stays masked"));
    }

    #[test]
    fn settings_paste_filters_control_characters_and_keeps_secret_masked() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::GithubCredential;
        app.open_settings_editor();

        app.handle_paste("TEST_SECRET\nVALUE\r");

        assert_eq!(
            app.settings_editor.as_ref().map(SettingsEditor::value),
            Some("TEST_SECRETVALUE")
        );
        assert!(app.status.contains("secret remains masked"));

        let backend = TestBackend::new(120, 28);
        let mut terminal = Terminal::new(backend).expect("terminal");
        terminal
            .draw(|frame| render_settings(frame, &app))
            .expect("draw settings");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(!rendered.contains("TEST_SECRETVALUE"));
        assert!(rendered.contains(&mask_secret("TEST_SECRETVALUE")));
    }

    #[test]
    fn settings_reject_oversized_paste_without_partial_secret_input() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::GithubCredential;
        app.open_settings_editor();
        let oversized = "x".repeat(credentials::MAX_PROVIDER_TOKEN_BYTES + 1);

        app.handle_paste(&oversized);

        assert_eq!(
            app.settings_editor.as_ref().map(SettingsEditor::value),
            Some("")
        );
        assert!(app.status.contains("exceeds the input size limit"));
        assert!(!app.status.contains(&oversized));
    }

    #[test]
    fn settings_report_when_typed_input_reaches_the_size_limit() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::GithubCredential;
        app.open_settings_editor();
        let maximum = "x".repeat(credentials::MAX_PROVIDER_TOKEN_BYTES);
        app.handle_paste(&maximum);

        app.handle_key(KeyCode::Char('y'));

        assert_eq!(
            app.settings_editor
                .as_ref()
                .map(SettingsEditor::value)
                .map(str::len),
            Some(credentials::MAX_PROVIDER_TOKEN_BYTES)
        );
        assert!(app.status.contains("reached the input size limit"));
        assert!(!app.status.contains(&maximum));
    }

    #[test]
    fn bitbucket_token_escape_returns_to_the_username_step() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::BitbucketCredential;
        app.open_settings_editor();
        app.handle_paste("reviewer@example.test");
        app.handle_key(KeyCode::Enter);
        app.handle_paste("TEST_SECRET_VALUE");

        app.handle_key(KeyCode::Esc);

        assert!(matches!(
            app.settings_editor.as_ref(),
            Some(SettingsEditor::BitbucketUsername { username })
                if username == "reviewer@example.test"
        ));
        assert_eq!(app.status, "Back to Bitbucket username");
    }

    #[test]
    fn settings_do_not_offer_to_remove_environment_credentials() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::GithubCredential;
        app.github_credential_status = CredentialStatus {
            provider: CredentialProvider::Github,
            available: true,
            source: CredentialSource::Environment,
        };

        app.handle_key(KeyCode::Char('d'));

        assert_eq!(
            app.status,
            "Environment credentials cannot be removed from Norn"
        );
    }

    #[test]
    fn settings_cancel_keeps_active_non_secret_values_unchanged() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.ai_provider = AiProvider::Claude;
        app.open_settings();
        app.settings_ai_provider = AiProvider::Codex;

        app.handle_key(KeyCode::Esc);

        assert!(!app.settings_open);
        assert_eq!(app.ai_provider, AiProvider::Claude);
        assert_eq!(app.status, "Settings cancelled");
    }

    #[test]
    fn settings_remain_usable_on_a_narrow_terminal() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.claude_cli_available = true;
        app.codex_cli_available = false;
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| render_settings(frame, &app))
            .expect("draw settings");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Norn settings"));
        assert!(rendered.contains("GitHub credential"));
        assert!(rendered.contains("Claude Code CLI"));
        assert!(rendered.contains("Available"));
        assert!(rendered.contains("Status"));
        assert!(rendered.contains("Ready"));
    }

    #[test]
    fn credential_editor_remains_explicit_on_a_narrow_terminal() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.settings_open = true;
        app.settings_field = SettingsField::GithubCredential;
        app.open_settings_editor();
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| render_settings(frame, &app))
            .expect("draw settings");
        let rendered = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();

        assert!(rendered.contains("Configure GitHub"));
        assert!(rendered.contains("Personal access token"));
        assert!(rendered.contains("input stays hidden"));
        assert!(rendered.contains("OS keychain"));
        assert!(rendered.contains("Enter"));
        assert!(!rendered.contains("TEST_SECRET"));
    }

    #[test]
    fn credential_status_labels_reveal_only_source_state() {
        let label = credential_status_label(CredentialStatus {
            provider: CredentialProvider::Github,
            available: true,
            source: CredentialSource::Keychain,
        });
        assert_eq!(label, "Configured · OS keychain");
        assert!(!label.contains('/'));
    }

    fn temp_repo_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("norn-tui-{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn tui_launch_defaults_to_current_repository() {
        assert_eq!(
            launch_mode_from_args(Vec::<String>::new()).unwrap().mode,
            TuiLaunchMode::CurrentRepo
        );
        assert_eq!(
            launch_mode_from_args(["--workspace"]).unwrap().mode,
            TuiLaunchMode::Workspace
        );
    }

    #[test]
    fn tui_launch_supports_skipping_readiness_checks() {
        let options = launch_mode_from_args(["--workspace", "--skip-readiness"]).unwrap();
        assert_eq!(options.mode, TuiLaunchMode::Workspace);
        assert!(options.skip_readiness);
    }

    #[test]
    fn tui_launch_supports_version_query_flag() {
        assert_eq!(
            launch_mode_from_args(["--version"]).unwrap().mode,
            TuiLaunchMode::Version
        );
        assert_eq!(
            launch_mode_from_args(["-V"]).unwrap().mode,
            TuiLaunchMode::Version
        );
    }

    #[test]
    fn tui_launch_rejects_unknown_options() {
        let error = launch_mode_from_args(["--repo"]).unwrap_err();
        assert!(error.contains("Unknown option"));
    }

    #[test]
    fn tui_preflight_rejects_current_repo_without_git_root() {
        let repo = temp_repo_path("without-git");
        std::fs::create_dir_all(&repo).unwrap();

        let error = run_tui_preflight(true, &repo);
        assert!(error.is_err());
        assert_eq!(
            error.unwrap_err(),
            "Readiness preflight failed; complete machine/repository setup before starting the TUI."
        );
    }

    #[test]
    fn tui_preflight_rejects_current_repo_with_invalid_config() {
        let repo = temp_repo_path("autoinit-invalid-config");
        std::fs::create_dir_all(&repo).expect("create invalid-config repo");
        assert!(std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args(["init", "--initial-branch", "main"])
            .status()
            .expect("git init")
            .success());
        std::fs::write(
            repo.join(".norn.yaml"),
            r#"
version: 2.0
review:
  mode: fast
"#,
        )
        .expect("write invalid config");
        assert!(run_tui_preflight(true, &repo).is_err());

        let original = fs::read_to_string(repo.join(".norn.yaml")).expect("config preserved");
        assert!(original.contains("version: 2.0"));
    }

    #[test]
    fn tui_preflight_accepts_workspace_mode_without_git_root() {
        let repo = temp_repo_path("workspace-no-git");
        std::fs::create_dir_all(&repo).unwrap();

        assert!(run_tui_preflight(false, &repo).is_ok());
    }

    #[test]
    fn tui_preflight_autocreates_default_repo_config_for_current_repo() {
        let repo = temp_repo_path("autoinit");
        std::fs::create_dir_all(&repo).unwrap();
        assert!(std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args(["init", "--initial-branch", "main"])
            .status()
            .expect("git init")
            .success());
        std::fs::write(repo.join("README.md"), "repo\n").expect("write readme");
        assert!(std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args(["add", "README.md"])
            .status()
            .expect("git add")
            .success());
        assert!(std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "-c",
                "user.email=ci@example.com",
                "-c",
                "user.name=CI",
                "commit",
                "-m",
                "init",
            ])
            .status()
            .expect("git commit")
            .success());
        assert!(std::process::Command::new("/usr/bin/git")
            .arg("-C")
            .arg(&repo)
            .args([
                "remote",
                "add",
                "origin",
                "https://github.com/example/repo.git",
            ])
            .status()
            .expect("git remote add")
            .success());

        assert!(run_tui_preflight(true, &repo).is_ok());
        let config = std::fs::read_to_string(repo.join(".norn.yaml")).expect("default config");
        assert!(config.contains("version: 0.1"));
        assert!(config.contains("review:\n  mode: balanced"));
    }

    #[test]
    fn initial_repository_resolution_renders_before_the_worker_finishes() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.repo_load = LoadState::Loading;
        app.status = "Resolving repository...".to_string();
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| render(frame, app.view_state()))
            .expect("draw loading frame");

        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Resolving repository"));
        assert!(text.contains("q quit"));
    }

    #[test]
    fn stale_repository_results_cannot_replace_the_current_selection() {
        let mut app = TuiApp::from_repos(vec![repo("current", "repo")]);
        app.repo_request_id = 2;
        app.pull_requests = vec![pr(7, "Current")];

        app.apply_load_event(LoadEvent::PullRequests {
            request_id: 1,
            result: Ok(vec![pr(99, "Stale")]),
        });

        assert_eq!(app.pull_requests.len(), 1);
        assert_eq!(app.pull_requests[0].id, 7);
    }

    #[test]
    fn repository_load_reset_discards_prs_and_markers_from_the_previous_target() {
        let mut app = TuiApp::from_repos(vec![repo("current", "repo")]);
        app.pull_requests = vec![pr(7, "Previous")];
        app.selected_pr = 0;
        app.ai_reviewed_pr_ids = vec![7];
        app.ai_review_running_pr_ids = vec![7];
        app.detail = Some(detail(7, "Previous"));
        app.detail_load = LoadState::Loading;
        app.comments_load = LoadState::Loading;
        app.diff_load = LoadState::Loading;

        app.clear_pr_context_for_repo_load();

        assert!(app.pull_requests.is_empty());
        assert!(app.ai_reviewed_pr_ids.is_empty());
        assert!(app.ai_review_running_pr_ids.is_empty());
        assert!(app.detail.is_none());
        assert!(matches!(app.detail_load, LoadState::Idle));
        assert!(matches!(app.comments_load, LoadState::Idle));
        assert!(matches!(app.diff_load, LoadState::Idle));
    }

    #[test]
    fn pr_load_rejects_a_list_that_is_not_ready() {
        let mut app = TuiApp::from_repos(vec![repo("current", "repo")]);
        app.pull_requests = vec![pr(7, "Stale")];
        app.pr_list_load = LoadState::Loading;

        app.load_selected_pr_for_view(DetailView::Diff);

        assert_eq!(app.pr_request_id, 0);
        assert_eq!(app.status, "Wait for the pull request list to load");
    }

    #[test]
    fn pr_load_deduplicates_while_resources_are_in_flight() {
        let mut app = TuiApp::from_repos(vec![repo("current", "repo")]);
        app.pull_requests = vec![pr(7, "Loading")];
        app.pr_list_load = LoadState::Ready;
        app.detail_load = LoadState::Loading;
        app.active_ai_target = Some(("current".to_string(), "repo".to_string(), 7));

        app.load_selected_pr_for_view(DetailView::PullRequest);

        assert_eq!(app.pr_request_id, 0);
        assert_eq!(app.status, "The selected pull request is already loading");
    }

    #[test]
    fn pr_load_allows_switching_away_from_an_in_flight_target() {
        let mut app = TuiApp::from_repos(vec![repo("current", "repo")]);
        app.pull_requests = vec![pr(7, "Loading"), pr(8, "Next")];
        app.selected_pr = 1;
        app.pr_list_load = LoadState::Ready;
        app.detail_load = LoadState::Loading;
        app.active_ai_target = Some(("current".to_string(), "repo".to_string(), 7));

        assert!(!app.pr_load_in_flight_for(&("current".to_string(), "repo".to_string(), 8)));
    }

    #[test]
    fn independent_resource_failures_keep_successful_pr_data_visible() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests = vec![pr(7, "Progressive loading")];
        app.pr_request_id = 12;
        app.detail_load = LoadState::Loading;
        app.comments_load = LoadState::Loading;
        app.diff_load = LoadState::Loading;
        app.ai_review_load = LoadState::Ready;

        app.apply_load_event(LoadEvent::Detail {
            request_id: 12,
            result: Ok(detail(7, "Progressive loading")),
        });
        app.apply_load_event(LoadEvent::Diff {
            request_id: 12,
            result: Err("provider timeout".to_string()),
        });

        assert_eq!(app.detail.as_ref().map(|detail| detail.id), Some(7));
        assert!(matches!(app.detail_load, LoadState::Ready));
        assert!(matches!(app.diff_load, LoadState::Failed(_)));
        assert!(app.comments_load.is_loading());
    }

    #[test]
    fn stale_ai_review_results_cannot_replace_a_newer_run() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.ai_request_id = 2;
        app.ai_review_state = Some(AiReviewRunState {
            status: AiReviewRunStatus::Running,
            ..AiReviewRunState::default()
        });

        app.apply_load_event(LoadEvent::AiReview {
            request_id: 1,
            pr_id: 7,
            state: Some(AiReviewRunState {
                status: AiReviewRunStatus::Succeeded,
                ..AiReviewRunState::default()
            }),
            output: Ok(Some("stale".to_string())),
        });

        assert_eq!(
            app.ai_review_state.as_ref().map(|state| state.status),
            Some(AiReviewRunStatus::Running)
        );
        assert!(app.ai_review_output.is_none());
    }

    #[test]
    fn ai_review_results_update_markers_for_the_event_target() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests = vec![pr(7, "Reviewed"), pr(8, "Highlighted")];
        app.selected_pr = 1;
        app.ai_request_id = 3;

        app.apply_load_event(LoadEvent::AiReview {
            request_id: 3,
            pr_id: 7,
            state: Some(AiReviewRunState {
                status: AiReviewRunStatus::Succeeded,
                ..AiReviewRunState::default()
            }),
            output: Ok(Some("review".to_string())),
        });

        assert_eq!(app.ai_reviewed_pr_ids, vec![7]);
        assert!(!app.ai_reviewed_pr_ids.contains(&8));
    }

    #[test]
    fn repo_selection_stays_in_bounds() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests.clear();

        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_repo, 0);

        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected_repo, 0);
    }

    #[test]
    fn mouse_selects_pull_request_rows() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests = vec![pr(1, "One"), pr(2, "Two")];

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 38,
                row: 5,
                modifiers: event::KeyModifiers::empty(),
            },
            ratatui::layout::Rect::new(0, 0, 100, 24),
        );

        assert_eq!(app.focus, FocusPane::PullRequests);
        assert_eq!(app.selected_pr, 1);
    }

    #[test]
    fn mouse_wheel_scrolls_only_detail_panes() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.detail_view = DetailView::AiReview;

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 2,
                row: 5,
                modifiers: event::KeyModifiers::empty(),
            },
            ratatui::layout::Rect::new(0, 0, 100, 24),
        );
        assert_eq!(app.ai_review_scroll, 0);

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 70,
                row: 15,
                modifiers: event::KeyModifiers::empty(),
            },
            ratatui::layout::Rect::new(0, 0, 100, 24),
        );
        assert_eq!(app.detail_view, DetailView::AiReview);
        assert_eq!(app.ai_review_scroll, 3);
    }

    #[test]
    fn diff_view_toggle_opens_and_closes_full_page() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests = vec![pr(1, "Test PR")];

        // First press 'g' opens prompt
        app.handle_key(KeyCode::Char('g'));
        assert!(app.diff_prompt_open);

        // Second press 'g' selects native diff
        app.handle_key(KeyCode::Char('g'));
        assert!(!app.diff_prompt_open);
        assert_eq!(app.detail_view, DetailView::Diff);
        assert_eq!(app.focus, FocusPane::Diff);

        // Press 'g' while in diff view toggles back to PR view
        app.handle_key(KeyCode::Char('g'));
        assert_eq!(app.detail_view, DetailView::PullRequest);
        assert_eq!(app.focus, FocusPane::PullRequests);
    }

    #[test]
    fn diff_choice_prompt_browser_selection_updates_web_state() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests = vec![pr(42, "Support web diff")];

        app.handle_key(KeyCode::Char('g'));
        assert!(app.diff_prompt_open);

        app.handle_key(KeyCode::Char('b'));
        assert!(!app.diff_prompt_open);

        let web_state = app.web_diff_state.read().unwrap();
        assert_eq!(web_state.pr_id, 42);
        assert_eq!(web_state.pr_title, "Support web diff");
    }

    #[test]
    fn browser_diff_does_not_reuse_loaded_diff_for_another_selected_pr() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests = vec![pr(41, "Loaded PR"), pr(42, "Selected PR")];
        app.active_ai_target = Some(("delaudio".to_string(), "norn".to_string(), 41));
        app.diff = Some("diff for PR 41".to_string());
        app.selected_pr = 1;

        let web_state = app.selected_web_diff_state().expect("selected PR state");

        assert_eq!(web_state.pr_id, 42);
        assert_eq!(web_state.pr_title, "Selected PR");
        assert_eq!(web_state.diff, None);
    }

    #[test]
    fn diff_choice_prompt_escape_cancels_prompt() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests = vec![pr(1, "Test PR")];

        app.handle_key(KeyCode::Char('g'));
        assert!(app.diff_prompt_open);

        app.handle_key(KeyCode::Esc);
        assert!(!app.diff_prompt_open);
        assert_eq!(app.status, "Diff view cancelled");
    }

    #[test]
    fn diff_view_selection_moves_between_files() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.detail_view = DetailView::Diff;
        app.focus = FocusPane::Diff;
        app.diff =
            Some("diff --git a/a.ts b/a.ts\n+one\ndiff --git a/b.ts b/b.ts\n+two\n".to_string());

        app.handle_key(KeyCode::Char('j'));
        assert_eq!(app.selected_diff_file, 1);
        assert_eq!(app.diff_scroll, 0);

        app.handle_key(KeyCode::Char('k'));
        assert_eq!(app.selected_diff_file, 0);
    }

    #[test]
    fn diff_view_mode_toggle_cycles_between_unified_and_split() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.diff_scroll = 12;

        app.handle_key(KeyCode::Char('u'));
        assert_eq!(app.diff_view_mode, DiffViewMode::Split);
        assert_eq!(app.diff_scroll, 0);
        assert_eq!(app.status, "Diff view: side-by-side");

        app.handle_key(KeyCode::Char('u'));
        assert_eq!(app.diff_view_mode, DiffViewMode::Unified);
        assert_eq!(app.status, "Diff view: unified");
    }

    #[test]
    fn running_review_marker_is_removed_when_review_finishes() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);

        app.mark_ai_review_running(7);
        assert_eq!(app.ai_review_running_pr_ids, vec![7]);

        app.mark_ai_reviewed(7);
        app.unmark_ai_review_running(7);

        assert!(app.ai_review_running_pr_ids.is_empty());
        assert_eq!(app.ai_reviewed_pr_ids, vec![7]);
    }

    #[test]
    fn stale_marker_snapshots_cannot_erase_newer_local_state() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.repo_request_id = 4;
        app.marker_generation = 1;
        app.mark_ai_review_running(7);

        app.apply_load_event(LoadEvent::ReviewMarkers {
            request_id: 4,
            marker_generation: 1,
            reviewed: vec![8],
            running: Vec::new(),
        });

        assert_eq!(app.ai_review_running_pr_ids, vec![7]);
        assert_eq!(app.ai_reviewed_pr_ids, vec![8]);
    }

    #[test]
    fn tui_ai_review_skips_duplicate_analyzers() {
        const { assert!(TUI_SKIP_AI_REVIEW_ANALYZERS) };
    }

    #[test]
    fn copies_loaded_ai_review_output_without_visible_wrapping() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.ai_review_output = Some("full markdown\nwith second line".to_string());
        app.ai_review_load = LoadState::Ready;
        let mut copied = String::new();

        app.copy_loaded_ai_review_output_with(|output| {
            copied = output.to_string();
            Ok(())
        });

        assert_eq!(copied, "full markdown\nwith second line");
        assert_eq!(app.status, "Copied AI review output");
        assert!(app.error.is_none());
    }

    #[test]
    fn copy_review_reports_missing_output() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.ai_review_load = LoadState::Ready;

        app.copy_loaded_ai_review_output_with(|_| Err("should not copy".to_string()));

        assert_eq!(app.status, "No AI review output to copy");
        assert!(app.error.is_none());
    }

    #[test]
    fn copy_review_rejects_output_from_the_previous_pr_while_loading() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.ai_review_output = Some("previous review".to_string());
        app.ai_review_load = LoadState::Loading;
        let mut copied = false;

        app.copy_loaded_ai_review_output_with(|_| {
            copied = true;
            Ok(())
        });

        assert!(!copied);
        assert_eq!(app.status, "Wait for the selected AI review to load");
    }

    #[test]
    fn quit_keys_mark_app_done() {
        let mut app = TuiApp::from_repos(Vec::new());

        app.handle_key(KeyCode::Char('q'));

        assert!(app.should_quit);
    }

    #[test]
    fn mouse_wheel_scrolls_diff_view_without_switching_to_ai_review() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.detail_view = DetailView::Diff;

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 70,
                row: 15,
                modifiers: event::KeyModifiers::empty(),
            },
            ratatui::layout::Rect::new(0, 0, 100, 24),
        );

        assert_eq!(app.detail_view, DetailView::Diff);
        assert_eq!(app.diff_scroll, 3);
        assert_eq!(app.ai_review_scroll, 0);
    }

    #[test]
    fn composer_stages_local_draft_without_publishing() {
        let mut app = TuiApp::from_repos(vec![repo("delaudio", "norn")]);
        app.pull_requests.push(PullRequestSummary {
            id: 7,
            title: "Draftable".to_string(),
            author_display_name: String::new(),
            author_account_id: None,
            source_branch: "feature".to_string(),
            destination_branch: "main".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            comment_count: 0,
            created_on: String::new(),
            updated_on: String::new(),
            reviewers: Vec::new(),
        });
        app.pr_list_load = LoadState::Ready;
        app.detail_load = LoadState::Ready;
        app.detail = Some(detail(7, "Draftable"));

        app.handle_key(KeyCode::Char('c'));
        app.handle_key(KeyCode::Char('n'));
        app.handle_key(KeyCode::Char('o'));
        app.handle_key(KeyCode::Char('t'));
        app.handle_key(KeyCode::Char('e'));
        app.handle_key(KeyCode::Enter);

        assert_eq!(app.drafts.len(), 1);
        assert_eq!(app.drafts[0].raw, "note");
        assert!(app.comments.is_empty());
    }

    #[test]
    fn publishing_waits_for_the_comment_snapshot() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.comments_load = LoadState::Loading;
        app.drafts.push(DraftComment {
            id: 1,
            raw: "pending".to_string(),
        });

        app.publish_drafts();

        assert_eq!(app.drafts.len(), 1);
        assert_eq!(
            app.status,
            "Wait for comments to load before publishing drafts"
        );
    }

    #[test]
    fn discard_drafts_keeps_remote_comments() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.drafts.push(DraftComment {
            id: 1,
            raw: "pending".to_string(),
        });
        app.comments.push(PrComment {
            id: "1".to_string(),
            parent_id: None,
            content_raw: "remote".to_string(),
            content_html: None,
            user_display_name: "reviewer".to_string(),
            created_on: String::new(),
            deleted: false,
            inline: None,
        });

        app.discard_drafts();

        assert!(app.drafts.is_empty());
        assert_eq!(app.comments.len(), 1);
    }

    #[test]
    fn partial_publish_keeps_failed_drafts_visible() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.drafts.push(DraftComment {
            id: 1,
            raw: "publish".to_string(),
        });
        app.drafts.push(DraftComment {
            id: 2,
            raw: "fail".to_string(),
        });

        app.publish_drafts_with(|raw| {
            if raw == "fail" {
                Err("remote rejected comment".to_string())
            } else {
                Ok(PrComment {
                    id: "10".to_string(),
                    parent_id: None,
                    content_raw: raw,
                    content_html: None,
                    user_display_name: "reviewer".to_string(),
                    created_on: String::new(),
                    deleted: false,
                    inline: None,
                })
            }
        });

        assert_eq!(app.comments.len(), 1);
        assert_eq!(app.drafts.len(), 1);
        assert_eq!(app.drafts[0].raw, "fail");
        assert!(app
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("draft #2"));
    }

    #[test]
    fn review_payload_includes_pr_context_and_diff() {
        let detail = detail(7, "Add GitHub TUI support");

        let payload =
            build_review_payload("Review carefully.", &detail, "diff --git a/a b/a\n+new");

        assert!(payload.contains("Review carefully."));
        assert!(payload.contains("Add GitHub TUI support (#7)"));
        assert!(payload.contains("Author: unknown"));
        assert!(payload.contains("Branch: feature/7 -> main"));
        assert!(payload.contains("```diff\ndiff --git a/a b/a\n+new\n```"));
    }

    #[test]
    fn draft_filter_uses_open_provider_state() {
        assert_eq!(PrListFilter::Open.provider_state(), "OPEN");
        assert_eq!(PrListFilter::Draft.provider_state(), "OPEN");
        assert_eq!(PrListFilter::Merged.provider_state(), "MERGED");
    }

    #[test]
    fn draft_filter_only_includes_draft_pull_requests() {
        let mut draft = pr(1, "Draft");
        draft.draft = true;
        let ready = pr(2, "Ready");

        assert!(PrListFilter::Draft.includes(&draft));
        assert!(!PrListFilter::Draft.includes(&ready));
        assert!(PrListFilter::Open.includes(&ready));
    }

    #[test]
    fn filter_key_cycles_pull_request_modes() {
        let mut app = TuiApp::from_repos(Vec::new());

        app.handle_key(KeyCode::Char('f'));
        assert_eq!(app.pr_filter, PrListFilter::Draft);

        app.handle_key(KeyCode::Char('f'));
        assert_eq!(app.pr_filter, PrListFilter::Merged);

        app.handle_key(KeyCode::Char('f'));
        assert_eq!(app.pr_filter, PrListFilter::Open);
    }

    #[test]
    fn mouse_click_sets_pull_request_filter() {
        let mut app = TuiApp::from_repos(Vec::new());

        app.handle_mouse(
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 14,
                row: 19,
                modifiers: event::KeyModifiers::empty(),
            },
            ratatui::layout::Rect::new(0, 0, 100, 24),
        );

        assert_eq!(app.pr_filter, PrListFilter::Draft);
    }

    #[test]
    fn lachesi_folder_prompt_replaces_default_prompt() {
        let repo_path = temp_repo_path("prompt-replace");
        let lachesi_dir = repo_path.join(".lachesi");
        let pack_dir = lachesi_dir.join("packs/team-rules");
        fs::create_dir_all(&pack_dir).expect("create lachesi folder");
        fs::write(lachesi_dir.join("system-prompt.md"), "Replacement prompt.")
            .expect("write prompt");
        fs::write(
            pack_dir.join("pack.yaml"),
            r#"
id: team-rules
review:
  prompt:
    extend: Policy pack prompt.
"#,
        )
        .expect("write pack");

        let mut repo = repo("delaudio", "norn");
        repo.local_path = Some(repo_path.display().to_string());
        let app = TuiApp::from_repos(vec![repo]);

        let prompt = app
            .review_prompt_for_selected_repo()
            .expect("resolve prompt");

        assert!(prompt.starts_with("Replacement prompt."));
        assert!(prompt.contains("Policy pack prompt."));
        assert!(!prompt
            .contains("You are a senior software engineer doing a thorough pull request review."));
        let _ = fs::remove_dir_all(repo_path);
    }
}
