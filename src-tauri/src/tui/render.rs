use std::cmp;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Color, Frame, Line, Modifier, Span, Style},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};
use ratatui_image::Image as TerminalImage;
use unicode_segmentation::UnicodeSegmentation;

use super::image_diff::{ImageDiffState, ImageVersionState};
use super::loading::LoadState;
use crate::config::{RepoRef, ReviewProvider};
use crate::services::bitbucket::{PrComment, PullRequestDetail, PullRequestSummary};
use crate::services::review::{AiReviewRunState, AiReviewRunStatus};

#[derive(Clone, Copy)]
pub struct TuiState<'a> {
    pub repos: &'a [RepoRef],
    pub selected_repo: usize,
    pub focus: FocusPane,
    pub pull_requests: &'a [PullRequestSummary],
    pub pr_filter: PrListFilter,
    pub selected_pr: usize,
    pub detail: Option<&'a PullRequestDetail>,
    pub comments: &'a [PrComment],
    pub ai_reviewed_pr_ids: &'a [u32],
    pub ai_review_running_pr_ids: &'a [u32],
    pub diff: Option<&'a str>,
    pub drafts: &'a [DraftComment],
    pub composer: Option<&'a str>,
    pub ai_review: Option<&'a AiReviewRunState>,
    pub ai_review_output: Option<&'a str>,
    pub detail_view: DetailView,
    pub detail_scroll: usize,
    pub ai_review_scroll: usize,
    pub diff_scroll: usize,
    pub selected_diff_file: usize,
    pub diff_view_mode: DiffViewMode,
    pub rendered_diff_output: Option<&'a str>,
    pub image_diff: Option<&'a ImageDiffState>,
    pub image_protocol: &'a str,
    pub loading: LoadingView<'a>,
    pub error: Option<&'a str>,
    pub status: &'a str,
    pub diff_prompt_open: bool,
}

#[derive(Clone, Copy)]
pub struct LoadingView<'a> {
    pub repo: &'a LoadState,
    pub pull_requests: &'a LoadState,
    pub detail: &'a LoadState,
    pub comments: &'a LoadState,
    pub diff: &'a LoadState,
    pub ai_review: &'a LoadState,
    pub tick: usize,
}

#[cfg(test)]
impl LoadingView<'static> {
    fn idle() -> Self {
        static IDLE: LoadState = LoadState::Idle;
        Self {
            repo: &IDLE,
            pull_requests: &IDLE,
            detail: &IDLE,
            comments: &IDLE,
            diff: &IDLE,
            ai_review: &IDLE,
            tick: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftComment {
    pub id: u64,
    pub raw: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    Repositories,
    PullRequests,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailView {
    PullRequest,
    AiReview,
    Diff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewMode {
    Unified,
    Split,
}

impl DiffViewMode {
    pub fn next(self) -> Self {
        match self {
            Self::Unified => Self::Split,
            Self::Split => Self::Unified,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Unified => "unified",
            Self::Split => "side-by-side",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrListFilter {
    Open,
    Draft,
    Merged,
}

impl PrListFilter {
    pub fn next(self) -> Self {
        match self {
            Self::Open => Self::Draft,
            Self::Draft => Self::Merged,
            Self::Merged => Self::Open,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Draft => "draft",
            Self::Merged => "merged",
        }
    }

    pub fn provider_state(self) -> &'static str {
        match self {
            Self::Open | Self::Draft => "OPEN",
            Self::Merged => "MERGED",
        }
    }

    pub fn includes(self, pr: &PullRequestSummary) -> bool {
        match self {
            Self::Open | Self::Merged => true,
            Self::Draft => pr.draft,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseTarget {
    Repository(usize),
    PullRequest(usize),
    PrFilter(PrListFilter),
    DiffFile(usize),
}

const SURFACE: Color = Color::Rgb(13, 17, 23);
const PANEL: Color = Color::Rgb(18, 24, 32);
const BORDER: Color = Color::Rgb(82, 96, 112);
const TEXT: Color = Color::Rgb(222, 228, 236);
const MUTED: Color = Color::Rgb(136, 148, 164);
const ACCENT: Color = Color::Rgb(255, 176, 64);
const INFO: Color = Color::Rgb(94, 188, 255);
const SUCCESS: Color = Color::Rgb(74, 222, 128);
const ERROR: Color = Color::LightRed;

pub fn render(frame: &mut Frame<'_>, state: TuiState<'_>) {
    let area = frame.area();
    frame.render_widget(Block::default().style(base_style()), area);
    let [header, body, footer] = *vertical_areas().split(area) else {
        return;
    };

    render_header(frame, header);

    if state.detail_view == DetailView::Diff {
        render_diff_page(frame, body, state);
        render_footer(frame, footer, state);
        if state.diff_prompt_open {
            render_diff_choice_modal(frame, area);
        }
        return;
    }

    let [left, review] = *body_areas().split(body) else {
        return;
    };

    render_left_panel(frame, left, state);
    render_review(frame, review, state);
    render_footer(frame, footer, state);

    if state.diff_prompt_open {
        render_diff_choice_modal(frame, area);
    }
}

pub fn mouse_target(area: Rect, x: u16, y: u16, state: TuiState<'_>) -> Option<MouseTarget> {
    let [_, body, _] = *vertical_areas().split(area) else {
        return None;
    };
    if state.detail_view == DetailView::Diff {
        let [files, _] = *diff_page_areas().split(body) else {
            return None;
        };
        let file_count = parse_diff_files(state.diff).len();
        let offset = diff_file_list_offset(file_count, files.height, state.selected_diff_file);
        return list_index_at(files, x, y, file_count.saturating_sub(offset))
            .map(|index| MouseTarget::DiffFile(index + offset));
    }

    let [left, review] = *body_areas().split(body) else {
        return None;
    };
    let [repos, filters] = *left_areas().split(left) else {
        return None;
    };
    if let Some(index) = list_index_at(repos, x, y, state.repos.len()) {
        return Some(MouseTarget::Repository(index));
    }
    if let Some(filter) = pr_filter_at(filters, x, y) {
        return Some(MouseTarget::PrFilter(filter));
    }

    let [pr_list, _] = *review_areas().split(review) else {
        return None;
    };
    list_index_at(pr_list, x, y, state.pull_requests.len()).map(MouseTarget::PullRequest)
}

pub fn detail_view_target(area: Rect, x: u16, y: u16, side_view: DetailView) -> Option<DetailView> {
    let [_, body, _] = *vertical_areas().split(area) else {
        return None;
    };
    let [_, review] = *body_areas().split(body) else {
        return None;
    };
    let [_, details] = *review_areas().split(review) else {
        return None;
    };
    let [pull_request, side_pane] = *detail_areas().split(details) else {
        return None;
    };
    if rect_contains(pull_request, x, y) {
        Some(DetailView::PullRequest)
    } else if rect_contains(side_pane, x, y) {
        Some(if side_view == DetailView::Diff {
            DetailView::Diff
        } else {
            DetailView::AiReview
        })
    } else {
        None
    }
}

fn vertical_areas() -> Layout {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
}

fn body_areas() -> Layout {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(28), Constraint::Percentage(72)])
}

fn left_areas() -> Layout {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(4)])
}

fn review_areas() -> Layout {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(66)])
}

fn detail_areas() -> Layout {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
}

fn diff_page_areas() -> Layout {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
}

pub fn diff_content_width_for_area(area: Rect) -> usize {
    let [_, body, _] = *vertical_areas().split(area) else {
        return 80;
    };
    let [_, diff_area] = *diff_page_areas().split(body) else {
        return 80;
    };
    content_width(diff_area)
}

pub fn diff_image_area_for_area(area: Rect, has_detail: bool) -> Rect {
    let [_, body, _] = *vertical_areas().split(area) else {
        return Rect::new(0, 0, 1, 1);
    };
    let [_, diff_area] = *diff_page_areas().split(body) else {
        return Rect::new(0, 0, 1, 1);
    };
    image_render_areas(diff_area, has_detail).1
}

fn rect_contains(area: Rect, x: u16, y: u16) -> bool {
    x >= area.x
        && x < area.x.saturating_add(area.width)
        && y >= area.y
        && y < area.y.saturating_add(area.height)
}

fn list_index_at(area: Rect, x: u16, y: u16, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let inner_x = area.x.saturating_add(1)..area.x.saturating_add(area.width.saturating_sub(1));
    let inner_y = area.y.saturating_add(1)..area.y.saturating_add(area.height.saturating_sub(1));
    if !inner_x.contains(&x) || !inner_y.contains(&y) {
        return None;
    }
    let index = usize::from(y.saturating_sub(area.y).saturating_sub(1));
    (index < len).then_some(index)
}

fn pr_filter_at(area: Rect, x: u16, y: u16) -> Option<PrListFilter> {
    if area.width < 3 || area.height < 3 {
        return None;
    }
    let inner_x_start = area.x.saturating_add(1);
    let inner_x_end = area.x.saturating_add(area.width.saturating_sub(1));
    let inner_y = area.y.saturating_add(1)..area.y.saturating_add(area.height.saturating_sub(1));
    if x < inner_x_start || x >= inner_x_end || !inner_y.contains(&y) {
        return None;
    }
    let width = inner_x_end.saturating_sub(inner_x_start).max(1);
    let relative_x = x.saturating_sub(inner_x_start);
    let segment = (u32::from(relative_x) * 3 / u32::from(width)).min(2);
    match segment {
        0 => Some(PrListFilter::Open),
        1 => Some(PrListFilter::Draft),
        _ => Some(PrListFilter::Merged),
    }
}

fn render_header(frame: &mut Frame<'_>, area: Rect) {
    let header = Paragraph::new(Line::from(vec![
        Span::styled("Norn", accent_style().add_modifier(Modifier::BOLD)),
        Span::styled(" terminal review workspace", text_style()),
    ]))
    .style(base_style())
    .block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(border_style()),
    );
    frame.render_widget(header, area);
}

fn render_left_panel(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let [repos, filters] = *left_areas().split(area) else {
        return;
    };
    render_repos(frame, repos, state);
    render_pr_filters(frame, filters, state.pr_filter);
}

fn render_repos(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let items = if state.repos.is_empty() && state.loading.repo.is_loading() {
        vec![ListItem::new(loading_line(
            "Resolving repository",
            state.loading.tick,
        ))]
    } else if let Some(error) = state.loading.repo.error() {
        vec![ListItem::new(Line::from(vec![
            Span::styled("Error: ", error_style()),
            Span::styled(error.to_string(), muted_style()),
        ]))]
    } else if state.repos.is_empty() {
        vec![ListItem::new("No repositories configured")]
    } else {
        state
            .repos
            .iter()
            .enumerate()
            .map(|(index, repo)| {
                let selected = index == state.selected_repo;
                let marker = if selected { ">" } else { " " };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        marker,
                        if selected {
                            accent_style()
                        } else {
                            muted_style()
                        },
                    ),
                    Span::raw(" "),
                    Span::styled(provider_label(repo.provider), provider_style(repo.provider)),
                    Span::styled(
                        format!(" {}/{}", repo.workspace, repo.repo),
                        if selected {
                            text_style().add_modifier(Modifier::BOLD)
                        } else {
                            text_style()
                        },
                    ),
                ]))
            })
            .collect()
    };
    let title = if state.focus == FocusPane::Repositories {
        "Repositories *"
    } else {
        "Repositories"
    };
    let list = List::new(items).style(panel_style()).block(panel_block(
        loading_title(title, state.loading.repo),
        state.focus == FocusPane::Repositories,
    ));
    frame.render_widget(list, area);
}

