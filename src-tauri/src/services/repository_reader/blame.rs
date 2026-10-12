use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BlameParams {
    pub context: Context,
    pub path: String,
    pub offset: usize,
    pub limit: usize,
    pub content_sha256: String,
}
impl BlameParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        context::validate_path(&self.path)?;
        if self.offset > MAX_FILE_BYTES
            || !(1..=32).contains(&self.limit)
            || !valid_hash(&self.content_sha256)
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BlamePage {
    pub request: BlameParams,
    pub base_sha: Option<String>,
    pub available: bool,
    pub total_lines: usize,
    pub lines: Vec<RepositoryBlameLine>,
    pub next_offset: Option<usize>,
}

pub(super) fn read_lines(
    root: &Path,
    path: &str,
    offset: usize,
    end: usize,
    control: &GitRunControl,
) -> Result<Vec<RepositoryBlameLine>, String> {
    let range = format!("{},{}", offset + 1, end);
    let raw = git::checked(
        root,
        &["blame", "--line-porcelain", "-L", &range, "--", path],
        MAX_METADATA,
        control,
    )?;
    let raw =
        String::from_utf8(raw).map_err(|_| "Blame metadata is not valid UTF-8.".to_string())?;
    let mut lines = legacy::parse_git_blame_line_porcelain(&raw);
    if lines.len() != end - offset
        || lines.iter().enumerate().any(|(index, line)| {
            line.line as usize != offset + index + 1
                || !matches!(line.sha.len(), 40 | 64)
                || !line.sha.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err(invalid());
    }
    let mut messages = BTreeMap::new();
    let mut assigned_bytes = raw.len();
    for line in &mut lines {
        control.check()?;
        if line.sha.bytes().all(|byte| byte == b'0') {
            continue;
        }
        if !messages.contains_key(&line.sha) {
            // Read each immutable commit separately: commit text may contain any
            // delimiter, including the legacy batch parser's marker.
            let bytes = git::checked(
                root,
                &["show", "-s", "--format=%B", &line.sha, "--"],
                CHUNK,
                control,
            )?;
            let message = String::from_utf8(bytes)
                .map_err(|_| "Commit message is not valid UTF-8.".to_string())?;
            messages.insert(line.sha.clone(), message.trim().to_owned());
        }
        let message = messages.get(&line.sha).ok_or_else(invalid)?;
        assigned_bytes = assigned_bytes.saturating_add(message.len());
        if assigned_bytes > MAX_METADATA {
            return Err("Blame metadata exceeds its preview limit.".into());
        }
        line.message = Some(message.clone());
    }
    Ok(lines)
}

pub(crate) fn legacy_blame(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<Vec<RepositoryBlameLine>, String> {
    let files = inventory::collect(root, control)?;
    let selected = files
        .files
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(invalid)?;
    let before = content::read_at(root, path, control)?;
    if before.state != ContentState::Text {
        return Err("Blame is available for text previews only.".into());
    }
    let head = git::head(root, control)?;
    if head.is_none()
        || matches!(
            selected.status,
            RepositoryFileStatus::Untracked | RepositoryFileStatus::Added
        )
    {
        return Ok(Vec::new());
    }
    let count = before.text.lines().count();
    let lines = if count == 0 {
        Vec::new()
    } else {
        read_lines(root, path, 0, count, control)?
    };
    let after = content::read_at(root, path, control)?;
    if before.sha != after.sha || git::head(root, control)? != head {
        return Err("Repository changed during blame loading. Refresh explicitly.".into());
    }
    Ok(lines)
}

pub(crate) fn page(params: BlameParams, control: &GitRunControl) -> Result<BlamePage, String> {
    params.validate()?;
    let root = context::resolve_context(&params.context, control)?;
    let inventory = inventory::collect(&root, control)?;
    let selected = inventory
        .files
        .iter()
        .find(|file| file.path == params.path)
        .ok_or_else(invalid)?;
    let document = content::read_at(&root, &params.path, control)?;
    if document.sha.as_ref() != Some(&params.content_sha256) || document.state != ContentState::Text
    {
        return Err("Load the current text preview before requesting blame.".into());
    }
    let total = document.text.lines().count();
    if params.offset > total {
        return Err(invalid());
    }
    let head = git::head(&root, control)?;
    let available = head.is_some()
        && !matches!(
            selected.status,
            RepositoryFileStatus::Untracked
                | RepositoryFileStatus::Added
                | RepositoryFileStatus::Conflicted
        );
    let end = (params.offset + params.limit).min(total);
    let mut lines = if available && end > params.offset {
        read_lines(&root, &params.path, params.offset, end, control)?
    } else {
        Vec::new()
    };
    // Repeated full commit messages can exceed a transport frame. Reduce the
    // page, retaining complete rows and advancing by the actual returned count.
    while serde_json::to_vec(&lines).map_err(|_| invalid())?.len() > 256 * 1024 {
        if lines.len() <= 1 {
            return Err("Blame metadata exceeds its preview limit.".into());
        }
        lines.pop();
    }
    let after = content::read_at(&root, &params.path, control)?;
    if after.sha != document.sha
        || git::head(&root, control)? != head
        || context::resolve_context(&params.context, control)? != root
    {
        return Err("Repository content changed during blame loading. Refresh explicitly.".into());
    }
    let next = params.offset + lines.len();
    Ok(BlamePage {
        request: params,
        base_sha: head,
        available,
        total_lines: total,
        lines,
        next_offset: (available && next < total).then_some(next),
    })
}
