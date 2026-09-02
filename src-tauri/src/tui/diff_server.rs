use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    process::{Command, ExitStatus},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    thread,
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

use crate::{
    config::ReviewProvider,
    services::bitbucket::{
        get_diffstat_native, get_pr_diff_native, get_pr_file_preview_native, DiffstatEntry,
    },
};

const MAX_REQUEST_HEAD_BYTES: usize = 16 * 1024;
const SESSION_TOKEN_BYTES: usize = 32;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDiffState {
    pub version: usize,
    pub provider: Option<ReviewProvider>,
    pub workspace: String,
    pub repo: String,
    pub pr_id: u32,
    pub pr_title: String,
    pub pr_author: String,
    pub source_branch: String,
    pub target_branch: String,
    pub diff: Option<String>,
    pub diffstat: Option<Vec<DiffstatEntry>>,
    pub population_failed: bool,
}

pub struct WebDiffServer {
    pub port: u16,
    session_token: String,
    state: Arc<RwLock<WebDiffState>>,
    shutdown: Arc<AtomicBool>,
}

impl WebDiffServer {
    pub fn start(state: Arc<RwLock<WebDiffState>>) -> Result<Self, String> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("Failed to bind web diff server: {e}"))?;

        let resolved_port = listener
            .local_addr()
            .map(|addr| addr.port())
            .map_err(|e| format!("Failed to resolve web diff server port: {e}"))?;
        let session_token = generate_session_token()?;

        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread_state = Arc::clone(&state);
        let thread_token: Arc<str> = Arc::from(session_token.as_str());
        let populate_lock = Arc::new(Mutex::new(()));

        thread::spawn(move || {
            let _ = listener.set_nonblocking(true);
            while !thread_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let st = Arc::clone(&thread_state);
                        let token = Arc::clone(&thread_token);
                        let fetch_lock = Arc::clone(&populate_lock);
                        thread::spawn(move || {
                            let _ = handle_connection(stream, st, &token, fetch_lock);
                        });
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        });

        Ok(Self {
            port: resolved_port,
            session_token,
            state,
            shutdown,
        })
    }

    pub fn update_pr(&self, mut next: WebDiffState) {
        if let Ok(mut lock) = self.state.write() {
            next.version = lock.version;
            let content_changed = lock.provider != next.provider
                || lock.workspace != next.workspace
                || lock.repo != next.repo
                || lock.pr_id != next.pr_id
                || lock.diff != next.diff
                || lock.pr_title != next.pr_title
                || lock.pr_author != next.pr_author
                || lock.source_branch != next.source_branch
                || lock.target_branch != next.target_branch;
            if !content_changed && next.diffstat.is_none() {
                next.diffstat = lock.diffstat.clone();
            }
            let changed = *lock != next;
            if changed {
                next.version += 1;
            }
            *lock = next;
        }
    }

    pub fn url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/session/{}/",
            self.port, self.session_token
        )
    }
}

fn generate_session_token() -> Result<String, String> {
    let mut token = [0u8; SESSION_TOKEN_BYTES];
    getrandom::getrandom(&mut token)
        .map_err(|error| format!("Failed to create web diff session: {error}"))?;
    Ok(hex::encode(token))
}

impl Drop for WebDiffServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

pub fn open_browser_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        browser_open_result(Command::new("open").arg(url).status())
    }

    #[cfg(target_os = "linux")]
    {
        browser_open_result(Command::new("xdg-open").arg(url).status())
    }

    #[cfg(target_os = "windows")]
    {
        browser_open_result(Command::new("cmd").args(["/c", "start", url]).status())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err("Unsupported operating system for opening browser".to_string())
    }
}

fn browser_open_result(result: std::io::Result<ExitStatus>) -> Result<(), String> {
    let status = result.map_err(|error| format!("Failed to open browser: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to open browser: opener exited with {status}"
        ))
    }
}

fn handle_connection(
    mut stream: TcpStream,
    state: Arc<RwLock<WebDiffState>>,
    session_token: &str,
    populate_lock: Arc<Mutex<()>>,
) -> std::io::Result<()> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let request_head = match read_request_head(&mut stream) {
        Ok(Some(request)) => request,
        Ok(None) => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::InvalidData => {
            return write_empty_response(&mut stream, "431 Request Header Fields Too Large")
        }
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
            return write_empty_response(&mut stream, "400 Bad Request")
        }
        Err(error) => return Err(error),
    };

    let request = match std::str::from_utf8(&request_head) {
        Ok(request) => request,
        Err(_) => return write_empty_response(&mut stream, "400 Bad Request"),
    };
    let first_line = request.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let full_path = parts.next().unwrap_or("");

    if method.is_empty() || full_path.is_empty() {
        return write_empty_response(&mut stream, "400 Bad Request");
    }

    if method != "GET" && method != "HEAD" {
        return write_empty_response(&mut stream, "405 Method Not Allowed");
    }
    let head_only = method == "HEAD";

    let (path, query) = match full_path.split_once('?') {
        Some((p, q)) => (p, q),
        None => (full_path, ""),
    };
    let Some(route) = authenticated_route(path, session_token) else {
        return write_empty_response(&mut stream, "404 Not Found");
    };

    match route {
        "/" | "/index.html" => {
            write_response(
                &mut stream,
                "200 OK",
                "text/html; charset=utf-8",
                "no-store",
                HTML_PAGE.as_bytes(),
                head_only,
            )?;
        }
        "/api/state" | "/api/diff" => {
            let known_version = parse_query(query)
                .get("version")
                .and_then(|version| version.parse::<usize>().ok());
            let Some(state_data) = get_or_populate_state(&state, &populate_lock, known_version)
            else {
                return write_empty_response(&mut stream, "204 No Content");
            };
            let json = serde_json::to_string(&state_data).unwrap_or_else(|_| "{}".to_string());
            write_response(
                &mut stream,
                "200 OK",
                "application/json; charset=utf-8",
                "no-store",
                json.as_bytes(),
                head_only,
            )?;
        }
        "/api/file-preview" => {
            let query_map = parse_query(query);
            let file_path = query_map.get("path").cloned().unwrap_or_default();
            let side = query_map
                .get("side")
                .cloned()
                .unwrap_or_else(|| "new".to_string());
            let state_data = get_or_populate_state(&state, &populate_lock, None)
                .expect("requests without a known version always return state");

            if state_data.workspace.is_empty()
                || state_data.repo.is_empty()
                || state_data.pr_id == 0
                || file_path.is_empty()
                || (side != "old" && side != "new")
            {
                return write_empty_response(&mut stream, "400 Bad Request");
            }
            if !is_allowed_preview_path(state_data.diffstat.as_deref(), &file_path, &side) {
                return write_empty_response(&mut stream, "404 Not Found");
            }
            if !is_raster_preview_path(&file_path) {
                return write_empty_response(&mut stream, "415 Unsupported Media Type");
            }

            match get_pr_file_preview_native(
                state_data.provider,
                &state_data.workspace,
                &state_data.repo,
                state_data.pr_id,
                &file_path,
                &side,
            ) {
                Ok(preview) => {
                    let Some((mime_part, base64_data)) = preview.data_url.split_once(',') else {
                        return write_empty_response(&mut stream, "415 Unsupported Media Type");
                    };
                    let mime_type = mime_part
                        .strip_prefix("data:")
                        .and_then(|p| p.strip_suffix(";base64"))
                        .unwrap_or(&preview.mime_type);
                    let Some(safe_mime_type) =
                        safe_preview_mime_type(mime_type, &preview.mime_type)
                    else {
                        return write_empty_response(&mut stream, "415 Unsupported Media Type");
                    };
                    let Ok(bytes) = STANDARD.decode(base64_data.trim()) else {
                        return write_empty_response(&mut stream, "415 Unsupported Media Type");
                    };
                    write_response(
                        &mut stream,
                        "200 OK",
                        safe_mime_type,
                        "no-store",
                        &bytes,
                        head_only,
                    )?;
                }
                Err(error) => {
                    let json = serde_json::json!({ "error": error }).to_string();
                    write_response(
                        &mut stream,
                        "404 Not Found",
                        "application/json; charset=utf-8",
                        "no-store",
                        json.as_bytes(),
                        head_only,
                    )?;
                }
            }
        }
        "/api/health" => {
            let body = r#"{"status":"ok"}"#;
            write_response(
                &mut stream,
                "200 OK",
                "application/json",
                "no-store",
                body.as_bytes(),
                head_only,
            )?;
        }
        _ => {
            write_empty_response(&mut stream, "404 Not Found")?;
        }
    }

    Ok(())
}

