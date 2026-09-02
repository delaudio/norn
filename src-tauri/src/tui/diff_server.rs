use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
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
const MAX_ACTIVE_CONNECTIONS: usize = 16;
const MAX_BROWSER_ASSET_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BROWSER_ASSET_TOTAL_BYTES: u64 = 32 * 1024 * 1024;
const MAX_BROWSER_ASSET_FILES: usize = 128;
const SESSION_TOKEN_BYTES: usize = 32;
const SECURITY_HEADERS: &str = "Content-Security-Policy: default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'none'\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\n";

#[derive(Clone)]
struct BrowserAsset {
    content_type: &'static str,
    body: Arc<[u8]>,
}

struct BrowserAssets {
    files: HashMap<String, BrowserAsset>,
}

impl BrowserAssets {
    #[cfg(not(test))]
    fn load_default() -> Result<Self, String> {
        let root = resolve_browser_asset_root()?;
        Self::load_from(&root)
    }

    fn load_from(root: &Path) -> Result<Self, String> {
        let root = root
            .canonicalize()
            .map_err(|error| format!("Failed to resolve packaged browser diff assets: {error}"))?;
        let mut files = HashMap::new();
        let mut total_bytes = 0;
        collect_browser_assets(&root, &root, &mut files, &mut total_bytes)?;
        if !files.contains_key("/browser-diff.html") {
            return Err("Browser diff assets do not contain browser-diff.html.".to_string());
        }
        Ok(Self { files })
    }

    fn get(&self, route: &str) -> Option<&BrowserAsset> {
        let route = match route {
            "/" | "/index.html" => "/browser-diff.html",
            route => route,
        };
        self.files.get(route)
    }

    #[cfg(test)]
    fn fixture() -> Self {
        Self {
            files: HashMap::from([
                (
                    "/browser-diff.html".to_string(),
                    BrowserAsset {
                        content_type: "text/html; charset=utf-8",
                        body: Arc::from(
                            b"<!doctype html><div id=\"root\"></div><script type=\"module\" src=\"./assets/viewer.js\"></script>"
                                .as_slice(),
                        ),
                    },
                ),
                (
                    "/assets/viewer.js".to_string(),
                    BrowserAsset {
                        content_type: "text/javascript; charset=utf-8",
                        body: Arc::from(b"document.title = 'Norn';".as_slice()),
                    },
                ),
            ]),
        }
    }
}

#[cfg(not(test))]
fn resolve_browser_asset_root() -> Result<std::path::PathBuf, String> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        candidates.extend(installed_browser_asset_candidates(&executable));
    }
    candidates
        .push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dist/browser-diff"));

    candidates
        .iter()
        .find(|candidate| candidate.is_dir())
        .cloned()
        .ok_or_else(|| {
            "Browser diff viewer assets are missing. Reinstall or upgrade Norn, or run `pnpm run browser-diff:build` from a source checkout."
                .to_string()
        })
}

fn installed_browser_asset_candidates(executable: &Path) -> Vec<PathBuf> {
    let Some(executable_dir) = executable.parent() else {
        return Vec::new();
    };
    if executable_dir.file_name().is_some_and(|name| name == "bin") {
        return executable_dir
            .parent()
            .map(|prefix| vec![prefix.join("share/norn/browser-diff")])
            .unwrap_or_default();
    }
    vec![executable_dir.join("share/norn/browser-diff")]
}

fn collect_browser_assets(
    root: &Path,
    directory: &Path,
    files: &mut HashMap<String, BrowserAsset>,
    total_bytes: &mut u64,
) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("Failed to read browser diff assets: {error}"))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("Failed to read browser diff asset: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Failed to inspect browser diff asset: {error}"))?;
        if file_type.is_symlink() {
            return Err("Browser diff assets must not contain symbolic links.".to_string());
        }
        if file_type.is_dir() {
            collect_browser_assets(root, &path, files, total_bytes)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        if files.len() >= MAX_BROWSER_ASSET_FILES {
            return Err("Browser diff asset count exceeds the configured limit.".to_string());
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Failed to inspect browser diff asset: {error}"))?;
        if metadata.len() > MAX_BROWSER_ASSET_BYTES {
            return Err("A browser diff asset exceeds the configured size limit.".to_string());
        }
        *total_bytes = total_bytes.saturating_add(metadata.len());
        if *total_bytes > MAX_BROWSER_ASSET_TOTAL_BYTES {
            return Err("Browser diff assets exceed the configured total size limit.".to_string());
        }
        let content_type = browser_asset_content_type(&path)
            .ok_or_else(|| "A browser diff asset has an unsupported file type.".to_string())?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Browser diff asset escaped its root directory.".to_string())?;
        let route = browser_asset_route(relative)?;
        let body = fs::read(&path)
            .map_err(|error| format!("Failed to read a packaged browser diff asset: {error}"))?;
        files.insert(
            route,
            BrowserAsset {
                content_type,
                body: Arc::from(body),
            },
        );
    }
    Ok(())
}

