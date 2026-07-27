mod render;
mod terminal;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io::Write,
    process::{Command, Stdio},
    time::Duration,
};

use crossterm::event::{self, Event, KeyCode, MouseButton, MouseEvent, MouseEventKind};

use crate::config::{self, AiProvider, AppConfig, RepoRef};
use crate::local_repo;
use crate::services::bitbucket::{
    create_general_comment_native, get_pr_diff_native, get_pull_request_native,
    list_comments_native, list_pull_requests_native, validate_repo_review_config_native,
    ListPrOptions, PrComment, PullRequestDetail, PullRequestSummary,
};
use crate::services::review::{
    get_ai_review_run_state_native, load_ai_review_store_native, start_inline_review_native,
    AiReviewRunState, AiReviewRunStatus, AiReviewRunStore,
};

const TUI_SKIP_AI_REVIEW_ANALYZERS: bool = true;
use render::{
    detail_view_target, diff_content_width_for_area, mouse_target, render,
    selected_diff_file_patch, DetailView, DiffViewMode, DraftComment, FocusPane, MouseTarget,
    PrListFilter, TuiState,
};
use terminal::TerminalGuard;

const TICK_RATE: Duration = Duration::from_millis(250);
const DEFAULT_REVIEW_PROMPT: &str = include_str!("../../../src/lib/defaultReviewPrompt.md");