fn render_pr_filters(frame: &mut Frame<'_>, area: Rect, selected_filter: PrListFilter) {
    let filters = [
        (PrListFilter::Open, "Open"),
        (PrListFilter::Draft, "Draft"),
        (PrListFilter::Merged, "Merged"),
    ];
    let mut spans = Vec::new();
    for (index, (filter, label)) in filters.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", muted_style()));
        }
        if filter == selected_filter {
            spans.push(Span::styled(
                format!("[{label}]"),
                accent_style().add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(format!(" {label} "), text_style()));
        }
    }
    let filters = Paragraph::new(Line::from(spans))
        .style(panel_style())
        .block(panel_block("PR filter", false));
    frame.render_widget(filters, area);
}

fn render_review(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let [pr_list, detail] = *review_areas().split(area) else {
        return;
    };
    render_pull_requests(frame, pr_list, state);
    render_detail(frame, detail, state);
}

fn render_diff_page(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let [files_area, diff_area] = *diff_page_areas().split(area) else {
        return;
    };
    let files = parse_diff_files(state.diff);
    render_diff_file_list(frame, files_area, &files, state.selected_diff_file);
    render_diff_file(frame, diff_area, state, &files);
}

fn render_diff_file_list(
    frame: &mut Frame<'_>,
    area: Rect,
    files: &[DiffFileSection<'_>],
    selected_file: usize,
) {
    let items = if files.is_empty() {
        vec![ListItem::new(Span::styled("No diff loaded", muted_style()))]
    } else {
        let offset = diff_file_list_offset(files.len(), area.height, selected_file);
        files
            .iter()
            .enumerate()
            .skip(offset)
            .map(|(index, file)| {
                let selected = index == selected_file.min(files.len().saturating_sub(1));
                let change_count = format!(" +{}/-{}", file.additions, file.deletions);
                let content_width = usize::from(area.width.saturating_sub(2));
                let path_width = content_width
                    .saturating_sub(2 + file.status_label().len() + 1 + change_count.len());
                let display_path = compact_path(file.display_path(), path_width);
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if selected { "> " } else { "  " },
                        if selected {
                            accent_style()
                        } else {
                            muted_style()
                        },
                    ),
                    Span::styled(file.status_label(), file.status_style()),
                    Span::styled(" ", muted_style()),
                    Span::styled(
                        display_path,
                        if selected {
                            text_style().add_modifier(Modifier::BOLD)
                        } else {
                            text_style()
                        },
                    ),
                    Span::styled(change_count, muted_style()),
                ]))
            })
            .collect()
    };
    let list = List::new(items)
        .style(panel_style())
        .block(panel_block("Files *", true));
    frame.render_widget(list, area);
}

fn render_diff_file(
    frame: &mut Frame<'_>,
    area: Rect,
    state: TuiState<'_>,
    files: &[DiffFileSection<'_>],
) {
    if let Some(image) = state
        .image_diff
        .filter(|image| image.selected_file == state.selected_diff_file)
    {
        render_image_diff_file(frame, area, state, image);
        return;
    }
    let mut lines = Vec::new();
    if state.loading.diff.is_loading() {
        lines.push(loading_line("Loading diff", state.loading.tick));
        lines.push(Line::from(""));
    }
    if let Some(error) = state.loading.diff.error() {
        lines.push(Line::from(vec![
            Span::styled("Diff failed: ", error_style()),
            Span::styled(error.to_string(), muted_style()),
        ]));
        lines.push(Line::from(""));
    }
    if let Some(detail) = state.detail {
        lines.push(Line::from(vec![
            Span::styled(
                format!("#{} ", detail.id),
                info_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                detail.title.clone(),
                text_style().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(detail.source_branch.clone(), branch_style()),
            Span::styled(" -> ", muted_style()),
            Span::styled(detail.destination_branch.clone(), branch_style()),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(file) = files.get(state.selected_diff_file.min(files.len().saturating_sub(1))) {
        lines.push(Line::from(vec![
            Span::styled(file.status_label(), file.status_style()),
            Span::styled(" ", muted_style()),
            Span::styled(
                file.display_path(),
                text_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  +{}/-{}", file.additions, file.deletions),
                muted_style(),
            ),
            Span::styled(format!("  {}", state.diff_view_mode.label()), muted_style()),
        ]));
        lines.push(Line::from(""));
        if let Some(output) = state.rendered_diff_output {
            append_pager_diff_lines(&mut lines, output);
        } else {
            match state.diff_view_mode {
                DiffViewMode::Unified => {
                    append_diff_file_lines(&mut lines, file, content_width(area))
                }
                DiffViewMode::Split => {
                    append_split_diff_file_lines(&mut lines, file, content_width(area))
                }
            }
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Load a pull request to view its diff.",
            muted_style(),
        )));
    }

    let content_len = lines.len();
    let scroll = clamped_scroll(content_len, area.height, state.diff_scroll);
    let diff = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .style(if state.loading.diff.is_loading() && state.diff.is_some() {
            panel_style().add_modifier(Modifier::DIM)
        } else {
            panel_style()
        })
        .block(panel_block(
            loading_title("Diff *", state.loading.diff),
            true,
        ));
    frame.render_widget(diff, area);
    render_scrollbar(frame, area, content_len, scroll);
}

fn render_image_diff_file(
    frame: &mut Frame<'_>,
    area: Rect,
    state: TuiState<'_>,
    image: &ImageDiffState,
) {
    let block = panel_block("Diff *", true);
    frame.render_widget(block, area);
    let (metadata_area, image_area) = image_render_areas(area, state.detail.is_some());
    let mut lines = Vec::new();
    if let Some(detail) = state.detail {
        lines.push(Line::from(vec![
            Span::styled(
                format!("#{} ", detail.id),
                info_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                detail.title.clone(),
                text_style().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(detail.source_branch.clone(), branch_style()),
            Span::styled(" -> ", muted_style()),
            Span::styled(detail.destination_branch.clone(), branch_style()),
        ]));
    }

    if let Some(areas) = image.comparison_areas(image_area) {
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", image.candidate.kind.label().to_uppercase()),
                accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "base and changed versions",
                text_style().add_modifier(Modifier::BOLD),
            ),
        ]));
        if let Some(version) = image.old.as_ref() {
            lines.push(image_version_metadata(
                "base",
                version,
                state.image_protocol,
            ));
        }
        if let Some(version) = image.new.as_ref() {
            lines.push(image_version_metadata(
                "changed",
                version,
                state.image_protocol,
            ));
        }
        frame.render_widget(Paragraph::new(lines).style(panel_style()), metadata_area);
        frame.render_widget(
            Paragraph::new("BASE").style(info_style().add_modifier(Modifier::BOLD)),
            areas.base_label,
        );
        frame.render_widget(
            Paragraph::new("CHANGED").style(info_style().add_modifier(Modifier::BOLD)),
            areas.changed_label,
        );
        if let Some(version) = image.old.as_ref() {
            render_image_version(frame, version, areas.base_image);
        }
        if let Some(version) = image.new.as_ref() {
            render_image_version(frame, version, areas.changed_image);
        }
        return;
    }

    if let Some(version) = image.selected() {
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{}] ", image.candidate.kind.label().to_uppercase()),
                accent_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                version.path().to_string(),
                text_style().add_modifier(Modifier::BOLD),
            ),
        ]));
        match version {
            ImageVersionState::Ready(version) => {
                lines.push(Line::from(vec![
                    Span::styled(format!("{} image", version.side.label()), info_style()),
                    Span::styled(" | ", muted_style()),
                    Span::styled(version.format, text_style()),
                    Span::styled(" | ", muted_style()),
                    Span::styled(
                        format!("{}x{}", version.width, version.height),
                        text_style(),
                    ),
                    Span::styled(" | ", muted_style()),
                    Span::styled(format_bytes(version.byte_size), text_style()),
                    Span::styled(" | ", muted_style()),
                    Span::styled(
                        if version.protocol.is_some() {
                            state.image_protocol.to_string()
                        } else {
                            "metadata fallback".to_string()
                        },
                        muted_style(),
                    ),
                ]));
                if image.candidate.old_path.is_some() && image.candidate.new_path.is_some() {
                    lines.push(Line::from(Span::styled(
                        "Press i to switch base/changed.",
                        muted_style(),
                    )));
                } else if let Some(error) = version.protocol_error.as_ref() {
                    lines.push(Line::from(Span::styled(error.clone(), error_style())));
                }
            }
            ImageVersionState::Failed(version) => {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} image | metadata unavailable", version.side.label()),
                        muted_style(),
                    ),
                    Span::styled(" | ", muted_style()),
                    Span::styled(state.image_protocol.to_string(), muted_style()),
                ]));
                lines.push(Line::from(Span::styled(
                    version.error.clone(),
                    error_style(),
                )));
            }
        }
    }
    frame.render_widget(Paragraph::new(lines).style(panel_style()), metadata_area);

    if let Some(version) = image.selected() {
        render_image_version(frame, version, image_area);
    }
}

