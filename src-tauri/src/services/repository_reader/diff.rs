use super::*;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiffParams {
    pub context: Context,
    pub path: String,
    pub offset: usize,
    pub max_bytes: usize,
    #[serde(deserialize_with = "required_hash")]
    pub content_sha256: Option<String>,
}
fn required_hash<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
impl DiffParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        context::validate_path(&self.path)?;
        if self.offset > MAX_DIFF
            || !(4..=CHUNK).contains(&self.max_bytes)
            || self
                .content_sha256
                .as_ref()
                .is_some_and(|value| !valid_hash(value))
            || self.offset > 0 && self.content_sha256.is_none()
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffPage {
    pub request: DiffParams,
    pub base_sha: Option<String>,
    pub total_bytes: usize,
    pub content_sha256: String,
    pub text: String,
    pub next_offset: Option<usize>,
}
pub(crate) fn read_at(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<(String, Option<String>), String> {
    context::validate_path(path)?;
    let files = inventory::collect(root, control)?;
    let file = files
        .files
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(|| "Select a file from the current repository inventory.".to_string())?;
    let before = content::read_at(root, path, control)?;
    if before.state == ContentState::Unsupported {
        return Err("This repository path cannot be diffed as a file.".into());
    }
    let head = git::head(root, control)?;
    let output = if head.is_none() || file.status == RepositoryFileStatus::Untracked {
        if before.state == ContentState::Missing {
            return Ok((String::new(), head));
        }
        // Git's null-device spelling is supported by Git for Windows as well.
        git::run(
            root,
            &[
                "diff",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--no-index",
                "--",
                "/dev/null",
                path,
            ],
            MAX_DIFF,
            control,
        )?
    } else {
        let mut args = vec![
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            head.as_deref().ok_or_else(invalid)?,
            "--",
            path,
        ];
        if let Some(old) = file.old_path.as_deref() {
            args.push(old);
        }
        git::run(root, &args, MAX_DIFF, control)?
    };
    let allowed = output.status.success()
        || ((head.is_none() || file.status == RepositoryFileStatus::Untracked)
            && output.status.code() == Some(1));
    if !allowed {
        return Err("The repository diff is unavailable.".into());
    }
    let text = String::from_utf8(output.stdout)
        .map_err(|_| "This diff is not valid UTF-8 and cannot be previewed.".to_string())?;
    let after = content::read_at(root, path, control)?;
    if before.state != after.state
        || before.sha != after.sha
        || before.size != after.size
        || git::head(root, control)? != head
    {
        return Err("Repository content changed during diff loading. Refresh explicitly.".into());
    }
    Ok((text, head))
}
pub(crate) fn page(params: DiffParams, control: &GitRunControl) -> Result<DiffPage, String> {
    params.validate()?;
    let root = context::resolve_context(&params.context, control)?;
    let (document, base_sha) = read_at(&root, &params.path, control)?;
    let sha = hash(document.as_bytes());
    if params
        .content_sha256
        .as_ref()
        .is_some_and(|expected| expected != &sha)
    {
        return Err("Repository diff changed. Refresh the selected file explicitly.".into());
    }
    if params.offset > document.len() || !document.is_char_boundary(params.offset) {
        return Err(invalid());
    }
    let mut end = (params.offset + params.max_bytes).min(document.len());
    while !document.is_char_boundary(end) {
        end -= 1;
    }
    if end == params.offset && end < document.len() {
        return Err(invalid());
    }
    if context::resolve_context(&params.context, control)? != root {
        return Err(invalid());
    }
    let text = document[params.offset..end].into();
    Ok(DiffPage {
        request: params,
        base_sha,
        total_bytes: document.len(),
        content_sha256: sha,
        text,
        next_offset: (end < document.len()).then_some(end),
    })
}
pub(crate) fn legacy_diff(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<RepositoryFileDiff, String> {
    let (raw_diff, _) = read_at(root, path, control)?;
    Ok(RepositoryFileDiff {
        path: path.into(),
        raw_diff,
    })
}