pub fn run_from_env() -> Result<(), String> {
    let launch_mode = launch_mode_from_args(std::env::args().skip(1))?;
    let mut config = config::load();
    let focus_prs = match launch_mode {
        TuiLaunchMode::Help => {
            println!("{}", tui_usage());
            return Ok(());
        }
        TuiLaunchMode::CurrentRepo => {
            config.repos = vec![local_repo::resolve_current_repo()?];
            true
        }
        TuiLaunchMode::Workspace => false,
    };
    let mut app = TuiApp::from_config(config);
    if focus_prs {
        app.focus = FocusPane::PullRequests;
    }
    app.load_selected_repo();
    let mut terminal = TerminalGuard::enter().map_err(|error| error.to_string())?;

    loop {
        app.refresh_ai_review_state();
        let area = terminal.area().map_err(|error| error.to_string())?;
        app.prepare_rendered_diff(area);
        terminal
            .draw(|frame| render(frame, app.view_state()))
            .map_err(|error| error.to_string())?;

        if app.should_quit || terminal.interrupted() {
            break;
        }

        if event::poll(TICK_RATE).map_err(|error| error.to_string())? {
            match event::read().map_err(|error| error.to_string())? {
                Event::Key(key) => app.handle_key(key.code),
                Event::Mouse(mouse) => {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TuiLaunchMode {
    Help,
    CurrentRepo,
    Workspace,
}

fn tui_usage() -> &'static str {
    "Usage: lac [--current-repo] [--workspace]\n\nBy default, lac opens pull requests for the git repository in the current directory."
}

fn launch_mode_from_args<I, S>(args: I) -> Result<TuiLaunchMode, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut mode = TuiLaunchMode::CurrentRepo;
    for arg in args {
        match arg.as_ref() {
            "--workspace" | "--global" => mode = TuiLaunchMode::Workspace,
            "--current-repo" => mode = TuiLaunchMode::CurrentRepo,
            "-h" | "--help" => return Ok(TuiLaunchMode::Help),
            unknown => {
                return Err(format!(
                    "Unknown option `{unknown}`. Use `lac --workspace` for the configured repository picker."
                ));
            }
        }
    }
    Ok(mode)
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
    error: Option<String>,
    status: String,
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
        Self {
            ai_provider: config.ai_provider,
            claude_model: config.claude_model,
            claude_effort: config.claude_effort,
            codex_model: config.codex_model,
            codex_effort: config.codex_effort,
            ..Self::from_repos(config.repos)
        }
    }

    fn from_repos(repos: Vec<RepoRef>) -> Self {
        Self {
            repos,
            selected_repo: 0,
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
            error: None,
            status: "Ready".to_string(),
            should_quit: false,
        }
    }

    fn handle_key(&mut self, code: KeyCode) {
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
            KeyCode::Char('g') => self.open_diff_view(),
            KeyCode::Char('u') => self.toggle_diff_view_mode(),
            KeyCode::Char('v') => self.toggle_detail_view(),
            KeyCode::Char('y') => self.copy_ai_review_output(),
            KeyCode::Char('r') => self.refresh_active_view(),
            KeyCode::PageUp => self.scroll_active_detail(-10),
            KeyCode::PageDown => self.scroll_active_detail(10),
            KeyCode::Home => self.reset_active_detail_scroll(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            _ => {}
        }
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
            self.status = "Selected next diff file".to_string();
        }
    }

    fn select_previous_diff_file(&mut self) {
        let previous = self.selected_diff_file;
        self.selected_diff_file = self.selected_diff_file.saturating_sub(1);
        if self.selected_diff_file != previous {
            self.diff_scroll = 0;
            self.rendered_diff = None;
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
        let Some(repo) = self.repos.get(self.selected_repo) else {
            self.pull_requests.clear();
            self.detail = None;
            self.comments.clear();
            self.ai_reviewed_pr_ids.clear();
            self.ai_review_running_pr_ids.clear();
            self.diff = None;
            self.drafts.clear();
            self.composer = None;
            self.reset_diff_state();
            self.status = "No repositories configured".to_string();
            return;
        };
        let provider = repo.provider;
        let workspace = repo.workspace.clone();
        let repo_name = repo.repo.clone();
        self.status = format!(
            "Loading {} PRs for {workspace}/{repo_name}...",
            self.pr_filter.label()
        );
        self.error = None;
        let opts = ListPrOptions {
            state: Some(self.pr_filter.provider_state().to_string()),
            page: Some(1),
            pagelen: Some(50),
            query: None,
            updated_after: None,
        };
        match list_pull_requests_native(
            Some(provider),
            workspace.as_str(),
            repo_name.as_str(),
            &opts,
        ) {
            Ok(page) => {
                self.pull_requests = page
                    .values
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
                self.rendered_diff = None;
                self.detail_view = DetailView::PullRequest;
                self.reset_detail_scrolls();
                self.reset_diff_state();
                self.ai_review_running_pr_ids.clear();
                self.refresh_ai_review_markers(workspace.as_str(), repo_name.as_str());
                self.status = format!(
                    "Loaded {} {} PRs",
                    self.pull_requests.len(),
                    self.pr_filter.label()
                );
                if !self.pull_requests.is_empty() {
                    self.load_selected_pr();
                }
            }
            Err(error) => {
                self.pull_requests.clear();
                self.detail = None;
                self.comments.clear();
                self.ai_reviewed_pr_ids.clear();
                self.ai_review_running_pr_ids.clear();
                self.diff = None;
                self.drafts.clear();
                self.composer = None;
                self.active_ai_target = None;
                self.ai_review_state = None;
                self.ai_review_output = None;
                self.detail_view = DetailView::PullRequest;
                self.reset_detail_scrolls();
                self.reset_diff_state();
                self.error = Some(error);
                self.status = "Failed to load PRs".to_string();
            }
        }
    }

    fn load_selected_pr(&mut self) {
        self.load_selected_pr_for_view(DetailView::PullRequest);
    }

    fn load_selected_pr_for_view(&mut self, target_view: DetailView) {
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
        self.status = format!("Loading PR #{pr_id}...");
        self.error = None;
        match get_pull_request_native(
            Some(provider),
            workspace.as_str(),
            repo_name.as_str(),
            pr_id,
        ) {
            Ok(detail) => {
                self.comments = list_comments_native(
                    Some(provider),
                    workspace.as_str(),
                    repo_name.as_str(),
                    pr_id,
                )
                .unwrap_or_else(|error| {
                    self.error = Some(format!("Comments failed: {error}"));
                    Vec::new()
                });
                self.diff = get_pr_diff_native(
                    Some(provider),
                    workspace.as_str(),
                    repo_name.as_str(),
                    pr_id,
                )
                .map_err(|error| {
                    self.error = Some(format!("Diff failed: {error}"));
                })
                .ok();
                self.selected_diff_file = 0;
                self.rendered_diff = None;
                self.detail = Some(detail);
                self.drafts.clear();
                self.composer = None;
                self.active_ai_target = Some((workspace.clone(), repo_name.clone(), pr_id));
                self.refresh_ai_review_state();
                self.refresh_ai_review_output();
                self.detail_view = target_view;
                self.reset_detail_scrolls();
                self.status = format!("Loaded PR #{pr_id}");
            }
            Err(error) => {
                self.detail = None;
                self.comments.clear();
                self.diff = None;
                self.drafts.clear();
                self.composer = None;
                self.active_ai_target = None;
                self.ai_review_state = None;
                self.ai_review_output = None;
                self.rendered_diff = None;
                self.detail_view = target_view;
                self.reset_detail_scrolls();
                self.error = Some(error);
                self.status = "Failed to load PR detail".to_string();
            }
        }
    }

    fn start_comment_composer(&mut self) {
        if self.selected_pull_request_id().is_none() {
            self.status = "Select a pull request before drafting a comment".to_string();
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
        if self.detail.as_ref().map(|detail| detail.id) != self.selected_pull_request_id() {
            self.load_selected_pr();
        }
        let Some((provider, workspace, repo, pr_id)) = self.selected_review_target() else {
            self.status = "Select a pull request before starting AI review".to_string();
            return;
        };
        let Some(detail) = self.detail.as_ref().filter(|detail| detail.id == pr_id) else {
            self.status = "Load pull request detail before starting AI review".to_string();
            return;
        };
        let title = detail.title.clone();
        let source_branch = detail.source_branch.clone();
        let destination_branch = detail.destination_branch.clone();
        let reviewed_head_sha = detail.source_commit_hash.clone();
        let diff = match self
            .diff
            .as_deref()
            .map(str::trim)
            .filter(|diff| !diff.is_empty())
        {
            Some(diff) => diff.to_string(),
            None => match get_pr_diff_native(Some(provider), &workspace, &repo, pr_id) {
                Ok(diff) => {
                    self.diff = Some(diff.clone());
                    diff
                }
                Err(error) => {
                    self.error = Some(error);
                    self.status = "Failed to load diff for AI review".to_string();
                    return;
                }
            },
        };
        let prompt = match self.review_prompt_for_selected_repo() {
            Ok(prompt) => prompt,
            Err(error) => {
                self.error = Some(error);
                self.status = "Failed to load review prompt".to_string();
                return;
            }
        };
        let payload = build_review_payload(&prompt, detail, &diff);
        match start_inline_review_native(
            self.ai_review_store.clone(),
            workspace.clone(),
            repo.clone(),
            pr_id,
            title,
            source_branch,
            destination_branch,
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
                self.active_ai_target = Some((workspace, repo, pr_id));
                self.ai_review_state = Some(state);
                self.ai_review_output = None;
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
            self.refresh_ai_review_output();
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
        if self.detail_view == DetailView::Diff {
            self.load_selected_pr_for_view(DetailView::Diff);
        } else {
            self.load_selected_repo();
        }
    }

    fn reset_diff_state(&mut self) {
        self.diff_scroll = 0;
        self.selected_diff_file = 0;
        self.rendered_diff = None;
    }

    fn toggle_diff_view_mode(&mut self) {
        self.diff_view_mode = self.diff_view_mode.next();
        self.diff_scroll = 0;
        self.rendered_diff = None;
        self.status = format!("Diff view: {}", self.diff_view_mode.label());
    }

    fn prepare_rendered_diff(&mut self, area: ratatui::layout::Rect) {
        if self.detail_view != DetailView::Diff {
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
        self.refresh_ai_review_output();
        self.copy_loaded_ai_review_output_with(|output| {
            terminal::copy_to_clipboard(output).map_err(|error| error.to_string())
        });
    }

    fn copy_loaded_ai_review_output_with(
        &mut self,
        copier: impl FnOnce(&str) -> Result<(), String>,
    ) {
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

    fn refresh_ai_review_state(&mut self) {
        self.refresh_running_ai_review_markers();
        let Some((workspace, repo, pr_id)) = self.active_ai_target.as_ref() else {
            return;
        };
        let active_pr_id = *pr_id;
        self.ai_review_state =
            get_ai_review_run_state_native(&self.ai_review_store, workspace, repo, active_pr_id);
        let current = self
            .ai_review_state
            .as_ref()
            .map(|state| (state.status, state.logs.last().cloned()));
        if let Some((status, latest_log)) = current {
            self.status = match status {
                AiReviewRunStatus::Running => {
                    self.mark_ai_review_running(active_pr_id);
                    format!(
                        "AI review running: {}",
                        latest_log.as_deref().unwrap_or("started")
                    )
                }
                AiReviewRunStatus::Succeeded => "AI review completed".to_string(),
                AiReviewRunStatus::Failed => "AI review failed".to_string(),
                AiReviewRunStatus::Cancelled => "AI review cancelled".to_string(),
                AiReviewRunStatus::Idle => self.status.clone(),
            };
        }
        if matches!(
            self.ai_review_state.as_ref().map(|state| state.status),
            Some(AiReviewRunStatus::Succeeded)
        ) {
            self.unmark_ai_review_running(active_pr_id);
            self.mark_ai_reviewed(active_pr_id);
            self.refresh_ai_review_output();
        } else if matches!(
            self.ai_review_state.as_ref().map(|state| state.status),
            Some(AiReviewRunStatus::Failed | AiReviewRunStatus::Cancelled)
        ) {
            self.unmark_ai_review_running(active_pr_id);
        }
    }

    fn refresh_running_ai_review_markers(&mut self) {
        let Some(repo) = self.repos.get(self.selected_repo) else {
            self.ai_review_running_pr_ids.clear();
            return;
        };
        let workspace = repo.workspace.clone();
        let repo_name = repo.repo.clone();
        let pr_ids = self
            .pull_requests
            .iter()
            .map(|pr| pr.id)
            .collect::<Vec<_>>();
        let mut running = Vec::new();
        let mut finished = Vec::new();
        for pr_id in pr_ids {
            let Some(state) = get_ai_review_run_state_native(
                &self.ai_review_store,
                &workspace,
                &repo_name,
                pr_id,
            ) else {
                continue;
            };
            match state.status {
                AiReviewRunStatus::Running => running.push(pr_id),
                AiReviewRunStatus::Succeeded => finished.push(pr_id),
                AiReviewRunStatus::Failed
                | AiReviewRunStatus::Cancelled
                | AiReviewRunStatus::Idle => {}
            }
        }
        self.ai_review_running_pr_ids = running;
        for pr_id in finished {
            self.mark_ai_reviewed(pr_id);
        }
    }

    fn refresh_ai_review_markers(&mut self, workspace: &str, repo: &str) {
        let mut reviewed = Vec::new();
        let mut running = Vec::new();
        for pr in &self.pull_requests {
            if matches!(
                get_ai_review_run_state_native(&self.ai_review_store, workspace, repo, pr.id)
                    .map(|state| state.status),
                Some(AiReviewRunStatus::Running)
            ) {
                running.push(pr.id);
            }
            if matches!(
                load_ai_review_store_native(workspace, repo, pr.id),
                Ok(Some(store)) if !store.review_runs.is_empty()
            ) {
                reviewed.push(pr.id);
            }
        }
        self.ai_reviewed_pr_ids = reviewed;
        self.ai_review_running_pr_ids = running;
    }

    fn mark_ai_reviewed(&mut self, pr_id: u32) {
        if !self.ai_reviewed_pr_ids.contains(&pr_id) {
            self.ai_reviewed_pr_ids.push(pr_id);
        }
    }

    fn mark_ai_review_running(&mut self, pr_id: u32) {
        if !self.ai_review_running_pr_ids.contains(&pr_id) {
            self.ai_review_running_pr_ids.push(pr_id);
        }
    }

    fn unmark_ai_review_running(&mut self, pr_id: u32) {
        self.ai_review_running_pr_ids.retain(|id| *id != pr_id);
    }

    fn refresh_ai_review_output(&mut self) {
        let Some((workspace, repo, pr_id)) = self.active_ai_target.as_ref() else {
            return;
        };
        match load_ai_review_store_native(workspace, repo, *pr_id) {
            Ok(Some(store)) => {
                self.ai_review_output = store
                    .review_runs
                    .iter()
                    .rev()
                    .find_map(|run| run.summary_markdown.clone());
            }
            Ok(None) => {
                self.ai_review_output = None;
            }
            Err(error) => {
                self.error = Some(format!("AI review output failed: {error}"));
            }
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
        let repo = self.repos.get(self.selected_repo)?;
        let pr_id = self.selected_pull_request_id()?;
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
            error: self.error.as_deref(),
            status: self.status.as_str(),
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

    fn temp_repo_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!("lachesi-tui-{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn tui_launch_defaults_to_current_repository() {
        assert_eq!(
            launch_mode_from_args(Vec::<String>::new()).unwrap(),
            TuiLaunchMode::CurrentRepo
        );
        assert_eq!(
            launch_mode_from_args(["--workspace"]).unwrap(),
            TuiLaunchMode::Workspace
        );
    }

    #[test]
    fn tui_launch_rejects_unknown_options() {
        let error = launch_mode_from_args(["--repo"]).unwrap_err();
        assert!(error.contains("Unknown option"));
    }

    #[test]
    fn repo_selection_stays_in_bounds() {
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
        app.pull_requests.clear();

        app.handle_key(KeyCode::Down);
        assert_eq!(app.selected_repo, 0);

        app.handle_key(KeyCode::Up);
        assert_eq!(app.selected_repo, 0);
    }

    #[test]
    fn mouse_selects_pull_request_rows() {
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
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
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
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
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);

        app.handle_key(KeyCode::Char('g'));
        assert_eq!(app.detail_view, DetailView::Diff);
        assert_eq!(app.focus, FocusPane::Diff);

        app.handle_key(KeyCode::Char('g'));
        assert_eq!(app.detail_view, DetailView::PullRequest);
        assert_eq!(app.focus, FocusPane::PullRequests);
    }

    #[test]
    fn diff_view_selection_moves_between_files() {
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
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
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
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
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);

        app.mark_ai_review_running(7);
        assert_eq!(app.ai_review_running_pr_ids, vec![7]);

        app.mark_ai_reviewed(7);
        app.unmark_ai_review_running(7);

        assert!(app.ai_review_running_pr_ids.is_empty());
        assert_eq!(app.ai_reviewed_pr_ids, vec![7]);
    }

    #[test]
    fn tui_ai_review_skips_duplicate_analyzers() {
        assert!(TUI_SKIP_AI_REVIEW_ANALYZERS);
    }

    #[test]
    fn copies_loaded_ai_review_output_without_visible_wrapping() {
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
        app.ai_review_output = Some("full markdown\nwith second line".to_string());
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
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);

        app.copy_loaded_ai_review_output_with(|_| Err("should not copy".to_string()));

        assert_eq!(app.status, "No AI review output to copy");
        assert!(app.error.is_none());
    }

    #[test]
    fn quit_keys_mark_app_done() {
        let mut app = TuiApp::from_repos(Vec::new());

        app.handle_key(KeyCode::Char('q'));

        assert!(app.should_quit);
    }

    #[test]
    fn mouse_wheel_scrolls_diff_view_without_switching_to_ai_review() {
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
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
        let mut app = TuiApp::from_repos(vec![repo("lachesi-hq", "lachesi")]);
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
    fn discard_drafts_keeps_remote_comments() {
        let mut app = TuiApp::from_repos(Vec::new());
        app.drafts.push(DraftComment {
            id: 1,
            raw: "pending".to_string(),
        });
        app.comments.push(PrComment {
            id: 1,
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
                    id: 10,
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

        let mut repo = repo("lachesi-hq", "lachesi");
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