fn image_version_metadata(
    side: &str,
    version: &ImageVersionState,
    protocol: &str,
) -> Line<'static> {
    match version {
        ImageVersionState::Ready(version) => Line::from(format!(
            "{side}: {} | {}x{} | {} | {}",
            version.format,
            version.width,
            version.height,
            format_bytes(version.byte_size),
            if version.protocol.is_some() {
                protocol.to_string()
            } else {
                "metadata fallback".to_string()
            }
        )),
        ImageVersionState::Failed(version) => {
            Line::from(format!("{side}: metadata unavailable | {}", version.error))
        }
    }
}

fn render_image_version(frame: &mut Frame<'_>, version: &ImageVersionState, area: Rect) {
    match version {
        ImageVersionState::Ready(version) => {
            if let Some(protocol) = version.protocol.as_ref() {
                frame.render_widget(TerminalImage::new(protocol), area);
            } else {
                let fallback = version.protocol_error.as_deref().unwrap_or(
                    "This terminal has no supported inline image protocol. Metadata remains available.",
                );
                frame.render_widget(Paragraph::new(fallback).style(muted_style()), area);
            }
        }
        ImageVersionState::Failed(_) => {
            frame.render_widget(
                Paragraph::new("Image preview unavailable.").style(muted_style()),
                area,
            );
        }
    }
}

fn image_render_areas(area: Rect, has_detail: bool) -> (Rect, Rect) {
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let metadata_height = if has_detail { 5 } else { 3 };
    let [metadata, image] = *Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(metadata_height.min(inner.height)),
            Constraint::Min(1),
        ])
        .split(inner)
    else {
        return (inner, inner);
    };
    (metadata, image)
}

fn format_bytes(size: usize) -> String {
    if size < 1024 {
        format!("{size} B")
    } else if size < 1024 * 1024 {
        format!("{:.1} KiB", size as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", size as f64 / (1024.0 * 1024.0))
    }
}

fn render_pull_requests(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let loading = state.loading.pull_requests.is_loading();
    let items = if state.pull_requests.is_empty() && loading {
        vec![ListItem::new(loading_line(
            "Loading pull requests",
            state.loading.tick,
        ))]
    } else if let Some(error) = state.loading.pull_requests.error() {
        vec![ListItem::new(Line::from(vec![
            Span::styled("Could not load PRs: ", error_style()),
            Span::styled(error.to_string(), muted_style()),
        ]))]
    } else if state.pull_requests.is_empty() {
        vec![ListItem::new(format!(
            "No {} pull requests loaded",
            state.pr_filter.label()
        ))]
    } else {
        state
            .pull_requests
            .iter()
            .enumerate()
            .map(|(index, pr)| {
                let selected = index == state.selected_pr;
                let reviewed = state.ai_reviewed_pr_ids.contains(&pr.id);
                let running = state.ai_review_running_pr_ids.contains(&pr.id);
                let marker = if selected { ">" } else { " " };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        marker,
                        if selected {
                            accent_style()
                        } else {
                            muted_style()
                        },
                    ),
                    Span::raw(" "),
                    pr_review_marker(running, reviewed),
                    Span::styled(format!("#{} ", pr.id), info_style()),
                    Span::styled(pr.source_branch.clone(), branch_style()),
                    Span::styled(" -> ", muted_style()),
                    Span::styled(pr.destination_branch.clone(), branch_style()),
                    Span::styled("  ", text_style()),
                    Span::styled(
                        pr.title.clone(),
                        if selected {
                            text_style().add_modifier(Modifier::BOLD)
                        } else {
                            text_style()
                        },
                    ),
                    Span::styled("  by ", muted_style()),
                    Span::styled(author_label(pr.author_display_name.as_str()), text_style()),
                ]))
                .style(if loading {
                    Style::default().add_modifier(Modifier::DIM)
                } else {
                    Style::default()
                })
            })
            .collect()
    };
    let title = if state.focus == FocusPane::PullRequests {
        "Pull requests *"
    } else {
        "Pull requests"
    };
    let list = List::new(items).style(panel_style()).block(panel_block(
        loading_title(title, state.loading.pull_requests),
        state.focus == FocusPane::PullRequests,
    ));
    frame.render_widget(list, area);
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let [pull_request, side_pane] = *detail_areas().split(area) else {
        return;
    };
    render_pull_request_detail(frame, pull_request, state);
    if state.detail_view == DetailView::Diff {
        render_diff_detail(frame, side_pane, state);
    } else {
        render_ai_review_detail(frame, side_pane, state);
    }
}

fn render_pull_request_detail(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let mut lines = Vec::new();
    let loading = state.loading.detail.is_loading() || state.loading.pull_requests.is_loading();

    if loading {
        lines.push(loading_line("Loading pull request", state.loading.tick));
        lines.push(Line::from(""));
    }
    if let Some(error) = state.loading.detail.error() {
        lines.push(Line::from(vec![
            Span::styled("Detail failed: ", error_style()),
            Span::styled(error.to_string(), muted_style()),
        ]));
        lines.push(Line::from(""));
    }
    if let Some(error) = state.loading.comments.error() {
        lines.push(Line::from(vec![
            Span::styled("Comments failed: ", error_style()),
            Span::styled(error.to_string(), muted_style()),
        ]));
        lines.push(Line::from(""));
    }

    if let Some(error) = state.error {
        lines.push(Line::from(vec![
            Span::styled("Error: ", error_style().add_modifier(Modifier::BOLD)),
            Span::styled(error, error_style()),
        ]));
        lines.push(Line::from(""));
    }

    match state.detail {
        Some(detail) => {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("#{} ", detail.id),
                    info_style().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    detail.title.as_str(),
                    text_style().add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(detail.source_branch.as_str(), branch_style()),
                Span::styled(" -> ", muted_style()),
                Span::styled(detail.destination_branch.as_str(), branch_style()),
                Span::styled(" | ", muted_style()),
                Span::styled(detail.state.as_str(), status_style(detail.state.as_str())),
                Span::styled(" | ", muted_style()),
                Span::styled(
                    if state.loading.comments.is_loading() {
                        format!("{} comments (updating)", state.comments.len())
                    } else {
                        format!("{} comments", state.comments.len())
                    },
                    text_style(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Author: ", muted_style()),
                Span::styled(
                    author_label(detail.author_display_name.as_str()),
                    text_style(),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("Drafts: ", muted_style()),
                Span::styled(format!("{} pending", state.drafts.len()), text_style()),
            ]));
            lines.push(Line::from(""));
            append_description_lines(
                &mut lines,
                detail.description_raw.as_str(),
                content_width(area),
            );
            lines.push(Line::from(""));
            lines.push(Line::from(diff_preview(state.diff)));
            append_draft_preview(&mut lines, state.drafts);
        }
        None => match state.repos.get(state.selected_repo) {
            Some(repo) => {
                lines.push(Line::from(vec![
                    Span::styled("Provider: ", accent_style().add_modifier(Modifier::BOLD)),
                    Span::styled(provider_label(repo.provider), provider_style(repo.provider)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Repository: ", accent_style().add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{}/{}", repo.workspace, repo.repo), text_style()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Local path: ", accent_style().add_modifier(Modifier::BOLD)),
                    Span::styled(
                        repo.local_path.as_deref().unwrap_or("not configured"),
                        text_style(),
                    ),
                ]));
            }
            None => {
                lines.push(Line::from("Configure repos in Norn settings."));
                lines.push(Line::from("TUI uses the desktop app settings file."));
            }
        },
    }

    if let Some(composer) = state.composer {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Draft: ", accent_style().add_modifier(Modifier::BOLD)),
            Span::styled(composer, text_style()),
        ]));
    }

    let content_len = lines.len();
    let scroll = clamped_scroll(content_len, area.height, state.detail_scroll);
    let title = if state.detail_view == DetailView::PullRequest {
        "Pull request *"
    } else {
        "Pull request"
    };
    let detail = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .style(if loading && state.detail.is_some() {
            panel_style().add_modifier(Modifier::DIM)
        } else {
            panel_style()
        })
        .block(panel_block(
            if loading {
                format!("{title} ↻")
            } else {
                title.to_string()
            },
            state.detail_view == DetailView::PullRequest,
        ));
    frame.render_widget(detail, area);
    render_scrollbar(frame, area, content_len, scroll);
}

fn render_ai_review_detail(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let mut lines = Vec::new();
    if state.loading.ai_review.is_loading() {
        lines.push(loading_line("Loading review state", state.loading.tick));
        lines.push(Line::from(""));
    }
    if let Some(error) = state.loading.ai_review.error() {
        lines.push(Line::from(vec![
            Span::styled("Review state failed: ", error_style()),
            Span::styled(error.to_string(), muted_style()),
        ]));
        lines.push(Line::from(""));
    }
    if let Some(ai_review) = state.ai_review {
        lines.push(Line::from(vec![
            Span::styled("Status: ", muted_style()),
            Span::styled(
                ai_review_status_label(ai_review.status),
                ai_review_status_style(ai_review.status),
            ),
            Span::styled(
                ai_review
                    .error
                    .as_deref()
                    .map(|error| format!(" ({error})"))
                    .unwrap_or_default(),
                error_style(),
            ),
        ]));
        if let Some(log) = ai_review.logs.last() {
            lines.push(Line::from(vec![
                Span::styled("Log: ", muted_style()),
                Span::styled(log.clone(), text_style()),
            ]));
        }
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from("No AI review run for this pull request."));
        lines.push(Line::from(""));
    }
    append_ai_review_output(&mut lines, state.ai_review_output, content_width(area));

    let content_len = lines.len();
    let scroll = clamped_scroll(content_len, area.height, state.ai_review_scroll);
    let title = if state.detail_view == DetailView::AiReview {
        "AI review *"
    } else {
        "AI review"
    };
    let detail = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .style(
            if state.loading.ai_review.is_loading() && state.ai_review.is_some() {
                panel_style().add_modifier(Modifier::DIM)
            } else {
                panel_style()
            },
        )
        .block(panel_block(
            loading_title(title, state.loading.ai_review),
            state.detail_view == DetailView::AiReview,
        ));
    frame.render_widget(detail, area);
    render_scrollbar(frame, area, content_len, scroll);
}