fn read_request_head<R: Read>(reader: &mut R) -> std::io::Result<Option<Vec<u8>>> {
    let mut request = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        if request.len() >= MAX_REQUEST_HEAD_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "HTTP request headers exceed the configured limit",
            ));
        }
        let remaining = MAX_REQUEST_HEAD_BYTES - request.len();
        let read_limit = remaining.min(chunk.len());
        let bytes_read = reader.read(&mut chunk[..read_limit])?;
        if bytes_read == 0 {
            if request.is_empty() {
                return Ok(None);
            }
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "HTTP request ended before the header terminator",
            ));
        }
        request.extend_from_slice(&chunk[..bytes_read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            return Ok(Some(request));
        }
    }
}

fn authenticated_route<'a>(path: &'a str, session_token: &str) -> Option<&'a str> {
    let session_path = path.strip_prefix("/session/")?;
    let token_end = session_path.find('/').unwrap_or(session_path.len());
    let candidate = &session_path[..token_end];
    if !constant_time_token_eq(candidate, session_token) {
        return None;
    }
    let route = &session_path[token_end..];
    match route {
        "" => Some("/"),
        route if route.starts_with('/') => Some(route),
        _ => None,
    }
}

fn constant_time_token_eq(candidate: &str, expected: &str) -> bool {
    if candidate.len() != expected.len() {
        return false;
    }
    let difference = candidate
        .as_bytes()
        .iter()
        .zip(expected.as_bytes())
        .fold(0u8, |difference, (left, right)| difference | (left ^ right));
    std::hint::black_box(difference) == 0
}

fn safe_preview_mime_type(candidate: &str, fallback: &str) -> Option<&'static str> {
    for mime_type in [candidate, fallback] {
        match mime_type {
            "image/png" => return Some("image/png"),
            "image/jpeg" => return Some("image/jpeg"),
            "image/gif" => return Some("image/gif"),
            "image/webp" => return Some("image/webp"),
            _ => {}
        }
    }
    None
}

fn is_raster_preview_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "webp"
            )
        })
}

fn write_empty_response(stream: &mut TcpStream, status: &str) -> std::io::Result<()> {
    write_response(
        stream,
        status,
        "text/plain; charset=utf-8",
        "no-store",
        &[],
        true,
    )
}

fn write_response(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    cache_control: &str,
    body: &[u8],
    head_only: bool,
) -> std::io::Result<()> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: {cache_control}\r\nContent-Security-Policy: default-src 'self'; img-src 'self' data:; style-src 'unsafe-inline'; script-src 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(headers.as_bytes())?;
    if !head_only {
        stream.write_all(body)?;
    }
    Ok(())
}

fn is_allowed_preview_path(diffstat: Option<&[DiffstatEntry]>, path: &str, side: &str) -> bool {
    diffstat.is_some_and(|entries| {
        entries.iter().any(|entry| match side {
            "old" => entry.old_path.as_deref() == Some(path),
            "new" => entry.new_path.as_deref() == Some(path),
            _ => false,
        })
    })
}

fn get_or_populate_state(
    state: &Arc<RwLock<WebDiffState>>,
    populate_lock: &Arc<Mutex<()>>,
    known_version: Option<usize>,
) -> Option<WebDiffState> {
    let snapshot = {
        let lock = state.read().unwrap();
        if state_is_unchanged_and_settled(&lock, known_version) {
            return None;
        }
        lock.clone()
    };
    if !needs_population(&snapshot) {
        return Some(snapshot);
    }

    let _guard = populate_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let snapshot = state.read().unwrap().clone();
    if !needs_population(&snapshot) {
        return Some(snapshot);
    }

    let diff_result = snapshot.diff.is_none().then(|| {
        get_pr_diff_native(
            snapshot.provider,
            &snapshot.workspace,
            &snapshot.repo,
            snapshot.pr_id,
        )
    });
    let diffstat_result = snapshot.diffstat.is_none().then(|| {
        get_diffstat_native(
            snapshot.provider,
            &snapshot.workspace,
            &snapshot.repo,
            snapshot.pr_id,
        )
    });

    let mut lock = state.write().unwrap();
    if same_pull_request(&lock, &snapshot) {
        let mut changed = false;
        if lock.diff.is_none() {
            match diff_result {
                Some(Ok(diff)) => {
                    lock.diff = Some(diff);
                    changed = true;
                }
                Some(Err(_)) => {
                    lock.population_failed = true;
                    changed = true;
                }
                None => {}
            }
        }
        if lock.diffstat.is_none() {
            match diffstat_result {
                Some(Ok(diffstat)) => {
                    lock.diffstat = Some(diffstat);
                    changed = true;
                }
                Some(Err(_)) => {
                    lock.population_failed = true;
                    changed = true;
                }
                None => {}
            }
        }
        if changed {
            lock.version += 1;
        }
    }
    Some(lock.clone())
}

fn needs_population(state: &WebDiffState) -> bool {
    !state.workspace.is_empty()
        && !state.repo.is_empty()
        && state.pr_id > 0
        && !state.population_failed
        && (state.diff.is_none() || state.diffstat.is_none())
}

fn state_is_unchanged_and_settled(state: &WebDiffState, known_version: Option<usize>) -> bool {
    known_version == Some(state.version) && !needs_population(state)
}

