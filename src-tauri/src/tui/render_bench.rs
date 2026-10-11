// Test-only module. Archive this harness before cutover.
use super::*;
use ratatui::{backend::TestBackend, Terminal};

fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchInputs {
    schema_version: u32,
    width: u16,
    height: u16,
    cases: Vec<BenchCase>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchCase {
    name: String,
    files: Vec<BenchFile>,
    selected_file: usize,
}

#[derive(serde::Deserialize)]
struct BenchFile {
    path: String,
    patch: String,
}

#[test]
#[ignore = "opt-in rendering benchmark; requires generated shared inputs, no configuration/provider access"]
fn matched_rendering_benchmark() {
    use sha2::Digest;
    use std::time::Instant;
    let input = std::env::var("NORN_RENDER_BENCH_INPUT")
        .expect("set NORN_RENDER_BENCH_INPUT to shared input JSON");
    let bytes = std::fs::read(input).expect("read benchmark input");
    assert!(bytes.len() <= 2 * 1024 * 1024, "benchmark input bound");
    let inputs: BenchInputs = serde_json::from_slice(&bytes).expect("shared benchmark input");
    assert_eq!(inputs.schema_version, 1);
    assert_eq!((inputs.width, inputs.height), (120, 30));
    let repos = vec![RepoRef {
        provider: ReviewProvider::Github,
        workspace: "acme".into(),
        repo: "payments".into(),
        local_path: None,
    }];
    let mut results = Vec::new();
    for case in inputs.cases {
        assert!(!case.files.is_empty() && case.files.len() <= 1000);
        assert!(case.selected_file < case.files.len());
        let patch = case
            .files
            .iter()
            .map(|file| file.patch.as_str())
            .collect::<Vec<_>>()
            .join("");
        assert!(patch.len() <= 1024 * 1024);
        let mut terminal = Terminal::new(TestBackend::new(inputs.width, inputs.height))
            .expect("native test terminal");
        let state = TuiState {
            repos: &repos,
            visible_repo_indices: &[0],
            repo_eligibility_loading: false,
            selected_repo: 0,
            focus: FocusPane::Diff,
            pull_requests: &[],
            local_snapshot: None,
            pr_filter: PrListFilter::Open,
            selected_pr: 0,
            detail: None,
            comments: &[],
            ai_reviewed_pr_ids: &[],
            ai_review_running_pr_ids: &[],
            diff: Some(&patch),
            drafts: &[],
            composer: None,
            ai_review: None,
            ai_review_output: None,
            detail_view: DetailView::Diff,
            detail_scroll: 0,
            ai_review_scroll: 0,
            diff_scroll: 0,
            selected_diff_file: case.selected_file,
            diff_view_mode: DiffViewMode::Unified,
            rendered_diff_output: None,
            image_diff: None,
            image_protocol: "unsupported",
            loading: LoadingView::idle(),
            error: None,
            status: "Rendering benchmark",
            diff_prompt_open: false,
        };
        let mut measurements = Vec::new();
        for mode in [DiffViewMode::Unified, DiffViewMode::Split] {
            let start = Instant::now();
            terminal
                .draw(|frame| {
                    render(
                        frame,
                        TuiState {
                            diff_view_mode: mode,
                            ..state
                        },
                    )
                })
                .expect("draw diff");
            let first_frame_ms = start.elapsed().as_secs_f64() * 1000.0;
            // A file-list label alone does not prove the actual diff was drawn.
            let text = buffer_text(&terminal);
            if case.name != "real-norn-backend" {
                assert!(text.contains("RENDER_BENCH"), "native diff body missing");
            }
            assert!(text.contains("Diff *"));
            if case.name == "real-norn-backend" {
                assert!(
                    text.contains("MAX_FRAME_BYTES"),
                    "real native diff body missing"
                );
            }
            let mut times = Vec::new();
            let areas = vertical_areas().split(Rect::new(0, 0, inputs.width, inputs.height));
            let panels = diff_page_areas().split(areas[1]);
            let diff_area = panels[1];
            let files = parse_diff_files(Some(&patch));
            let selected = &files[case.selected_file];
            let mut lines = Vec::new();
            match mode {
                DiffViewMode::Unified => {
                    append_diff_file_lines(&mut lines, selected, content_width(diff_area))
                }
                DiffViewMode::Split => {
                    append_split_diff_file_lines(&mut lines, selected, content_width(diff_area))
                }
            }
            let content_rows = lines.len() + 2;
            let max_scroll = clamped_scroll(content_rows, diff_area.height, usize::MAX);
            for sample in 0..100 {
                let start = Instant::now();
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            TuiState {
                                diff_view_mode: mode,
                                diff_scroll: sample * max_scroll / 99,
                                ..state
                            },
                        )
                    })
                    .expect("draw scroll");
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            times.sort_by(f64::total_cmp);
            measurements.push(serde_json::json!({"mode":if mode==DiffViewMode::Unified{"unified"}else{"split"},"firstFrameMs":first_frame_ms,"scrollP95Ms":times[94],"samples":100,"contentRows":content_rows}));
        }
        results.push(serde_json::json!({"case":case.name,"files":case.files.len(),"selectedPath":case.files[case.selected_file].path,"selectedPatchBytes":case.files[case.selected_file].patch.len(),"totalReviewBytes":patch.len(),"processPeakRssMiB":process_peak_rss_mib(),"measurements":measurements}));
    }
    let result = serde_json::json!({"inputSha256":format!("{:x}",sha2::Sha256::digest(&bytes)),"renderer":"ratatui","width":inputs.width,"height":inputs.height,"platform":std::env::consts::OS,"arch":std::env::consts::ARCH,"profile":if cfg!(debug_assertions){"debug"}else{"release"},"results":results});
    let output = serde_json::to_vec_pretty(&result).expect("serialize measurements");
    if let Ok(path) = std::env::var("NORN_RENDER_BENCH_OUTPUT") {
        std::fs::write(path, output).expect("write benchmark output");
    } else {
        println!("{}", String::from_utf8(output).expect("JSON utf8"));
    }
}

fn process_peak_rss_mib() -> Option<f64> {
    #[cfg(unix)]
    {
        let mut usage = std::mem::MaybeUninit::<libc::rusage>::uninit();
        // SAFETY: getrusage initializes a valid writable rusage on success.
        if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
            return None;
        }
        let usage = unsafe { usage.assume_init() };
        #[cfg(target_os = "macos")]
        let bytes = usage.ru_maxrss as f64;
        #[cfg(not(target_os = "macos"))]
        let bytes = usage.ru_maxrss as f64 * 1024.0;
        Some(bytes / (1024.0 * 1024.0))
    }
    #[cfg(not(unix))]
    {
        None
    }
}