fn render_diff_detail(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    let mut lines = Vec::new();
    if state.loading.diff.is_loading() {
        lines.push(loading_line("Loading diff", state.loading.tick));
        lines.push(Line::from(""));
    }
    if let Some(error) = state.loading.diff.error() {
        lines.push(Line::from(vec![
            Span::styled("Diff failed: ", error_style()),
            Span::styled(error.to_string(), muted_style()),
        ]));
        lines.push(Line::from(""));
    }
    if let Some(detail) = state.detail {
        lines.push(Line::from(vec![
            Span::styled(
                format!("#{} ", detail.id),
                info_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                detail.title.clone(),
                text_style().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled(detail.source_branch.clone(), branch_style()),
            Span::styled(" -> ", muted_style()),
            Span::styled(detail.destination_branch.clone(), branch_style()),
        ]));
        lines.push(Line::from(""));
    } else {
        lines.push(Line::from(Span::styled(
            "Load a pull request to view its diff.",
            muted_style(),
        )));
        lines.push(Line::from(""));
    }
    append_diff_lines(&mut lines, state.diff, content_width(area));

    let content_len = lines.len();
    let scroll = clamped_scroll(content_len, area.height, state.diff_scroll);
    let title = if state.detail_view == DetailView::Diff {
        "Diff *"
    } else {
        "Diff"
    };
    let detail = Paragraph::new(lines)
        .scroll((scroll as u16, 0))
        .style(if state.loading.diff.is_loading() && state.diff.is_some() {
            panel_style().add_modifier(Modifier::DIM)
        } else {
            panel_style()
        })
        .block(panel_block(
            loading_title(title, state.loading.diff),
            state.detail_view == DetailView::Diff,
        ));
    frame.render_widget(detail, area);
    render_scrollbar(frame, area, content_len, scroll);
}

#[derive(Debug, Clone)]
struct DiffFileSection<'a> {
    old_path: &'a str,
    new_path: &'a str,
    lines: Vec<&'a str>,
    additions: usize,
    deletions: usize,
}

impl DiffFileSection<'_> {
    fn display_path(&self) -> &str {
        if self.new_path == "/dev/null" {
            self.old_path
        } else {
            self.new_path
        }
    }

    fn status_label(&self) -> &'static str {
        if self.old_path == "/dev/null" {
            "[A]"
        } else if self.new_path == "/dev/null" {
            "[D]"
        } else if self.old_path != self.new_path {
            "[R]"
        } else {
            "[M]"
        }
    }

    fn status_style(&self) -> Style {
        if self.old_path == "/dev/null" {
            success_style()
        } else if self.new_path == "/dev/null" {
            error_style()
        } else if self.old_path != self.new_path {
            info_style()
        } else {
            accent_style()
        }
    }
}

fn render_footer(frame: &mut Frame<'_>, area: Rect, state: TuiState<'_>) {
    if state.diff_prompt_open {
        let spans = vec![
            Span::styled("Open diff in: ", panel_muted_style()),
            Span::styled("g", accent_style().add_modifier(Modifier::BOLD)),
            Span::styled(" native TUI  ", muted_style()),
            Span::styled("b", accent_style().add_modifier(Modifier::BOLD)),
            Span::styled(" browser  ", muted_style()),
            Span::styled("Esc", accent_style()),
            Span::styled(" cancel  |  ", muted_style()),
            Span::styled(state.status, info_style()),
        ];
        frame.render_widget(Paragraph::new(Line::from(spans)).style(base_style()), area);
        return;
    }
    let diff_action = if state.detail_view == DetailView::Diff {
        " close diff  "
    } else {
        " diff  "
    };
    let compact = area.width <= 120;
    let footer = if compact {
        let primary = Line::from(vec![
            Span::styled("q", accent_style()),
            Span::styled(" quit ", muted_style()),
            Span::styled("tab", accent_style()),
            Span::styled(" focus ", muted_style()),
            Span::styled("s", accent_style()),
            Span::styled(" settings ", muted_style()),
            Span::styled("j/k", accent_style()),
            Span::styled(" select ", muted_style()),
            Span::styled("enter", accent_style()),
            Span::styled(" load | ", muted_style()),
            Span::styled(state.status, info_style()),
        ]);
        let secondary = Line::from(vec![
            Span::styled("c", accent_style()),
            Span::styled(" draft ", muted_style()),
            Span::styled("p", accent_style()),
            Span::styled(" send ", muted_style()),
            Span::styled("x", accent_style()),
            Span::styled(" drop ", muted_style()),
            Span::styled("a", accent_style()),
            Span::styled(" AI ", muted_style()),
            Span::styled("f", accent_style()),
            Span::styled(" PRs ", muted_style()),
            Span::styled("g", accent_style()),
            Span::styled(" diff ", muted_style()),
            Span::styled("u", accent_style()),
            Span::styled(" view ", muted_style()),
            Span::styled("i", accent_style()),
            Span::styled(" img ", muted_style()),
            Span::styled("v", accent_style()),
            Span::styled(" pane ", muted_style()),
            Span::styled("y", accent_style()),
            Span::styled(" copy ", muted_style()),
            Span::styled("r", accent_style()),
            Span::styled(" reload", muted_style()),
        ]);
        Paragraph::new(vec![primary, secondary]).style(base_style())
    } else {
        let spans = vec![
            Span::styled("q", accent_style()),
            Span::styled(" quit  ", muted_style()),
            Span::styled("tab", accent_style()),
            Span::styled(" focus  ", muted_style()),
            Span::styled("s", accent_style()),
            Span::styled(" settings  ", muted_style()),
            Span::styled("j/k", accent_style()),
            Span::styled(" select  ", muted_style()),
            Span::styled("enter", accent_style()),
            Span::styled(" load  ", muted_style()),
            Span::styled("c", accent_style()),
            Span::styled(" draft  ", muted_style()),
            Span::styled("p", accent_style()),
            Span::styled(" publish  ", muted_style()),
            Span::styled("x", accent_style()),
            Span::styled(" discard  ", muted_style()),
            Span::styled("a", accent_style()),
            Span::styled(" ai review  ", muted_style()),
            Span::styled("f", accent_style()),
            Span::styled(" filter  ", muted_style()),
            Span::styled("g", accent_style()),
            Span::styled(diff_action, muted_style()),
            Span::styled("u", accent_style()),
            Span::styled(" unified/split  ", muted_style()),
            Span::styled("v", accent_style()),
            Span::styled(" pane  ", muted_style()),
            Span::styled("y", accent_style()),
            Span::styled(" copy review  ", muted_style()),
            Span::styled("PgUp/PgDn", accent_style()),
            Span::styled(" scroll  ", muted_style()),
            Span::styled("r", accent_style()),
            Span::styled(" refresh  ", muted_style()),
            Span::styled(state.status, info_style()),
        ];
        Paragraph::new(Line::from(spans)).style(base_style())
    };
    frame.render_widget(footer, area);
}