fn browser_asset_route(relative: &Path) -> Result<String, String> {
    let relative = relative
        .to_str()
        .ok_or_else(|| "Browser diff asset paths must be valid UTF-8.".to_string())?;
    Ok(format!("/{}", relative.replace('\\', "/")))
}

fn browser_asset_content_type(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|extension| extension.to_str())? {
        "html" => Some("text/html; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "js" => Some("text/javascript; charset=utf-8"),
        "woff" => Some("font/woff"),
        "woff2" => Some("font/woff2"),
        _ => None,
    }
}

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
        let browser_assets = {
            #[cfg(test)]
            {
                Arc::new(BrowserAssets::fixture())
            }
            #[cfg(not(test))]
            {
                Arc::new(BrowserAssets::load_default()?)
            }
        };
        Self::start_with_assets(state, browser_assets)
    }

    fn start_with_assets(
        state: Arc<RwLock<WebDiffState>>,
        browser_assets: Arc<BrowserAssets>,
    ) -> Result<Self, String> {
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
        let available_connection_slots = Arc::new(AtomicUsize::new(MAX_ACTIVE_CONNECTIONS));

        thread::spawn(move || {
            let _ = listener.set_nonblocking(true);
            while !thread_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if !try_acquire_connection_slot(&available_connection_slots) {
                            let mut stream = stream;
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
                            let _ = write_empty_response(&mut stream, "503 Service Unavailable");
                            continue;
                        }
                        let st = Arc::clone(&thread_state);
                        let token = Arc::clone(&thread_token);
                        let fetch_lock = Arc::clone(&populate_lock);
                        let assets = Arc::clone(&browser_assets);
                        let slot = ConnectionSlot::new(Arc::clone(&available_connection_slots));
                        thread::spawn(move || {
                            let _slot = slot;
                            let _ = handle_connection(stream, st, &token, fetch_lock, assets);
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

struct ConnectionSlot {
    available: Arc<AtomicUsize>,
}

impl ConnectionSlot {
    fn new(available: Arc<AtomicUsize>) -> Self {
        Self { available }
    }
}

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.available.fetch_add(1, Ordering::Release);
    }
}

fn try_acquire_connection_slot(available: &AtomicUsize) -> bool {
    let mut slots = available.load(Ordering::Acquire);
    loop {
        if slots == 0 {
            return false;
        }
        match available.compare_exchange_weak(slots, slots - 1, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => return true,
            Err(current) => slots = current,
        }
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
    browser_assets: Arc<BrowserAssets>,
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
    if route == "/" && !path.ends_with('/') {
        return write_session_redirect(&mut stream, session_token);
    }
    if head_only && matches!(route, "/api/state" | "/api/diff" | "/api/file-preview") {
        return write_empty_response(&mut stream, "405 Method Not Allowed");
    }

    if let Some(asset) = browser_assets.get(route) {
        return write_response(
            &mut stream,
            "200 OK",
            asset.content_type,
            "no-store",
            &asset.body,
            head_only,
        );
    }

    match route {
        "/" | "/index.html" => {
            write_response(
                &mut stream,
                "500 Internal Server Error",
                "text/plain; charset=utf-8",
                "no-store",
                b"Browser diff viewer assets are unavailable.",
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
            if file_path.is_empty() || (side != "old" && side != "new") {
                return write_empty_response(&mut stream, "400 Bad Request");
            }
            if !is_raster_preview_path(&file_path) {
                return write_empty_response(&mut stream, "415 Unsupported Media Type");
            }
            let initial_state = state
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            if initial_state.workspace.is_empty()
                || initial_state.repo.is_empty()
                || initial_state.pr_id == 0
            {
                return write_empty_response(&mut stream, "400 Bad Request");
            }
            if initial_state.diffstat.is_some()
                && !is_allowed_preview_path(initial_state.diffstat.as_deref(), &file_path, &side)
            {
                return write_empty_response(&mut stream, "404 Not Found");
            }
            let state_data = if initial_state.diffstat.is_some() {
                initial_state
            } else {
                get_or_populate_diffstat(&state, &populate_lock)
            };
            if !is_allowed_preview_path(state_data.diffstat.as_deref(), &file_path, &side) {
                return write_empty_response(&mut stream, "404 Not Found");
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
        if let Some(terminator) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            request.truncate(terminator + 4);
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

fn write_session_redirect(stream: &mut TcpStream, session_token: &str) -> std::io::Result<()> {
    let headers = format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: /session/{session_token}/\r\nContent-Length: 0\r\nCache-Control: no-store\r\n{SECURITY_HEADERS}Connection: close\r\n\r\n"
    );
    stream.write_all(headers.as_bytes())
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
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: {cache_control}\r\n{SECURITY_HEADERS}Connection: close\r\n\r\n",
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

fn get_or_populate_diffstat(
    state: &Arc<RwLock<WebDiffState>>,
    populate_lock: &Arc<Mutex<()>>,
) -> WebDiffState {
    let snapshot = state
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if snapshot.diffstat.is_some() || snapshot.population_failed || !needs_population(&snapshot) {
        return snapshot;
    }

    let _guard = populate_lock
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let snapshot = state
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    if snapshot.diffstat.is_some() || snapshot.population_failed || !needs_population(&snapshot) {
        return snapshot;
    }

    let diffstat_result = get_diffstat_native(
        snapshot.provider,
        &snapshot.workspace,
        &snapshot.repo,
        snapshot.pr_id,
    );
    let mut lock = state
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if same_pull_request(&lock, &snapshot) {
        match diffstat_result {
            Ok(diffstat) => lock.diffstat = Some(diffstat),
            Err(_) => lock.population_failed = true,
        }
        lock.version += 1;
    }
    lock.clone()
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
    fn browser_server_connection_slots_are_bounded_and_reusable() {
        let available = Arc::new(AtomicUsize::new(1));
        assert!(try_acquire_connection_slot(&available));
        assert!(!try_acquire_connection_slot(&available));
        drop(ConnectionSlot::new(Arc::clone(&available)));
        assert!(try_acquire_connection_slot(&available));
    }

    #[test]
    fn browser_assets_require_a_built_entry_point_and_supported_files() {
        let directory = tempfile::tempdir().expect("temporary asset directory");
        let missing = BrowserAssets::load_from(directory.path())
            .err()
            .expect("missing entry point should fail");
        assert!(missing.contains("browser-diff.html"));

        fs::create_dir(directory.path().join("assets")).expect("create assets directory");
        fs::write(
            directory.path().join("browser-diff.html"),
            b"<div id=\"root\"></div>",
        )
        .expect("write browser entry point");
        fs::write(directory.path().join("assets/viewer.js"), b"export {};")
            .expect("write browser script");
        let assets = BrowserAssets::load_from(directory.path()).expect("load browser assets");
        assert_eq!(
            assets.get("/").map(|asset| asset.content_type),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            assets
                .get("/assets/viewer.js")
                .map(|asset| asset.content_type),
            Some("text/javascript; charset=utf-8")
        );
    }

    #[cfg(unix)]
    #[test]
    fn browser_assets_reject_non_utf8_routes() {
        use std::{ffi::OsString, os::unix::ffi::OsStringExt};

        let path = PathBuf::from(OsString::from_vec(vec![0xff, b'.', b'j', b's']));
        let error = browser_asset_route(&path).expect_err("non-UTF-8 route should fail");
        assert!(error.contains("valid UTF-8"));
    }

    #[test]
    fn browser_asset_candidates_match_prefix_and_archive_layouts() {
        assert_eq!(
            installed_browser_asset_candidates(Path::new("/opt/norn/bin/norn-tui")),
            vec![PathBuf::from("/opt/norn/share/norn/browser-diff")]
        );
        assert_eq!(
            installed_browser_asset_candidates(Path::new("/tmp/norn-archive/norn-tui")),
            vec![PathBuf::from("/tmp/norn-archive/share/norn/browser-diff")]
        );
    }

    #[test]
    fn browser_server_serves_shared_ui_assets_only_inside_the_session() {
        let server = WebDiffServer::start(Arc::new(RwLock::new(WebDiffState::default())))
            .expect("server should start");
        let document = request(
            server.port,
            &format!(
                "GET /session/{}/ HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                server.session_token
            ),
        );
        assert!(document.starts_with("HTTP/1.1 200 OK"));
        assert!(document.contains("./assets/viewer.js"));
        assert!(document.contains("script-src 'self'"));
        assert!(!document.contains("script-src 'unsafe-inline'"));

        let script = request(
            server.port,
            &format!(
                "GET /session/{}/assets/viewer.js HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                server.session_token
            ),
        );
        assert!(script.starts_with("HTTP/1.1 200 OK"));
        assert!(script.contains("Content-Type: text/javascript; charset=utf-8"));
        assert!(script.ends_with("document.title = 'Norn';"));

        let unauthenticated = request(
            server.port,
            "GET /assets/viewer.js HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        );
        assert!(unauthenticated.starts_with("HTTP/1.1 404 Not Found"));
    }

    #[test]
    fn browser_server_redirects_the_session_root_to_a_trailing_slash() {
        let server = WebDiffServer::start(Arc::new(RwLock::new(WebDiffState::default())))
            .expect("server should start");
        let response = request(
            server.port,
            &format!(
                "GET /session/{} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                server.session_token
            ),
        );

        assert!(response.starts_with("HTTP/1.1 307 Temporary Redirect"));
        assert!(response.contains(&format!("Location: /session/{}/\r\n", server.session_token)));
        assert!(response.contains("Content-Length: 0"));
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
    fn request_headers_exclude_bytes_after_the_first_terminator() {
        let mut reader = Cursor::new(
            b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\nGET /next HTTP/1.1\r\n\r\n".to_vec(),
        );
        let request = read_request_head(&mut reader)
            .expect("request should parse")
            .expect("request should be present");
        assert_eq!(request, b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n");
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
    fn head_state_requests_never_populate_provider_data() {
        let state = Arc::new(RwLock::new(WebDiffState {
            version: 4,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 42,
            ..WebDiffState::default()
        }));
        let server = WebDiffServer::start(Arc::clone(&state)).expect("server should start");

        let response = request(
            server.port,
            &format!(
                "HEAD /session/{}/api/state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                server.session_token
            ),
        );

        assert!(response.starts_with("HTTP/1.1 405 Method Not Allowed"));
        let state = state.read().unwrap();
        assert_eq!(state.version, 4);
        assert!(state.diff.is_none());
        assert!(state.diffstat.is_none());
        assert!(!state.population_failed);
    }

    #[test]
    fn invalid_preview_requests_never_populate_provider_data() {
        let state = Arc::new(RwLock::new(WebDiffState {
            version: 5,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id: 42,
            ..WebDiffState::default()
        }));
        let server = WebDiffServer::start(Arc::clone(&state)).expect("server should start");

        let response = request(
            server.port,
            &format!(
                "GET /session/{}/api/file-preview?path=&side=new HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                server.session_token
            ),
        );

        assert!(response.starts_with("HTTP/1.1 400 Bad Request"));
        let state = state.read().unwrap();
        assert_eq!(state.version, 5);
        assert!(state.diff.is_none());
        assert!(state.diffstat.is_none());
        assert!(!state.population_failed);
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
    fn pull_request_changes_advance_the_session_version() {
        let state = Arc::new(RwLock::new(WebDiffState::default()));
        let server = WebDiffServer::start(Arc::clone(&state)).expect("server should start");
        let settled = |pr_id, title: &str| WebDiffState {
            provider: None,
            workspace: "workspace".to_string(),
            repo: "repo".to_string(),
            pr_id,
            pr_title: title.to_string(),
            diff: Some(String::new()),
            diffstat: Some(Vec::new()),
            ..WebDiffState::default()
        };

        server.update_pr(settled(7, "First"));
        let first_version = state.read().unwrap().version;
        server.update_pr(settled(8, "Second"));

        let current = state.read().unwrap();
        assert_eq!(current.pr_id, 8);
        assert_eq!(current.version, first_version + 1);
        assert!(!state_is_unchanged_and_settled(
            &current,
            Some(first_version)
        ));
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