fn same_pull_request(current: &WebDiffState, snapshot: &WebDiffState) -> bool {
    current.version == snapshot.version
        && current.provider == snapshot.provider
        && current.workspace == snapshot.workspace
        && current.repo == snapshot.repo
        && current.pr_id == snapshot.pr_id
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            let decoded_k = url_decode(k);
            let decoded_v = url_decode(v);
            map.insert(decoded_k, decoded_v);
        }
    }
    map
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' if index + 2 < bytes.len() => {
                if let (Some(high), Some(low)) =
                    (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
                {
                    decoded.push((high << 4) | low);
                    index += 3;
                    continue;
                }
                decoded.push(b'%');
            }
            b'+' => decoded.push(b' '),
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

const HTML_PAGE: &str = r#"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Norn · Web Diff Viewer</title>
  <style>
    :root {
      --bg: #09090b;
      --bg-card: #121215;
      --bg-card-header: #18181b;
      --bg-sidebar: #0f0f12;
      --bg-hover: #1f1f23;
      --border: #27272a;
      --border-subtle: #1e1e24;
      --text: #f4f4f5;
      --text-muted: #a1a1aa;
      --text-dim: #71717a;
      --accent: #06b6d4;
      --accent-glow: rgba(6, 182, 212, 0.15);
      --add-bg: rgba(34, 197, 94, 0.12);
      --add-text: #4ade80;
      --add-gutter: rgba(34, 197, 94, 0.25);
      --add-word: rgba(34, 197, 94, 0.35);
      --del-bg: rgba(239, 68, 68, 0.12);
      --del-text: #f87171;
      --del-gutter: rgba(239, 68, 68, 0.25);
      --del-word: rgba(239, 68, 68, 0.35);
      --hunk-bg: #1e1b4b;
      --hunk-text: #a5b4fc;
      --badge-bg: #27272a;
      --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", monospace;
    }

    [data-theme="light"] {
      --bg: #f8fafc;
      --bg-card: #ffffff;
      --bg-card-header: #f1f5f9;
      --bg-sidebar: #ffffff;
      --bg-hover: #e2e8f0;
      --border: #cbd5e1;
      --border-subtle: #e2e8f0;
      --text: #0f172a;
      --text-muted: #475569;
      --text-dim: #94a3b8;
      --accent: #0284c7;
      --accent-glow: rgba(2, 132, 199, 0.15);
      --add-bg: #f0fdf4;
      --add-text: #15803d;
      --add-gutter: #bbf7d0;
      --add-word: #86efac;
      --del-bg: #fef2f2;
      --del-text: #b91c1c;
      --del-gutter: #fecaca;
      --del-word: #fca5a5;
      --hunk-bg: #e0e7ff;
      --hunk-text: #4338ca;
      --badge-bg: #e2e8f0;
    }

    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      background: var(--bg);
      color: var(--text);
      font-family: var(--font-sans);
      font-size: 14px;
      line-height: 1.5;
      display: flex;
      flex-direction: column;
      height: 100vh;
      overflow: hidden;
    }

    header {
      background: var(--bg-card);
      border-bottom: 1px solid var(--border);
      padding: 10px 20px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      flex-shrink: 0;
      gap: 16px;
      z-index: 20;
    }

    .brand-section {
      display: flex;
      align-items: center;
      gap: 12px;
      min-width: 0;
    }

    .norn-badge {
      background: linear-gradient(135deg, #06b6d4, #3b82f6);
      color: #000;
      font-weight: 800;
      font-size: 11px;
      letter-spacing: 0.5px;
      padding: 3px 8px;
      border-radius: 6px;
      flex-shrink: 0;
    }

    .pr-title-group {
      display: flex;
      flex-direction: column;
      min-width: 0;
    }

    .pr-title {
      font-weight: 600;
      font-size: 15px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
      display: flex;
      align-items: center;
      gap: 8px;
    }

    .pr-number {
      color: var(--accent);
    }

    .pr-meta {
      font-size: 12px;
      color: var(--text-muted);
      display: flex;
      gap: 10px;
      align-items: center;
    }

    .controls-section {
      display: flex;
      align-items: center;
      gap: 10px;
      flex-shrink: 0;
    }

    .btn-group {
      display: flex;
      background: var(--bg);
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 2px;
    }

    .btn {
      background: transparent;
      border: none;
      color: var(--text-muted);
      padding: 5px 12px;
      font-size: 12px;
      font-weight: 500;
      border-radius: 4px;
      cursor: pointer;
      display: flex;
      align-items: center;
      gap: 6px;
      transition: all 0.15s ease;
    }

    .btn:hover {
      color: var(--text);
      background: var(--bg-hover);
    }

    .btn.active {
      background: var(--accent);
      color: #000;
      font-weight: 600;
    }

    .btn-icon {
      padding: 6px 10px;
      background: var(--bg-card);
      border: 1px solid var(--border);
      border-radius: 6px;
      color: var(--text);
    }

    .live-dot {
      display: inline-block;
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: #22c55e;
      box-shadow: 0 0 8px #22c55e;
    }

    main {
      display: flex;
      flex: 1;
      overflow: hidden;
    }

    aside {
      width: 320px;
      background: var(--bg-sidebar);
      border-right: 1px solid var(--border);
      display: flex;
      flex-direction: column;
      flex-shrink: 0;
      overflow: hidden;
    }

    .sidebar-header {
      padding: 12px 14px;
      border-bottom: 1px solid var(--border);
      display: flex;
      flex-direction: column;
      gap: 8px;
    }

    .search-input {
      width: 100%;
      background: var(--bg);
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 6px 10px;
      font-size: 12px;
      color: var(--text);
      outline: none;
    }

    .search-input:focus {
      border-color: var(--accent);
    }

    .file-list {
      flex: 1;
      overflow-y: auto;
      padding: 6px 0;
      list-style: none;
    }

    .file-item {
      padding: 6px 14px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      font-size: 12px;
      cursor: pointer;
      color: var(--text-muted);
      border-left: 2px solid transparent;
      user-select: none;
    }

    .file-item:hover {
      background: var(--bg-hover);
      color: var(--text);
    }

    .file-item.active {
      background: var(--bg-hover);
      color: var(--text);
      border-left-color: var(--accent);
      font-weight: 500;
    }

    .file-item-name {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      flex: 1;
    }

    .file-item-dir {
      color: var(--text-dim);
      font-size: 11px;
    }

    .status-badge {
      font-size: 10px;
      font-weight: 700;
      padding: 1px 5px;
      border-radius: 4px;
      text-transform: uppercase;
    }

    .status-badge.added { background: rgba(34, 197, 94, 0.2); color: #4ade80; }
    .status-badge.modified { background: rgba(234, 179, 8, 0.2); color: #facc15; }
    .status-badge.removed { background: rgba(239, 68, 68, 0.2); color: #f87171; }
    .status-badge.renamed { background: rgba(59, 130, 246, 0.2); color: #60a5fa; }

    .diff-container {
      flex: 1;
      overflow-y: auto;
      padding: 20px;
      display: flex;
      flex-direction: column;
      gap: 24px;
    }

    .file-diff-card {
      background: var(--bg-card);
      border: 1px solid var(--border);
      border-radius: 8px;
      overflow: hidden;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
    }

    .file-diff-header {
      background: var(--bg-card-header);
      border-bottom: 1px solid var(--border);
      padding: 8px 14px;
      display: flex;
      align-items: center;
      justify-content: space-between;
      gap: 12px;
      font-size: 13px;
      font-weight: 600;
      position: sticky;
      top: 0;
      z-index: 10;
    }

    .file-diff-path {
      display: flex;
      align-items: center;
      gap: 8px;
      font-family: var(--font-mono);
      font-size: 12px;
      color: var(--text);
    }

    .copy-btn {
      background: transparent;
      border: none;
      color: var(--text-dim);
      cursor: pointer;
      font-size: 11px;
      padding: 2px 6px;
      border-radius: 4px;
    }

    .copy-btn:hover {
      background: var(--bg-hover);
      color: var(--text);
    }

    /* Diff table */
    .diff-table {
      width: 100%;
      border-collapse: collapse;
      font-family: var(--font-mono);
      font-size: 12px;
      line-height: 1.45;
    }

    .diff-line {
      display: flex;
      width: 100%;
    }

    .gutter {
      width: 44px;
      min-width: 44px;
      padding: 1px 8px;
      text-align: right;
      color: var(--text-dim);
      user-select: none;
      background: var(--bg-card);
      border-right: 1px solid var(--border-subtle);
      font-size: 11px;
    }

    .sign {
      width: 20px;
      min-width: 20px;
      text-align: center;
      user-select: none;
      color: var(--text-dim);
    }

    .code {
      flex: 1;
      padding: 1px 8px;
      white-space: pre-wrap;
      word-break: break-all;
    }

    .diff-row-hunk {
      background: var(--hunk-bg);
      color: var(--hunk-text);
      font-style: italic;
      padding: 4px 12px;
      font-size: 11px;
      border-top: 1px solid var(--border-subtle);
      border-bottom: 1px solid var(--border-subtle);
    }

    .diff-row-add {
      background: var(--add-bg);
      color: var(--add-text);
    }
    .diff-row-add .gutter { background: var(--add-gutter); color: var(--add-text); }
    .diff-row-add .sign { color: var(--add-text); }

    .diff-row-del {
      background: var(--del-bg);
      color: var(--del-text);
    }
    .diff-row-del .gutter { background: var(--del-gutter); color: var(--del-text); }
    .diff-row-del .sign { color: var(--del-text); }

    .diff-word-del {
      background: var(--del-word);
      border-radius: 2px;
      padding: 0 1px;
    }

    .diff-word-add {
      background: var(--add-word);
      border-radius: 2px;
      padding: 0 1px;
    }

    /* Split / Side-by-side mode */
    .split-table {
      width: 100%;
      display: flex;
      flex-direction: column;
    }

    .split-row {
      display: flex;
      width: 100%;
      border-bottom: 1px solid transparent;
    }

    .split-pane {
      width: 50%;
      display: flex;
      overflow: hidden;
    }

    .split-pane:first-child {
      border-right: 1px solid var(--border);
    }

    /* Image diff preview */
    .image-diff-wrapper {
      padding: 24px;
      display: flex;
      flex-direction: column;
      align-items: center;
      gap: 16px;
      background: #000;
      background-image: linear-gradient(45deg, #18181b 25%, transparent 25%), linear-gradient(-45deg, #18181b 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #18181b 75%), linear-gradient(-45deg, transparent 75%, #18181b 75%);
      background-size: 20px 20px;
      background-position: 0 0, 0 10px, 10px -10px, -10px 0px;
    }

    .image-diff-2up {
      display: flex;
      gap: 24px;
      justify-content: center;
      width: 100%;
      flex-wrap: wrap;
    }

    .image-card {
      display: flex;
      flex-direction: column;
      align-items: center;
      background: rgba(15, 23, 42, 0.8);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 12px;
      max-width: 48%;
    }

    .image-card img {
      max-width: 100%;
      max-height: 400px;
      object-fit: contain;
      border-radius: 4px;
    }

    .image-card-title {
      font-size: 11px;
      font-weight: 700;
      text-transform: uppercase;
      margin-bottom: 8px;
      color: var(--text-muted);
    }

    .image-preview-empty {
      padding: 20px;
      color: var(--text-dim);
    }

    .empty-state {
      padding: 80px 20px;
      text-align: center;
      color: var(--text-muted);
    }
  </style>
</head>
<body>
  <header>
    <div class="brand-section">
      <span class="norn-badge">NORN DIFF</span>
      <div class="pr-title-group">
        <div class="pr-title">
          <span id="pr-number" class="pr-number">#...</span>
          <span id="pr-title-text">Loading pull request...</span>
        </div>
        <div class="pr-meta">
          <span id="pr-repo">...</span>
          <span>•</span>
          <span id="pr-branches">...</span>
          <span>•</span>
          <span id="pr-stats">0 files</span>
        </div>
      </div>
    </div>

    <div class="controls-section">
      <div class="btn-group">
        <button id="mode-split" class="btn active" onclick="setViewMode('split')">Split</button>
        <button id="mode-unified" class="btn" onclick="setViewMode('unified')">Unified</button>
      </div>
      <button class="btn btn-icon" onclick="toggleCollapseAll()" title="Expand/Collapse all files">⇕ Files</button>
      <button class="btn btn-icon" onclick="toggleTheme()" title="Toggle Theme">🌓</button>
      <div title="Connected to Norn local server" style="display:flex;align-items:center;gap:6px;font-size:11px;color:var(--text-dim);margin-left:6px;">
        <span class="live-dot"></span>
        <span>Live</span>
      </div>
    </div>
  </header>

  <main>
    <aside>
      <div class="sidebar-header">
        <input id="file-search" class="search-input" type="text" placeholder="Filter files (e.g. .rs, src/)..." oninput="filterFiles(this.value)">
      </div>
      <ul id="file-list" class="file-list"></ul>
    </aside>

    <div id="diff-container" class="diff-container">
      <div class="empty-state">Loading diff...</div>
    </div>
  </main>

  <script>
    const sessionBase = window.location.pathname.replace(/\/+$/, '');

    function apiUrl(path, params = {}) {
      const url = new URL(sessionBase + path, window.location.origin);
      for (const [key, value] of Object.entries(params)) {
        url.searchParams.set(key, value);
      }
      return url.pathname + url.search;
    }

    let state = {
      version: -1,
      prId: 0,
      diff: '',
      diffstat: [],
      files: [],
      viewMode: 'split',
      theme: 'dark',
      filterQuery: '',
      collapsedFiles: new Set()
    };

    function escapeHtml(str) {
      if (!str) return '';
      return str
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;")
        .replace(/'/g, "&#039;");
    }

    function normalizeStatus(status) {
      if (status === 'added' || status === 'removed' || status === 'renamed') return status;
      return 'modified';
    }

    function decodeGitPathToken(token) {
      if (!token.startsWith('"') || !token.endsWith('"')) return token;
      const bytes = [];
      const encoder = new TextEncoder();
      const escapes = {
        a: 7,
        b: 8,
        t: 9,
        n: 10,
        v: 11,
        f: 12,
        r: 13,
        '"': 34,
        '\\': 92
      };

      for (let index = 1; index < token.length - 1; index++) {
        const current = token[index];
        if (current !== '\\') {
          bytes.push(...encoder.encode(current));
          continue;
        }

        const escaped = token[++index];
        if (escaped === undefined) break;
        if (/^[0-7]$/.test(escaped)) {
          let octal = escaped;
          while (octal.length < 3 && /^[0-7]$/.test(token[index + 1] || '')) {
            octal += token[++index];
          }
          bytes.push(parseInt(octal, 8));
        } else if (Object.prototype.hasOwnProperty.call(escapes, escaped)) {
          bytes.push(escapes[escaped]);
        } else {
          bytes.push(...encoder.encode(escaped));
        }
      }

      return new TextDecoder().decode(new Uint8Array(bytes));
    }

    function readQuotedGitToken(input) {
      if (!input.startsWith('"')) return null;
      let escaped = false;
      for (let index = 1; index < input.length; index++) {
        const current = input[index];
        if (escaped) {
          escaped = false;
        } else if (current === '\\') {
          escaped = true;
        } else if (current === '"') {
          return { token: input.slice(0, index + 1), rest: input.slice(index + 1) };
        }
      }
      return null;
    }

    function stripGitSidePrefix(token, prefix) {
      const path = decodeGitPathToken(token);
      return path.startsWith(prefix) ? path.slice(prefix.length) : '';
    }

    function parseDiffGitHeader(line) {
      const payload = line.slice('diff --git '.length);
      let oldToken = '';
      let newToken = '';

      if (payload.startsWith('"')) {
        const oldPart = readQuotedGitToken(payload);
        if (!oldPart) return { oldPath: '', newPath: '' };
        oldToken = oldPart.token;
        const remaining = oldPart.rest.trimStart();
        const newPart = readQuotedGitToken(remaining);
        newToken = newPart ? newPart.token : remaining;
      } else {
        const separator = Math.max(payload.lastIndexOf(' b/'), payload.lastIndexOf(' "b/'));
        if (separator < 0) return { oldPath: '', newPath: '' };
        oldToken = payload.slice(0, separator);
        newToken = payload.slice(separator + 1);
      }

      return {
        oldPath: stripGitSidePrefix(oldToken, 'a/'),
        newPath: stripGitSidePrefix(newToken, 'b/')
      };
    }

    function isImageFile(path) {
      if (!path) return false;
      const lower = path.toLowerCase();
      return ['.png', '.jpg', '.jpeg', '.gif', '.webp'].some(ext => lower.endsWith(ext));
    }

    function parseUnifiedDiff(raw) {
      const files = [];
      if (!raw) return files;
      const lines = raw.split('\n');
      let currentFile = null;
      let currentHunk = null;

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        if (line.startsWith('diff --git ')) {
          const { oldPath, newPath } = parseDiffGitHeader(line);
          currentFile = {
            oldPath,
            newPath,
            path: newPath || oldPath,
            status: 'modified',
            hunks: [],
            linesAdded: 0,
            linesRemoved: 0,
            isBinary: false
          };
          files.push(currentFile);
          currentHunk = null;
        } else if (currentFile) {
          if (line.startsWith('new file mode ')) {
            currentFile.status = 'added';
          } else if (line.startsWith('deleted file mode ')) {
            currentFile.status = 'removed';
          } else if (line.startsWith('rename from ') || line.startsWith('similarity index ')) {
            currentFile.status = 'renamed';
          } else if (line.startsWith('Binary files ') || line.includes('GIT binary patch')) {
            currentFile.isBinary = true;
          } else if (line.startsWith('@@ ')) {
            const match = line.match(/^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(.*)$/);
            if (match) {
              const oldStart = parseInt(match[1], 10);
              const oldCount = match[2] !== undefined ? parseInt(match[2], 10) : 1;
              const newStart = parseInt(match[3], 10);
              const newCount = match[4] !== undefined ? parseInt(match[4], 10) : 1;
              currentHunk = {
                header: line,
                oldStart,
                oldCount,
                newStart,
                newCount,
                context: match[5] || '',
                lines: []
              };
              currentFile.hunks.push(currentHunk);
            }
          } else if (currentHunk) {
            if (line.startsWith('+')) {
              currentFile.linesAdded++;
              currentHunk.lines.push({ type: 'add', content: line.slice(1) });
            } else if (line.startsWith('-')) {
              currentFile.linesRemoved++;
              currentHunk.lines.push({ type: 'delete', content: line.slice(1) });
            } else if (line.startsWith(' ')) {
              currentHunk.lines.push({ type: 'context', content: line.slice(1) });
            } else if (line.startsWith('\\')) {
              currentHunk.lines.push({ type: 'meta', content: line });
            }
          }
        }
      }
      return files;
    }

    function mergeDiffWithDiffstat(parsedFiles, diffstat) {
      const merged = [...parsedFiles];
      if (!diffstat || diffstat.length === 0) return merged;

      for (const entry of diffstat) {
        const path = entry.newPath || entry.oldPath;
        let existing = merged.find(f => (entry.newPath && f.newPath === entry.newPath) || (entry.oldPath && f.oldPath === entry.oldPath));
        if (existing) {
          existing.status = normalizeStatus(entry.status || existing.status);
          if (entry.linesAdded) existing.linesAdded = entry.linesAdded;
          if (entry.linesRemoved) existing.linesRemoved = entry.linesRemoved;
        } else {
          merged.push({
            oldPath: entry.oldPath || '',
            newPath: entry.newPath || '',
            path: path || '',
            status: normalizeStatus(entry.status),
            hunks: [],
            linesAdded: entry.linesAdded || 0,
            linesRemoved: entry.linesRemoved || 0,
            isBinary: isImageFile(path)
          });
        }
      }
      return merged;
    }

    async function pollState() {
      try {
        const res = await fetch(apiUrl('/api/state', { version: state.version }), {
          credentials: 'same-origin',
          cache: 'no-store'
        });
        if (res.status === 204) return;
        if (res.ok) {
          const data = await res.json();
          if (data.version !== state.version || data.prId !== state.prId) {
            state.version = data.version;
            state.prId = data.prId;
            state.pr = data;
            state.diff = data.diff || '';
            state.diffstat = data.diffstat || [];

            const parsed = parseUnifiedDiff(state.diff);
            state.files = mergeDiffWithDiffstat(parsed, state.diffstat);

            renderHeader(data);
            renderSidebar();
            renderDiff();
          }
        }
      } catch (e) {}
    }

    function renderHeader(pr) {
      document.getElementById('pr-number').innerText = '#' + (pr.prId || '');
      document.getElementById('pr-title-text').innerText = pr.prTitle || 'Pull Request';
      document.getElementById('pr-repo').innerText = (pr.workspace ? pr.workspace + '/' : '') + (pr.repo || '');
      document.getElementById('pr-branches').innerText = (pr.sourceBranch || 'source') + ' → ' + (pr.targetBranch || 'target');

      const totalAdded = state.files.reduce((acc, f) => acc + (f.linesAdded || 0), 0);
      const totalRemoved = state.files.reduce((acc, f) => acc + (f.linesRemoved || 0), 0);
      document.getElementById('pr-stats').innerHTML =
        `<b>${state.files.length}</b> files changed (<span style="color:var(--add-text)">+${totalAdded}</span> <span style="color:var(--del-text)">-${totalRemoved}</span>)`;
    }

    function renderSidebar() {
      const list = document.getElementById('file-list');
      const query = state.filterQuery.toLowerCase();

      const items = state.files.filter(f => !query || f.path.toLowerCase().includes(query)).map((file, idx) => {
        const parts = file.path.split('/');
        const filename = parts.pop();
        const dir = parts.join('/');
        const statusClass = normalizeStatus(file.status);
        const statusLetter = statusClass === 'added' ? 'A' : statusClass === 'removed' ? 'D' : statusClass === 'renamed' ? 'R' : 'M';

        return `
          <li class="file-item" data-scroll-path="${escapeHtml(file.path)}">
            <span class="status-badge ${statusClass}">${statusLetter}</span>
            <div class="file-item-name">
              ${dir ? `<span class="file-item-dir">${escapeHtml(dir)}/</span>` : ''}
              <span>${escapeHtml(filename)}</span>
            </div>
            <div style="font-size:11px; font-family:var(--font-mono); display:flex; gap:4px;">
              ${file.linesAdded > 0 ? `<span style="color:var(--add-text)">+${file.linesAdded}</span>` : ''}
              ${file.linesRemoved > 0 ? `<span style="color:var(--del-text)">-${file.linesRemoved}</span>` : ''}
            </div>
          </li>
        `;
      }).join('');

      list.innerHTML = items || '<div style="padding:16px;color:var(--text-dim);font-size:12px;">No matching files</div>';
    }

    function renderDiff() {
      const container = document.getElementById('diff-container');
      const query = state.filterQuery.toLowerCase();
      const filesToRender = state.files.filter(f => !query || f.path.toLowerCase().includes(query));
      const loadWarning = state.pr && state.pr.populationFailed
        ? '<div style="padding:12px 16px;color:var(--del-text);border-bottom:1px solid var(--border);">Some diff data could not be loaded. Return to Norn and open the browser diff again to retry.</div>'
        : '';

      if (state.pr && state.pr.populationFailed && filesToRender.length === 0) {
        container.innerHTML = '<div class="empty-state">Could not load diff data. Return to Norn and open the browser diff again to retry.</div>';
        return;
      }

      if (filesToRender.length === 0) {
        container.innerHTML = '<div class="empty-state">No files to display</div>';
        return;
      }

      container.innerHTML = loadWarning + filesToRender.map(file => {
        const isCollapsed = state.collapsedFiles.has(file.path);
        const isImage = isImageFile(file.path) || file.isBinary;
        const anchorId = 'file-' + file.path.replace(/[^a-zA-Z0-9_-]/g, '_');

        return `
          <div id="${anchorId}" class="file-diff-card">
            <div class="file-diff-header">
              <div class="file-diff-path" data-toggle-path="${escapeHtml(file.path)}">
                <span style="cursor:pointer;">${isCollapsed ? '▶' : '▼'}</span>
                <span class="status-badge ${normalizeStatus(file.status)}">${normalizeStatus(file.status)}</span>
                <span>${escapeHtml(file.path)}</span>
                <button class="copy-btn" data-copy-path="${escapeHtml(file.path)}">Copy</button>
              </div>
              <div style="font-size:12px; font-family:var(--font-mono);">
                <span style="color:var(--add-text)">+${file.linesAdded}</span>
                <span style="color:var(--del-text)">-${file.linesRemoved}</span>
              </div>
            </div>
            ${isCollapsed ? '' : renderFileContent(file, isImage)}
          </div>
        `;
      }).join('');
    }

    function renderFileContent(file, isImage) {
      if (isImage) {
        return renderImageDiff(file);
      }
      if (state.viewMode === 'split') {
        return renderSplitDiff(file);
      }
      return renderUnifiedDiff(file);
    }

    function renderImageDiff(file) {
      const oldUrl = apiUrl('/api/file-preview', { path: file.oldPath || file.path, side: 'old' });
      const newUrl = apiUrl('/api/file-preview', { path: file.newPath || file.path, side: 'new' });

      return `
        <div class="image-diff-wrapper">
          <div class="image-diff-2up">
            ${file.status !== 'added' ? `
              <div class="image-card">
                <div class="image-card-title">Base (Old)</div>
                <img src="${oldUrl}" onerror="handleImagePreviewError(this)" alt="Old version" />
              </div>
            ` : ''}
            ${normalizeStatus(file.status) !== 'removed' ? `
              <div class="image-card">
                <div class="image-card-title">Changed (New)</div>
                <img src="${newUrl}" onerror="handleImagePreviewError(this)" alt="New version" />
              </div>
            ` : ''}
          </div>
        </div>
      `;
    }

    function handleImagePreviewError(image) {
      const parent = image.parentElement;
      if (!parent) return;
      const fallback = document.createElement('div');
      fallback.className = 'image-preview-empty';
      fallback.textContent = 'No preview available';
      parent.replaceChildren(fallback);
    }

    function renderUnifiedDiff(file) {
      if (!file.hunks || file.hunks.length === 0) {
        return '<div style="padding:16px;color:var(--text-dim);font-family:var(--font-mono);font-size:12px;">No text changes or binary file</div>';
      }

      let html = '<div class="diff-table">';
      for (const hunk of file.hunks) {
        html += `<div class="diff-row-hunk">${escapeHtml(hunk.header)}</div>`;
        let oldLine = hunk.oldStart;
        let newLine = hunk.newStart;

        for (const line of hunk.lines) {
          if (line.type === 'add') {
            html += `
              <div class="diff-line diff-row-add">
                <div class="gutter"></div>
                <div class="gutter">${newLine++}</div>
                <div class="sign">+</div>
                <div class="code">${escapeHtml(line.content)}</div>
              </div>
            `;
          } else if (line.type === 'delete') {
            html += `
              <div class="diff-line diff-row-del">
                <div class="gutter">${oldLine++}</div>
                <div class="gutter"></div>
                <div class="sign">-</div>
                <div class="code">${escapeHtml(line.content)}</div>
              </div>
            `;
          } else if (line.type === 'context') {
            html += `
              <div class="diff-line">
                <div class="gutter">${oldLine++}</div>
                <div class="gutter">${newLine++}</div>
                <div class="sign"> </div>
                <div class="code">${escapeHtml(line.content)}</div>
              </div>
            `;
          }
        }
      }
      html += '</div>';
      return html;
    }

    function renderSplitDiff(file) {
      if (!file.hunks || file.hunks.length === 0) {
        return '<div style="padding:16px;color:var(--text-dim);font-family:var(--font-mono);font-size:12px;">No text changes or binary file</div>';
      }

      let html = '<div class="split-table">';
      for (const hunk of file.hunks) {
        html += `<div class="diff-row-hunk">${escapeHtml(hunk.header)}</div>`;
        let oldLine = hunk.oldStart;
        let newLine = hunk.newStart;

        let i = 0;
        while (i < hunk.lines.length) {
          const current = hunk.lines[i];
          if (current.type === 'context') {
            html += `
              <div class="split-row">
                <div class="split-pane">
                  <div class="gutter">${oldLine++}</div>
                  <div class="sign"> </div>
                  <div class="code">${escapeHtml(current.content)}</div>
                </div>
                <div class="split-pane">
                  <div class="gutter">${newLine++}</div>
                  <div class="sign"> </div>
                  <div class="code">${escapeHtml(current.content)}</div>
                </div>
              </div>
            `;
            i++;
          } else {
            const dels = [];
            const adds = [];
            while (i < hunk.lines.length && hunk.lines[i].type === 'delete') {
              dels.push(hunk.lines[i]);
              i++;
            }
            while (i < hunk.lines.length && hunk.lines[i].type === 'add') {
              adds.push(hunk.lines[i]);
              i++;
            }

            const maxLen = Math.max(dels.length, adds.length);
            for (let k = 0; k < maxLen; k++) {
              const del = dels[k];
              const add = adds[k];

              html += '<div class="split-row">';
              if (del) {
                html += `
                  <div class="split-pane diff-row-del">
                    <div class="gutter">${oldLine++}</div>
                    <div class="sign">-</div>
                    <div class="code">${escapeHtml(del.content)}</div>
                  </div>
                `;
              } else {
                html += `
                  <div class="split-pane">
                    <div class="gutter"></div>
                    <div class="sign"></div>
                    <div class="code"></div>
                  </div>
                `;
              }

              if (add) {
                html += `
                  <div class="split-pane diff-row-add">
                    <div class="gutter">${newLine++}</div>
                    <div class="sign">+</div>
                    <div class="code">${escapeHtml(add.content)}</div>
                  </div>
                `;
              } else {
                html += `
                  <div class="split-pane">
                    <div class="gutter"></div>
                    <div class="sign"></div>
                    <div class="code"></div>
                  </div>
                `;
              }
              html += '</div>';
            }
          }
        }
      }
      html += '</div>';
      return html;
    }

    function setViewMode(mode) {
      state.viewMode = mode;
      document.getElementById('mode-split').classList.toggle('active', mode === 'split');
      document.getElementById('mode-unified').classList.toggle('active', mode === 'unified');
      renderDiff();
    }

    function toggleTheme() {
      state.theme = state.theme === 'dark' ? 'light' : 'dark';
      document.documentElement.setAttribute('data-theme', state.theme);
    }

    function toggleCollapseAll() {
      if (state.collapsedFiles.size === state.files.length) {
        state.collapsedFiles.clear();
      } else {
        state.files.forEach(f => state.collapsedFiles.add(f.path));
      }
      renderDiff();
    }

    function toggleFileCollapse(path) {
      if (state.collapsedFiles.has(path)) {
        state.collapsedFiles.delete(path);
      } else {
        state.collapsedFiles.add(path);
      }
      renderDiff();
    }

    function filterFiles(query) {
      state.filterQuery = query;
      renderSidebar();
      renderDiff();
    }

    function scrollToFile(path) {
      const anchorId = 'file-' + path.replace(/[^a-zA-Z0-9_-]/g, '_');
      const el = document.getElementById(anchorId);
      if (el) {
        el.scrollIntoView({ behavior: 'smooth', block: 'start' });
      }
    }

    function copyPath(path) {
      navigator.clipboard.writeText(path).then(() => {
        // Feedback
      });
    }

    document.addEventListener('click', (event) => {
      if (!(event.target instanceof Element)) return;
      const copyTarget = event.target.closest('[data-copy-path]');
      if (copyTarget) {
        event.stopPropagation();
        copyPath(copyTarget.dataset.copyPath);
        return;
      }
      const toggleTarget = event.target.closest('[data-toggle-path]');
      if (toggleTarget) {
        toggleFileCollapse(toggleTarget.dataset.togglePath);
        return;
      }
      const scrollTarget = event.target.closest('[data-scroll-path]');
      if (scrollTarget) {
        scrollToFile(scrollTarget.dataset.scrollPath);
      }
    });

    // Keyboard shortcuts
    window.addEventListener('keydown', (e) => {
      if (e.target.tagName === 'INPUT') return;
      if (e.key === 'u') setViewMode(state.viewMode === 'split' ? 'unified' : 'split');
      if (e.key === 't') toggleTheme();
      if (e.key === 'f') {
        e.preventDefault();
        document.getElementById('file-search').focus();
      }
    });

    setInterval(pollState, 1200);
    pollState();
  </script>
</body>
</html>
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    struct ChunkedReader {
        source: Cursor<Vec<u8>>,
        chunk_size: usize,
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            let limit = buffer.len().min(self.chunk_size);
            self.source.read(&mut buffer[..limit])
        }
    }

    fn diffstat(old_path: Option<&str>, new_path: Option<&str>) -> DiffstatEntry {
        DiffstatEntry {
            status: "modified".to_string(),
            lines_added: 1,
            lines_removed: 1,
            old_path: old_path.map(str::to_string),
            new_path: new_path.map(str::to_string),
        }
    }

    fn request(port: u16, request: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to diff server");
        stream
            .write_all(request.as_bytes())
            .expect("write HTTP request");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read HTTP response");
        response
    }

    #[test]
    fn parses_query_string() {
        let q = parse_query("path=src%2Fmain.rs&side=old");
        assert_eq!(q.get("path").unwrap(), "src/main.rs");
        assert_eq!(q.get("side").unwrap(), "old");
    }

    #[test]
    fn decodes_url_encoding() {
        assert_eq!(url_decode("hello%20world%2Bfoo"), "hello world+foo");
        assert_eq!(url_decode("caf%C3%A9.png"), "café.png");
    }

    #[cfg(unix)]
    #[test]
    fn browser_open_result_rejects_nonzero_exit_status() {
        use std::os::unix::process::ExitStatusExt;

        assert!(browser_open_result(Ok(ExitStatus::from_raw(0))).is_ok());
        let error = browser_open_result(Ok(ExitStatus::from_raw(7 << 8)))
            .expect_err("non-zero opener status should fail");
        assert!(error.contains("Failed to open browser"));
        assert!(error.contains('7'));
    }

    #[test]
    fn reads_fragmented_request_headers_until_the_terminator() {
        let mut reader = ChunkedReader {
            source: Cursor::new(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n".to_vec()),
            chunk_size: 3,
        };
        let request = read_request_head(&mut reader)
            .expect("fragmented request should parse")
            .expect("request should be present");
        assert!(request.ends_with(b"\r\n\r\n"));
    }

    #[test]
    fn rejects_request_headers_over_the_bound() {
        let mut reader = Cursor::new(vec![b'a'; MAX_REQUEST_HEAD_BYTES + 1]);
        let error = read_request_head(&mut reader).expect_err("oversized headers should fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn authenticates_only_the_exact_session_path() {
        assert_eq!(authenticated_route("/session/secret/", "secret"), Some("/"));
        assert_eq!(
            authenticated_route("/session/secret/api/state", "secret"),
            Some("/api/state")
        );
        assert_eq!(authenticated_route("/api/state", "secret"), None);
        assert_eq!(
            authenticated_route("/session/secret-other/api/state", "secret"),
            None
        );
        assert_eq!(
            authenticated_route("/session/secres/api/state", "secret"),
            None
        );
    }

    #[test]
    fn preview_mime_types_are_allowlisted_before_header_rendering() {
        assert_eq!(
            safe_preview_mime_type("image/png", "application/octet-stream"),
            Some("image/png")
        );
        assert_eq!(
            safe_preview_mime_type("bad\r\nX-Injected: yes", "image/webp"),
            Some("image/webp")
        );
        assert_eq!(
            safe_preview_mime_type("bad\r\nX-Injected: yes", "also-invalid"),
            None
        );
        assert_eq!(
            safe_preview_mime_type("image/svg+xml", "image/svg+xml"),
            None
        );
    }

    #[test]
    fn preview_paths_allow_only_bounded_raster_extensions() {
        for path in ["logo.png", "photo.JPG", "clip.gif", "asset.webp"] {
            assert!(is_raster_preview_path(path), "{path}");
        }
        for path in ["icon.ico", "bitmap.bmp", "vector.svg", "notes.txt"] {
            assert!(!is_raster_preview_path(path), "{path}");
        }
    }

    #[test]
    fn allows_file_previews_only_for_the_matching_changed_side() {
        let entries = vec![diffstat(Some("old/logo.png"), Some("new/logo.png"))];
        assert!(is_allowed_preview_path(
            Some(&entries),
            "old/logo.png",
            "old"
        ));
        assert!(is_allowed_preview_path(
            Some(&entries),
            "new/logo.png",
            "new"
        ));
        assert!(!is_allowed_preview_path(
            Some(&entries),
            "private/other.png",
            "new"
        ));
        assert!(!is_allowed_preview_path(
            Some(&entries),
            "new/logo.png",
            "old"
        ));
        assert!(!is_allowed_preview_path(None, "new/logo.png", "new"));
    }

    #[test]
    fn server_starts_and_updates_state() {
        let state = Arc::new(RwLock::new(WebDiffState::default()));
        let server = WebDiffServer::start(Arc::clone(&state)).expect("server should start");
        assert!(server.port > 0);
        assert_eq!(server.session_token.len(), SESSION_TOKEN_BYTES * 2);
        assert!(server.url().contains(&server.session_token));

        server.update_pr(WebDiffState {
            version: 0,
            provider: None,
            workspace: "my-workspace".to_string(),
            repo: "my-repo".to_string(),
            pr_id: 42,
            pr_title: "feat: add server".to_string(),
            pr_author: "test-user".to_string(),
            source_branch: "feature".to_string(),
            target_branch: "main".to_string(),
            diff: Some("diff --git a/a b/a".to_string()),
            diffstat: None,
            population_failed: false,
        });

        let lock = state.read().unwrap();
        assert_eq!(lock.pr_id, 42);
        assert_eq!(lock.pr_title, "feat: add server");
    }

    #[test]
    fn unchanged_populated_state_short_circuits_without_cloning_or_serializing_the_diff() {
        let state = Arc::new(RwLock::new(WebDiffState {
            version: 9,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 42,
            diff: Some("large diff body".to_string()),
            diffstat: Some(Vec::new()),
            ..WebDiffState::default()
        }));
        let server = WebDiffServer::start(state).expect("server should start");

        let response = request(
            server.port,
            &format!(
                "GET /session/{}/api/state?version=9 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                server.session_token
            ),
        );

        assert!(response.starts_with("HTTP/1.1 204 No Content"));
        assert!(!response.contains("large diff body"));
        assert!(response.contains("Content-Length: 0"));
    }

    #[test]
    fn unchanged_state_still_populates_missing_provider_data() {
        let state = WebDiffState {
            version: 9,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 42,
            ..WebDiffState::default()
        };

        assert!(!state_is_unchanged_and_settled(&state, Some(9)));
    }

    #[test]
    fn diffstat_changes_increment_the_state_version() {
        let state = Arc::new(RwLock::new(WebDiffState::default()));
        let server = WebDiffServer::start(Arc::clone(&state)).expect("server should start");
        let diff = Some("diff --git a/a.png b/a.png".to_string());

        server.update_pr(WebDiffState {
            version: 0,
            provider: None,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 7,
            pr_title: "Images".to_string(),
            pr_author: "author".to_string(),
            source_branch: "feature".to_string(),
            target_branch: "main".to_string(),
            diff: diff.clone(),
            diffstat: None,
            population_failed: false,
        });
        let initial_version = state.read().unwrap().version;
        server.update_pr(WebDiffState {
            version: 0,
            provider: None,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 7,
            pr_title: "Images".to_string(),
            pr_author: "author".to_string(),
            source_branch: "feature".to_string(),
            target_branch: "main".to_string(),
            diff,
            diffstat: Some(vec![diffstat(Some("a.png"), Some("a.png"))]),
            population_failed: false,
        });

        assert_eq!(state.read().unwrap().version, initial_version + 1);
    }

    #[test]
    fn failed_population_waits_for_an_explicit_retry() {
        let mut state = WebDiffState {
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 7,
            population_failed: true,
            ..WebDiffState::default()
        };

        assert!(!needs_population(&state));
        state.population_failed = false;
        assert!(needs_population(&state));
    }

    #[test]
    fn updating_the_viewer_resets_a_failed_population_for_explicit_retry() {
        let state = Arc::new(RwLock::new(WebDiffState {
            version: 3,
            provider: None,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 7,
            pr_title: "Retry".to_string(),
            population_failed: true,
            ..WebDiffState::default()
        }));
        let server = WebDiffServer::start(Arc::clone(&state)).expect("server should start");

        server.update_pr(WebDiffState {
            provider: None,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 7,
            pr_title: "Retry".to_string(),
            population_failed: false,
            ..WebDiffState::default()
        });

        let state = state.read().unwrap();
        assert!(!state.population_failed);
        assert!(needs_population(&state));
        assert_eq!(state.version, 4);
    }

    #[test]
    fn server_rejects_unauthenticated_requests_without_cors() {
        let state = Arc::new(RwLock::new(WebDiffState::default()));
        let server = WebDiffServer::start(state).expect("server should start");

        let unauthenticated = request(
            server.port,
            "GET /api/health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        );
        assert!(unauthenticated.starts_with("HTTP/1.1 404 Not Found"));
        assert!(!unauthenticated.contains("Access-Control-Allow-Origin"));

        let authenticated = request(
            server.port,
            &format!(
                "GET /session/{}/api/health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                server.session_token
            ),
        );
        assert!(authenticated.starts_with("HTTP/1.1 200 OK"));
        assert!(authenticated.contains("Content-Security-Policy:"));
        assert!(authenticated.ends_with(r#"{"status":"ok"}"#));
    }
}