pub fn render_diff_choice_modal(frame: &mut Frame<'_>, area: Rect) {
    let width = 58.min(area.width.saturating_sub(4));
    let height = 9.min(area.height.saturating_sub(2));
    let modal_area = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal_area);
    let block = panel_block(" Open PR Diff ", true);
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let lines = vec![
        Line::from(Span::styled(
            "Choose how you want to view the diff:",
            panel_muted_style(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [g] ", panel_accent_style().add_modifier(Modifier::BOLD)),
            Span::styled(
                "Native TUI diff ",
                panel_text_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled("(in-terminal viewer)", panel_muted_style()),
        ]),
        Line::from(vec![
            Span::styled("  [b] ", panel_accent_style().add_modifier(Modifier::BOLD)),
            Span::styled(
                "Browser diff ",
                panel_text_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled("(rich interactive web viewer)", panel_muted_style()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Esc] ", panel_accent_style()),
            Span::styled("Cancel", panel_muted_style()),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).style(panel_style()), inner);
}

fn append_draft_preview(lines: &mut Vec<Line<'_>>, drafts: &[DraftComment]) {
    if drafts.is_empty() {
        return;
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled("Pending drafts:", accent_style())));
    for draft in drafts.iter().take(3) {
        lines.push(Line::from(vec![
            Span::styled(format!("- #{} ", draft.id), info_style()),
            Span::styled(description_preview(draft.raw.as_str()), text_style()),
        ]));
    }
}

fn pr_review_marker(running: bool, reviewed: bool) -> Span<'static> {
    if running {
        return Span::styled("[RUN] ", accent_style().add_modifier(Modifier::BOLD));
    }
    if reviewed {
        return Span::styled("[AI] ", success_style().add_modifier(Modifier::BOLD));
    }
    Span::styled("[--] ", muted_style())
}

fn author_label(author: &str) -> &str {
    let author = author.trim();
    if author.is_empty() {
        "unknown"
    } else {
        author
    }
}

fn append_ai_review_output(lines: &mut Vec<Line<'_>>, output: Option<&str>, width: usize) {
    let Some(output) = output.map(str::trim).filter(|output| !output.is_empty()) else {
        lines.push(Line::from(Span::styled(
            "No saved AI review output yet.",
            muted_style(),
        )));
        return;
    };
    lines.push(Line::from(Span::styled("Output:", accent_style())));
    for line in output.lines() {
        for wrapped in wrap_plain_line(line, width) {
            lines.push(Line::from(style_review_line(wrapped.as_str())));
        }
    }
}

fn append_diff_lines(lines: &mut Vec<Line<'_>>, diff: Option<&str>, width: usize) {
    let Some(diff) = diff.map(str::trim).filter(|diff| !diff.is_empty()) else {
        lines.push(Line::from(Span::styled("Diff not loaded.", muted_style())));
        return;
    };
    for line in diff.lines() {
        for wrapped in wrap_plain_line(line, width) {
            lines.push(Line::from(style_diff_line(wrapped.as_str())));
        }
    }
}

fn append_diff_file_lines(lines: &mut Vec<Line<'_>>, file: &DiffFileSection<'_>, width: usize) {
    for line in &file.lines {
        for wrapped in wrap_plain_line(line, width) {
            lines.push(Line::from(style_diff_line(wrapped.as_str())));
        }
    }
}

fn append_pager_diff_lines(lines: &mut Vec<Line<'_>>, output: &str) {
    let trimmed = output.trim_end();
    if trimmed.is_empty() {
        return;
    }
    for line in trimmed.lines() {
        lines.push(Line::from(ansi_spans(line)));
    }
}

fn append_split_diff_file_lines(
    lines: &mut Vec<Line<'_>>,
    file: &DiffFileSection<'_>,
    width: usize,
) {
    let column_width = width.saturating_sub(3).checked_div(2).unwrap_or(0).max(12);
    let mut index = 0;
    while index < file.lines.len() {
        let line = file.lines[index];
        if is_diff_meta_line(line) || line.starts_with("@@") {
            for wrapped in wrap_plain_line(line, width) {
                lines.push(Line::from(style_diff_line(wrapped.as_str())));
            }
            index += 1;
            continue;
        }

        if line.starts_with('-') && !line.starts_with("---") {
            let deletion_start = index;
            while index < file.lines.len()
                && file.lines[index].starts_with('-')
                && !file.lines[index].starts_with("---")
            {
                index += 1;
            }
            let addition_start = index;
            while index < file.lines.len()
                && file.lines[index].starts_with('+')
                && !file.lines[index].starts_with("+++")
            {
                index += 1;
            }
            let deletion_count = addition_start.saturating_sub(deletion_start);
            let addition_count = index.saturating_sub(addition_start);
            for row_index in 0..deletion_count.max(addition_count) {
                let left = file
                    .lines
                    .get(deletion_start + row_index)
                    .and_then(|line| line.strip_prefix('-'))
                    .unwrap_or("");
                let right = file
                    .lines
                    .get(addition_start + row_index)
                    .and_then(|line| line.strip_prefix('+'))
                    .unwrap_or("");
                push_split_diff_row(
                    lines,
                    left,
                    error_style(),
                    right,
                    success_style(),
                    column_width,
                );
            }
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            push_split_diff_row(
                lines,
                "",
                muted_style(),
                line.strip_prefix('+').unwrap_or(line),
                success_style(),
                column_width,
            );
            index += 1;
            continue;
        }

        let context = line.strip_prefix(' ').unwrap_or(line);
        push_split_diff_row(
            lines,
            context,
            text_style(),
            context,
            text_style(),
            column_width,
        );
        index += 1;
    }
}

fn push_split_diff_row<'a>(
    lines: &mut Vec<Line<'a>>,
    left: &str,
    left_style: Style,
    right: &str,
    right_style: Style,
    column_width: usize,
) {
    lines.push(Line::from(vec![
        Span::styled(fit_diff_cell(left, column_width), left_style),
        Span::styled(" | ", muted_style()),
        Span::styled(fit_diff_cell(right, column_width), right_style),
    ]));
}

fn fit_diff_cell(line: &str, width: usize) -> String {
    let width = width.max(1);
    let mut cell: String = line.chars().take(width).collect();
    if line.chars().count() > width {
        cell.pop();
        cell.push('~');
    }
    while cell.chars().count() < width {
        cell.push(' ');
    }
    cell
}

fn is_diff_meta_line(line: &str) -> bool {
    line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("---")
        || line.starts_with("+++")
        || line.starts_with("new file mode")
        || line.starts_with("deleted file mode")
        || line.starts_with("similarity index ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
}

fn ansi_spans(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut style = text_style();
    let mut chars = line.chars().peekable();

    while let Some(character) = chars.next() {
        if character == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            let mut sequence = String::new();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    if next == 'm' {
                        push_ansi_span(&mut spans, &mut current, style);
                        style = apply_sgr(sequence.as_str(), style);
                    }
                    break;
                }
                sequence.push(next);
            }
        } else {
            current.push(character);
        }
    }

    push_ansi_span(&mut spans, &mut current, style);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), text_style()));
    }
    spans
}

fn push_ansi_span(spans: &mut Vec<Span<'static>>, current: &mut String, style: Style) {
    if current.is_empty() {
        return;
    }
    spans.push(Span::styled(std::mem::take(current), style));
}

fn apply_sgr(sequence: &str, mut style: Style) -> Style {
    let codes = parse_sgr_codes(sequence);
    if codes.is_empty() {
        return text_style();
    }

    let mut index = 0;
    while index < codes.len() {
        let code = codes[index];
        match code {
            0 => style = text_style(),
            1 => style = style.add_modifier(Modifier::BOLD),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            22 => style = style.remove_modifier(Modifier::BOLD),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),
            30..=37 | 90..=97 => {
                style = style.fg(ansi_standard_color(code));
            }
            39 => style = style.fg(TEXT),
            40..=47 | 100..=107 => {
                style = style.bg(ansi_standard_bg_color(code));
            }
            49 => style = style.bg(SURFACE),
            38 | 48 => {
                if let Some((color, consumed)) = parse_extended_color(&codes[index..]) {
                    style = if code == 38 {
                        style.fg(color)
                    } else {
                        style.bg(color)
                    };
                    index += consumed;
                    continue;
                }
            }
            _ => {}
        }
        index += 1;
    }
    style
}

fn parse_sgr_codes(sequence: &str) -> Vec<u16> {
    if sequence.trim().is_empty() {
        return Vec::new();
    }
    sequence
        .split(';')
        .filter_map(|part| part.parse::<u16>().ok())
        .collect()
}

fn parse_extended_color(codes: &[u16]) -> Option<(Color, usize)> {
    match codes {
        [_, 2, red, green, blue, ..] => Some((
            Color::Rgb(
                (*red).min(255) as u8,
                (*green).min(255) as u8,
                (*blue).min(255) as u8,
            ),
            5,
        )),
        [_, 5, color, ..] => Some((ansi_256_color((*color).min(255) as u8), 3)),
        _ => None,
    }
}

fn ansi_standard_color(code: u16) -> Color {
    match code {
        30 => Color::Black,
        31 => ERROR,
        32 => SUCCESS,
        33 => ACCENT,
        34 => INFO,
        35 => Color::Magenta,
        36 => Color::Cyan,
        37 => TEXT,
        90 => MUTED,
        91 => Color::LightRed,
        92 => Color::LightGreen,
        93 => Color::LightYellow,
        94 => Color::LightBlue,
        95 => Color::LightMagenta,
        96 => Color::LightCyan,
        97 => Color::White,
        _ => TEXT,
    }
}

fn ansi_standard_bg_color(code: u16) -> Color {
    match code {
        40 => Color::Black,
        41 => Color::Rgb(63, 0, 1),
        42 => Color::Rgb(0, 40, 0),
        43 => Color::Rgb(64, 48, 0),
        44 => Color::Rgb(0, 24, 64),
        45 => Color::Rgb(48, 0, 64),
        46 => Color::Rgb(0, 48, 64),
        47 => Color::Gray,
        100 => Color::DarkGray,
        101 => Color::LightRed,
        102 => Color::LightGreen,
        103 => Color::LightYellow,
        104 => Color::LightBlue,
        105 => Color::LightMagenta,
        106 => Color::LightCyan,
        107 => Color::White,
        _ => SURFACE,
    }
}

fn ansi_256_color(code: u8) -> Color {
    match code {
        0 => Color::Black,
        1 => ERROR,
        2 => SUCCESS,
        3 => ACCENT,
        4 => INFO,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => TEXT,
        8 => MUTED,
        9 => Color::LightRed,
        10 => Color::LightGreen,
        11 => Color::LightYellow,
        12 => Color::LightBlue,
        13 => Color::LightMagenta,
        14 => Color::LightCyan,
        15 => Color::White,
        16..=231 => {
            let color = code - 16;
            let red = color / 36;
            let green = (color % 36) / 6;
            let blue = color % 6;
            Color::Rgb(
                color_cube_value(red),
                color_cube_value(green),
                color_cube_value(blue),
            )
        }
        232..=255 => {
            let value = 8 + (code - 232) * 10;
            Color::Rgb(value, value, value)
        }
    }
}

fn color_cube_value(value: u8) -> u8 {
    if value == 0 {
        0
    } else {
        55 + value * 40
    }
}

pub fn selected_diff_file_patch(diff: Option<&str>, selected_file: usize) -> Option<String> {
    let files = parse_diff_files(diff);
    let file = files.get(selected_file.min(files.len().saturating_sub(1)))?;
    Some(file.lines.join("\n") + "\n")
}

fn parse_diff_files(diff: Option<&str>) -> Vec<DiffFileSection<'_>> {
    let Some(diff) = diff.map(str::trim).filter(|diff| !diff.is_empty()) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    let mut current: Option<DiffFileSection<'_>> = None;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if let Some(file) = current.take() {
                files.push(file);
            }
            let (old_path, new_path) = parse_diff_git_paths(line);
            current = Some(DiffFileSection {
                old_path,
                new_path,
                lines: vec![line],
                additions: 0,
                deletions: 0,
            });
            continue;
        }

        let file = current.get_or_insert_with(|| DiffFileSection {
            old_path: "diff",
            new_path: "diff",
            lines: Vec::new(),
            additions: 0,
            deletions: 0,
        });
        if let Some(path) = line.strip_prefix("--- ") {
            file.old_path = normalize_diff_path(path);
        } else if let Some(path) = line.strip_prefix("+++ ") {
            file.new_path = normalize_diff_path(path);
        } else if line.starts_with("new file mode ") {
            file.old_path = "/dev/null";
        } else if line.starts_with("deleted file mode ") {
            file.new_path = "/dev/null";
        } else if line.starts_with('+') {
            file.additions += 1;
        } else if line.starts_with('-') {
            file.deletions += 1;
        }
        file.lines.push(line);
    }

    if let Some(file) = current {
        files.push(file);
    }
    files
}

fn parse_diff_git_paths(line: &str) -> (&str, &str) {
    let mut parts = line.split_whitespace();
    let _ = parts.next();
    let _ = parts.next();
    let old_path = parts.next().map(normalize_diff_path).unwrap_or("unknown");
    let new_path = parts.next().map(normalize_diff_path).unwrap_or(old_path);
    (old_path, new_path)
}

fn normalize_diff_path(path: &str) -> &str {
    let trimmed = path.trim();
    if trimmed == "/dev/null" {
        return trimmed;
    }
    trimmed
        .strip_prefix("a/")
        .or_else(|| trimmed.strip_prefix("b/"))
        .unwrap_or(trimmed)
}

fn diff_file_list_offset(file_count: usize, area_height: u16, selected_file: usize) -> usize {
    let visible_rows = usize::from(area_height.saturating_sub(2)).max(1);
    if selected_file < visible_rows {
        0
    } else {
        selected_file
            .saturating_sub(visible_rows - 1)
            .min(file_count.saturating_sub(visible_rows))
    }
}

fn compact_path(path: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if Span::raw(path).width() <= max_width {
        return path.to_string();
    }

    let components = path
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    let Some(filename) = components.last().copied() else {
        return truncate_start(path, max_width);
    };
    let filename_width = Span::raw(filename).width();
    if filename_width > max_width {
        return truncate_start(filename, max_width);
    }
    if filename_width == max_width {
        return filename.to_string();
    }

    let mut suffix = filename.to_string();
    for component in components[..components.len().saturating_sub(1)]
        .iter()
        .rev()
    {
        let candidate = format!("…/{component}/{suffix}");
        if Span::raw(candidate.as_str()).width() > max_width {
            break;
        }
        suffix = format!("{component}/{suffix}");
    }

    let compact = format!("…/{suffix}");
    if Span::raw(compact.as_str()).width() <= max_width {
        compact
    } else {
        truncate_start(path, max_width)
    }
}

fn truncate_start(value: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if Span::raw(value).width() <= max_width {
        return value.to_string();
    }

    let mut suffix = String::new();
    for grapheme in value.graphemes(true).rev() {
        let candidate = format!("…{grapheme}{suffix}");
        if Span::raw(candidate.as_str()).width() > max_width {
            break;
        }
        suffix.insert_str(0, grapheme);
    }
    format!("…{suffix}")
}

fn style_diff_line(line: &str) -> Vec<Span<'static>> {
    if line.starts_with('+') && !line.starts_with("+++") {
        return vec![Span::styled(line.to_string(), success_style())];
    }
    if line.starts_with('-') && !line.starts_with("---") {
        return vec![Span::styled(line.to_string(), error_style())];
    }
    if line.starts_with("@@") {
        return vec![Span::styled(
            line.to_string(),
            info_style().add_modifier(Modifier::BOLD),
        )];
    }
    if line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("---")
        || line.starts_with("+++")
        || line.starts_with("new file mode")
        || line.starts_with("deleted file mode")
    {
        return vec![Span::styled(line.to_string(), muted_style())];
    }
    vec![Span::styled(line.to_string(), text_style())]
}

fn append_description_lines<'a>(lines: &mut Vec<Line<'a>>, description: &str, width: usize) {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        lines.push(Line::from(Span::styled("No description.", muted_style())));
        return;
    }
    lines.push(Line::from(Span::styled("Description:", accent_style())));
    for line in trimmed.lines() {
        for wrapped in wrap_plain_line(line, width) {
            lines.push(Line::from(wrapped));
        }
    }
}

fn description_preview(description: &str) -> String {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return "No description.".to_string();
    }
    trimmed.lines().take(4).collect::<Vec<_>>().join(" ")
}

fn diff_preview(diff: Option<&str>) -> String {
    let Some(diff) = diff.map(str::trim).filter(|diff| !diff.is_empty()) else {
        return "Diff not loaded.".to_string();
    };
    let files = diff
        .lines()
        .filter(|line| line.starts_with("diff --git "))
        .count();
    let additions = diff
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count();
    let deletions = diff
        .lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("---"))
        .count();
    format!("Diff: {files} files, +{additions}/-{deletions}")
}

fn render_scrollbar(frame: &mut Frame<'_>, area: Rect, content_len: usize, offset: usize) {
    let inner_height = usize::from(area.height.saturating_sub(2));
    if content_len <= inner_height.max(1) {
        return;
    }
    let mut scrollbar_state = ScrollbarState::new(content_len).position(offset);
    frame.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .style(Style::default().fg(ACCENT).bg(PANEL)),
        area,
        &mut scrollbar_state,
    );
}

fn content_width(area: Rect) -> usize {
    usize::from(area.width.saturating_sub(3)).max(8)
}

fn clamped_scroll(content_len: usize, area_height: u16, offset: usize) -> usize {
    let visible_rows = usize::from(area_height.saturating_sub(2)).max(1);
    let max_offset = content_len.saturating_sub(visible_rows);
    offset.min(max_offset).min(usize::from(u16::MAX))
}

fn wrap_plain_line(line: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    if line.is_empty() {
        return vec![String::new()];
    }

    let mut rows = Vec::new();
    let mut remaining = line.trim_end().to_string();
    while remaining.chars().count() > width {
        let split_at = preferred_wrap_index(remaining.as_str(), width);
        let (head, tail) = split_at_char(remaining.as_str(), split_at);
        rows.push(head.trim_end().to_string());
        remaining = tail.trim_start().to_string();
        if remaining.is_empty() {
            break;
        }
    }
    rows.push(remaining);
    rows
}

fn preferred_wrap_index(line: &str, width: usize) -> usize {
    let mut last_break = None;
    for (index, character) in line.chars().enumerate() {
        if index >= width {
            break;
        }
        if character.is_whitespace() || matches!(character, '/' | '-' | ',' | '.') {
            last_break = Some(index + 1);
        }
    }
    last_break.unwrap_or(width)
}

fn split_at_char(line: &str, char_index: usize) -> (&str, &str) {
    if char_index == 0 {
        return ("", line);
    }
    let byte_index = line
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or_else(|| line.len());
    line.split_at(cmp::min(byte_index, line.len()))
}

fn style_review_line(line: &str) -> Vec<Span<'static>> {
    if line.starts_with("**[Critical]**") || line.starts_with("**[CRITICAL]**") {
        return vec![Span::styled(
            line.to_string(),
            error_style().add_modifier(Modifier::BOLD),
        )];
    }
    if line.starts_with("**[Major]**") || line.starts_with("**[MAJOR]**") {
        return vec![Span::styled(
            line.to_string(),
            accent_style().add_modifier(Modifier::BOLD),
        )];
    }
    if line.starts_with("**[Minor]**") || line.starts_with("**[MINOR]**") {
        return vec![Span::styled(
            line.to_string(),
            info_style().add_modifier(Modifier::BOLD),
        )];
    }
    if line.starts_with("**[Nit]**") || line.starts_with("**[NIT]**") {
        return vec![Span::styled(line.to_string(), muted_style())];
    }
    if line.starts_with("Fix:") {
        return vec![
            Span::styled("Fix:", success_style().add_modifier(Modifier::BOLD)),
            Span::styled(line.trim_start_matches("Fix:").to_string(), text_style()),
        ];
    }
    if line.starts_with('#') {
        return vec![Span::styled(
            line.to_string(),
            info_style().add_modifier(Modifier::BOLD),
        )];
    }
    vec![Span::styled(line.to_string(), text_style())]
}

fn loading_title(title: &str, state: &LoadState) -> String {
    if state.is_loading() {
        format!("{title} ↻")
    } else {
        title.to_string()
    }
}

fn loading_line(label: &str, tick: usize) -> Line<'static> {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    Line::from(vec![
        Span::styled(FRAMES[tick % FRAMES.len()], accent_style()),
        Span::styled(format!(" {label}..."), muted_style()),
    ])
}

pub(super) fn panel_block(title: impl Into<Line<'static>>, active: bool) -> Block<'static> {
    let title_style = if active {
        accent_style().add_modifier(Modifier::BOLD)
    } else {
        muted_style()
    };
    Block::default()
        .title(title.into().style(title_style))
        .borders(Borders::ALL)
        .border_style(if active {
            accent_style()
        } else {
            border_style()
        })
        .style(panel_style())
}

fn base_style() -> Style {
    Style::default().fg(TEXT).bg(SURFACE)
}

pub(super) fn panel_style() -> Style {
    Style::default().fg(TEXT).bg(PANEL)
}

pub(super) fn panel_accent_style() -> Style {
    Style::default().fg(ACCENT).bg(PANEL)
}

pub(super) fn panel_muted_style() -> Style {
    Style::default().fg(MUTED).bg(PANEL)
}

pub(super) fn panel_text_style() -> Style {
    Style::default().fg(TEXT).bg(PANEL)
}

pub(super) fn panel_info_style() -> Style {
    Style::default().fg(INFO).bg(PANEL)
}

pub(super) fn panel_success_style() -> Style {
    Style::default().fg(SUCCESS).bg(PANEL)
}

pub(super) fn panel_error_style() -> Style {
    Style::default().fg(ERROR).bg(PANEL)
}

fn border_style() -> Style {
    Style::default().fg(BORDER).bg(SURFACE)
}

fn accent_style() -> Style {
    Style::default().fg(ACCENT).bg(SURFACE)
}

fn muted_style() -> Style {
    Style::default().fg(MUTED).bg(SURFACE)
}

fn text_style() -> Style {
    Style::default().fg(TEXT).bg(SURFACE)
}

fn info_style() -> Style {
    Style::default().fg(INFO).bg(SURFACE)
}

fn success_style() -> Style {
    Style::default().fg(SUCCESS).bg(SURFACE)
}

fn error_style() -> Style {
    Style::default().fg(ERROR).bg(SURFACE)
}

fn branch_style() -> Style {
    Style::default().fg(INFO).bg(SURFACE)
}

fn provider_style(provider: ReviewProvider) -> Style {
    match provider {
        ReviewProvider::Bitbucket => accent_style(),
        ReviewProvider::Github => success_style(),
    }
}

fn status_style(status: &str) -> Style {
    match status {
        "OPEN" => success_style(),
        "MERGED" => info_style(),
        "DECLINED" | "SUPERSEDED" => error_style(),
        _ => text_style(),
    }
}

fn ai_review_status_style(status: AiReviewRunStatus) -> Style {
    match status {
        AiReviewRunStatus::Succeeded => success_style(),
        AiReviewRunStatus::Failed | AiReviewRunStatus::Cancelled => error_style(),
        AiReviewRunStatus::Running => accent_style(),
        AiReviewRunStatus::Idle => muted_style(),
    }
}

fn provider_label(provider: ReviewProvider) -> &'static str {
    match provider {
        ReviewProvider::Bitbucket => "bitbucket",
        ReviewProvider::Github => "github",
    }
}

fn ai_review_status_label(status: AiReviewRunStatus) -> &'static str {
    match status {
        AiReviewRunStatus::Idle => "idle",
        AiReviewRunStatus::Running => "running",
        AiReviewRunStatus::Succeeded => "succeeded",
        AiReviewRunStatus::Failed => "failed",
        AiReviewRunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{backend::TestBackend, Terminal};

    use super::*;

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    #[test]
    fn renders_empty_repository_state() {
        let backend = TestBackend::new(120, 20);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::Repositories,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: None,
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: None,
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::PullRequest,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Ready",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("No repositories configured"));
        assert!(text.contains("Configure repos"));
        assert!(text.contains("s settings"));
        assert!(text.contains("c draft p send x drop a AI"));
        assert!(text.contains("r reload"));
    }

    #[test]
    fn renders_selected_repository_detail() {
        let backend = TestBackend::new(140, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let repos = vec![RepoRef {
            provider: ReviewProvider::Github,
            workspace: "delaudio".to_string(),
            repo: "norn".to_string(),
            local_path: Some("/tmp/lachesi".to_string()),
        }];
        let pull_requests = vec![PullRequestSummary {
            id: 12,
            title: "Add terminal UI".to_string(),
            author_display_name: "fdg".to_string(),
            author_account_id: None,
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            comment_count: 2,
            created_on: String::new(),
            updated_on: String::new(),
            reviewers: Vec::new(),
        }];

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &repos,
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &pull_requests,
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: None,
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: None,
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::PullRequest,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Ready",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("github delaudio/norn"));
        assert!(text.contains("/tmp/lachesi"));
        assert!(text.contains("PR filter"));
        assert!(text.contains("[Open]"));
        assert!(text.contains("#12 feature/tui -> main"));
        assert!(text.contains("by fdg"));
    }

    #[test]
    fn renders_ai_review_markers_in_pull_request_list() {
        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let pull_requests = vec![
            PullRequestSummary {
                id: 12,
                title: "Reviewed".to_string(),
                author_display_name: String::new(),
                author_account_id: None,
                source_branch: "feature/reviewed".to_string(),
                destination_branch: "main".to_string(),
                state: "OPEN".to_string(),
                draft: false,
                comment_count: 0,
                created_on: String::new(),
                updated_on: String::new(),
                reviewers: Vec::new(),
            },
            PullRequestSummary {
                id: 13,
                title: "Pending".to_string(),
                author_display_name: String::new(),
                author_account_id: None,
                source_branch: "feature/pending".to_string(),
                destination_branch: "main".to_string(),
                state: "OPEN".to_string(),
                draft: false,
                comment_count: 0,
                created_on: String::new(),
                updated_on: String::new(),
                reviewers: Vec::new(),
            },
            PullRequestSummary {
                id: 14,
                title: "Running".to_string(),
                author_display_name: String::new(),
                author_account_id: None,
                source_branch: "feature/running".to_string(),
                destination_branch: "main".to_string(),
                state: "OPEN".to_string(),
                draft: false,
                comment_count: 0,
                created_on: String::new(),
                updated_on: String::new(),
                reviewers: Vec::new(),
            },
        ];

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &pull_requests,
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: None,
                        comments: &[],
                        ai_reviewed_pr_ids: &[12],
                        ai_review_running_pr_ids: &[14],
                        diff: None,
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::PullRequest,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Ready",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("[AI] #12"));
        assert!(text.contains("[--] #13"));
        assert!(text.contains("[RUN] #14"));
    }

    #[test]
    fn renders_loaded_pull_request_detail_and_diff_summary() {
        let backend = TestBackend::new(140, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };
        let comments = vec![PrComment {
            id: "1".to_string(),
            parent_id: None,
            content_raw: "Looks good".to_string(),
            content_html: None,
            user_display_name: "reviewer".to_string(),
            created_on: String::new(),
            deleted: false,
            inline: None,
        }];

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &comments,
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some("diff --git a/a b/a\n+new\n-old\n"),
                        drafts: &[DraftComment {
                            id: 1,
                            raw: "Please check this.".to_string(),
                        }],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::PullRequest,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("#12 Add terminal UI"));
        assert!(text.contains("Author: fdg"));
        assert!(text.contains("1 comments"));
        assert!(text.contains("Drafts: 1 pending"));
        assert!(text.contains("Diff: 1 files, +1/-1"));
    }

    #[test]
    fn renders_active_composer_text() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: None,
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: None,
                        drafts: &[],
                        composer: Some("pending thought"),
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::PullRequest,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Composing",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("Draft: pending thought"));
    }

    #[test]
    fn maps_mouse_clicks_to_repository_and_pull_request_rows() {
        let repos = vec![
            RepoRef {
                provider: ReviewProvider::Github,
                workspace: "delaudio".to_string(),
                repo: "norn".to_string(),
                local_path: None,
            },
            RepoRef {
                provider: ReviewProvider::Bitbucket,
                workspace: "team".to_string(),
                repo: "api".to_string(),
                local_path: None,
            },
        ];
        let pull_requests = vec![
            PullRequestSummary {
                id: 12,
                title: "One".to_string(),
                author_display_name: String::new(),
                author_account_id: None,
                source_branch: "one".to_string(),
                destination_branch: "main".to_string(),
                state: "OPEN".to_string(),
                draft: false,
                comment_count: 0,
                created_on: String::new(),
                updated_on: String::new(),
                reviewers: Vec::new(),
            },
            PullRequestSummary {
                id: 13,
                title: "Two".to_string(),
                author_display_name: String::new(),
                author_account_id: None,
                source_branch: "two".to_string(),
                destination_branch: "main".to_string(),
                state: "OPEN".to_string(),
                draft: false,
                comment_count: 0,
                created_on: String::new(),
                updated_on: String::new(),
                reviewers: Vec::new(),
            },
        ];
        let state = TuiState {
            repos: &repos,
            selected_repo: 0,
            focus: FocusPane::Repositories,
            pull_requests: &pull_requests,
            pr_filter: PrListFilter::Open,
            selected_pr: 0,
            detail: None,
            comments: &[],
            ai_reviewed_pr_ids: &[],
            ai_review_running_pr_ids: &[],
            diff: None,
            drafts: &[],
            composer: None,
            ai_review: None,
            ai_review_output: None,
            detail_view: DetailView::PullRequest,
            detail_scroll: 0,
            ai_review_scroll: 0,
            diff_scroll: 0,
            selected_diff_file: 0,
            diff_view_mode: DiffViewMode::Unified,
            rendered_diff_output: None,
            image_diff: None,
            image_protocol: "unsupported",
            loading: LoadingView::idle(),
            error: None,
            status: "Ready",
            diff_prompt_open: false,
        };

        assert_eq!(
            mouse_target(Rect::new(0, 0, 100, 24), 2, 5, state),
            Some(MouseTarget::Repository(1))
        );
        assert_eq!(
            mouse_target(Rect::new(0, 0, 100, 24), 38, 5, state),
            Some(MouseTarget::PullRequest(1))
        );
    }

    #[test]
    fn maps_mouse_clicks_to_pull_request_filter_segments() {
        let state = TuiState {
            repos: &[],
            selected_repo: 0,
            focus: FocusPane::Repositories,
            pull_requests: &[],
            pr_filter: PrListFilter::Open,
            selected_pr: 0,
            detail: None,
            comments: &[],
            ai_reviewed_pr_ids: &[],
            ai_review_running_pr_ids: &[],
            diff: None,
            drafts: &[],
            composer: None,
            ai_review: None,
            ai_review_output: None,
            detail_view: DetailView::PullRequest,
            detail_scroll: 0,
            ai_review_scroll: 0,
            diff_scroll: 0,
            selected_diff_file: 0,
            diff_view_mode: DiffViewMode::Unified,
            rendered_diff_output: None,
            image_diff: None,
            image_protocol: "unsupported",
            loading: LoadingView::idle(),
            error: None,
            status: "Ready",
            diff_prompt_open: false,
        };

        assert_eq!(
            mouse_target(Rect::new(0, 0, 100, 24), 2, 19, state),
            Some(MouseTarget::PrFilter(PrListFilter::Open))
        );
        assert_eq!(
            mouse_target(Rect::new(0, 0, 100, 24), 14, 19, state),
            Some(MouseTarget::PrFilter(PrListFilter::Draft))
        );
        assert_eq!(
            mouse_target(Rect::new(0, 0, 100, 24), 26, 19, state),
            Some(MouseTarget::PrFilter(PrListFilter::Merged))
        );
    }

    #[test]
    fn compact_path_preserves_the_filename_and_nearest_directories() {
        let path = "apps/procurement-frontend/src/app/dashboard/suppliers/SupplierDetail.tsx";

        let compact = compact_path(path, 37);

        assert!(compact.starts_with("…/"));
        assert!(compact.ends_with("suppliers/SupplierDetail.tsx"));
        assert!(Span::raw(compact.as_str()).width() <= 37);
    }

    #[test]
    fn compact_path_truncates_a_filename_from_the_start_when_needed() {
        let compact = compact_path("screenshots/SupplierDetailV2--Panoramica.png", 20);

        assert!(compact.starts_with('…'));
        assert!(compact.ends_with("Panoramica.png"));
        assert!(Span::raw(compact.as_str()).width() <= 20);
    }

    #[test]
    fn compact_path_preserves_a_filename_that_exactly_fits() {
        let filename = "Supplier.tsx";

        assert_eq!(
            compact_path(
                "dashboard/suppliers/Supplier.tsx",
                Span::raw(filename).width()
            ),
            filename
        );
    }

    #[test]
    fn compact_path_does_not_split_unicode_graphemes() {
        let compact = compact_path("screenshots/👩‍💻-supplier-review.png", 17);

        assert!(!compact.contains('\u{200d}') || compact.contains("👩‍💻"));
        assert!(Span::raw(compact.as_str()).width() <= 17);
    }

    #[test]
    fn compact_path_leaves_short_paths_unchanged() {
        assert_eq!(compact_path("src/App.tsx", 20), "src/App.tsx");
    }

    #[test]
    fn renders_ai_review_output_view() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some("diff --git a/a b/a\n+new\n-old\n"),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: Some("## Summary\nReview result body."),
                        detail_view: DetailView::AiReview,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("AI review"));
        assert!(text.contains("Review result body"));
        assert!(text.contains("Review in the terminal."));
        assert!(text.contains("Diff: 1 files"));
    }

    #[test]
    fn renders_pull_request_diff_view() {
        let backend = TestBackend::new(120, 48);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::Diff,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some(
                            "diff --git a/src/App.tsx b/src/App.tsx\n@@ -1 +1 @@\n-old\n+new\n",
                        ),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::Diff,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("Diff"));
        assert!(text.contains("#12 Add terminal UI"));
        assert!(text.contains("feature/tui -> main"));
        assert!(text.contains("diff --git a/src/App.tsx"));
        assert!(text.contains("+new"));
    }

    #[test]
    fn renders_split_pull_request_diff_view() {
        let backend = TestBackend::new(120, 48);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::Diff,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some(
                            "diff --git a/src/App.tsx b/src/App.tsx\n@@ -1 +1 @@\n-old\n+new\n",
                        ),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::Diff,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Split,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("side-by-side"));
        assert!(text.contains("old"));
        assert!(text.contains(" | "));
        assert!(text.contains("new"));
    }

    #[test]
    fn renders_pager_diff_output_with_ansi_styles() {
        let backend = TestBackend::new(120, 48);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::Diff,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some(
                            "diff --git a/src/App.tsx b/src/App.tsx\n@@ -1 +1 @@\n-old\n+new\n",
                        ),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::Diff,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Split,
                        rendered_diff_output: Some(
                            "\u{1b}[31m-old\u{1b}[0m\n\u{1b}[32m+new\u{1b}[0m\n",
                        ),
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("-old"));
        assert!(text.contains("+new"));
    }

    #[test]
    fn parses_rgb_ansi_spans() {
        let spans = ansi_spans("\u{1b}[38;2;1;2;3mnew\u{1b}[0m");

        assert_eq!(spans[0].content.as_ref(), "new");
        assert_eq!(spans[0].style.fg, Some(Color::Rgb(1, 2, 3)));
    }

    #[test]
    fn scrolls_ai_review_output_without_hiding_pull_request_detail() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };
        let output = (0..32)
            .map(|index| format!("Review output line {index:02}"))
            .collect::<Vec<_>>()
            .join("\n");

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some("diff --git a/a b/a\n+new\n-old\n"),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: Some(&output),
                        detail_view: DetailView::AiReview,
                        detail_scroll: 0,
                        ai_review_scroll: 10,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("Review in the terminal."));
        assert!(text.contains("Review output line 10"));
        assert!(!text.contains("Review output line 00"));
    }

    #[test]
    fn scrolls_wrapped_ai_review_output_rows() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };
        let output = format!("{}tail-marker", "wrapped review phrase ".repeat(80));

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some("diff --git a/a b/a\n+new\n-old\n"),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: Some(&output),
                        detail_view: DetailView::AiReview,
                        detail_scroll: 0,
                        ai_review_scroll: usize::MAX,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("Review in the terminal."));
        assert!(text.contains("tail-marker"));
        assert!(!text.contains("Output:"));
    }

    #[test]
    fn clamps_large_ai_review_scroll_offsets() {
        let backend = TestBackend::new(100, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let detail = PullRequestDetail {
            id: 12,
            title: "Add terminal UI".to_string(),
            description_raw: "Review in the terminal.".to_string(),
            state: "OPEN".to_string(),
            draft: false,
            author_display_name: "fdg".to_string(),
            reviewers: Vec::new(),
            source_branch: "feature/tui".to_string(),
            destination_branch: "main".to_string(),
            source_commit_hash: None,
            destination_commit_hash: None,
            created_on: String::new(),
            updated_on: String::new(),
        };
        let output = (0..20)
            .map(|index| format!("Review output line {index:02}"))
            .collect::<Vec<_>>()
            .join("\n");

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::PullRequests,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: Some(&detail),
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: None,
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: Some(&output),
                        detail_view: DetailView::AiReview,
                        detail_scroll: usize::MAX,
                        ai_review_scroll: usize::MAX,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: None,
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("Review output line 18"));
    }

    #[test]
    fn renders_image_metadata_fallback_without_a_terminal_protocol() {
        use crate::tui::image_diff::{
            FailedImageVersion, ImageChangeKind, ImageDiffCandidate, ImageSide,
        };

        let backend = TestBackend::new(120, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let diff = "diff --git a/public/new.png b/public/new.png\nnew file mode 100644\nBinary files /dev/null and b/public/new.png differ\n";
        let image = ImageDiffState {
            selected_file: 0,
            candidate: ImageDiffCandidate {
                kind: ImageChangeKind::Added,
                old_path: None,
                new_path: Some("public/new.png".to_string()),
            },
            selected_side: ImageSide::New,
            old: None,
            new: Some(ImageVersionState::Failed(FailedImageVersion {
                path: "public/new.png".to_string(),
                side: ImageSide::New,
                error: "Could not decode image: corrupt PNG".to_string(),
            })),
        };

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::Diff,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: None,
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some(diff),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::Diff,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: Some(&image),
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("[A] public/new.png"));
        assert!(text.contains("[ADDED] public/new.png"));
        assert!(text.contains("metadata unavailable"));
        assert!(text.contains("corrupt PNG"));
        assert!(text.contains("Image preview unavailable."));
    }

    #[test]
    fn renders_wide_modified_images_with_base_and_changed_columns() {
        use std::io::Cursor;

        use crate::services::bitbucket::PrFilePreview;
        use crate::tui::image_diff::{ImageChangeKind, ImageDiffCandidate, ImageSide};
        use base64::{engine::general_purpose::STANDARD, Engine};
        use image::{DynamicImage, ImageFormat};

        let backend = TestBackend::new(220, 24);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut encoded = Cursor::new(Vec::new());
        DynamicImage::new_rgba8(1, 1)
            .write_to(&mut encoded, ImageFormat::Png)
            .expect("encode png");
        let bytes = encoded.into_inner();
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(&bytes));
        let size = bytes.len();
        let candidate = ImageDiffCandidate {
            kind: ImageChangeKind::Modified,
            old_path: Some("public/before.png".to_string()),
            new_path: Some("public/after.png".to_string()),
        };
        let image = ImageDiffState::load(0, candidate, |_side, path| {
            Ok(PrFilePreview {
                path: path.to_string(),
                mime_type: "image/png".to_string(),
                data_url: data_url.clone(),
                size,
            })
        });
        assert!(image
            .comparison_areas(diff_image_area_for_area(Rect::new(0, 0, 220, 24), false))
            .is_some());

        terminal
            .draw(|frame| {
                render(
                    frame,
                    TuiState {
                        repos: &[],
                        selected_repo: 0,
                        focus: FocusPane::Diff,
                        pull_requests: &[],
                        pr_filter: PrListFilter::Open,
                        selected_pr: 0,
                        detail: None,
                        comments: &[],
                        ai_reviewed_pr_ids: &[],
                        ai_review_running_pr_ids: &[],
                        diff: Some("Binary files differ"),
                        drafts: &[],
                        composer: None,
                        ai_review: None,
                        ai_review_output: None,
                        detail_view: DetailView::Diff,
                        detail_scroll: 0,
                        ai_review_scroll: 0,
                        diff_scroll: 0,
                        selected_diff_file: 0,
                        diff_view_mode: DiffViewMode::Unified,
                        rendered_diff_output: None,
                        image_diff: Some(&image),
                        image_protocol: "unsupported",
                        loading: LoadingView::idle(),
                        error: None,
                        status: "Loaded",
                        diff_prompt_open: false,
                    },
                );
            })
            .expect("draw");

        let text = buffer_text(&terminal);
        assert!(text.contains("BASE"));
        assert!(text.contains("CHANGED"));
        assert!(text.contains("base: PNG"));
        assert!(text.contains("changed: PNG"));
        assert_eq!(image.selected_side, ImageSide::New);
    }
}
