use super::*;
use std::io::Read;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContentState {
    Text,
    Binary,
    TooLarge,
    Missing,
    Deleted,
    Unsupported,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentParams {
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
impl ContentParams {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        context::validate_path(&self.path)?;
        if self.offset > MAX_FILE_BYTES
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
pub(crate) struct ContentPage {
    pub request: ContentParams,
    pub state: ContentState,
    pub size: Option<u64>,
    pub content_sha256: Option<String>,
    pub text: String,
    pub next_offset: Option<usize>,
}
pub(crate) struct Document {
    pub state: ContentState,
    pub size: Option<u64>,
    pub text: String,
    pub sha: Option<String>,
}
pub(crate) fn read_at(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<Document, String> {
    let Some(mut file) = context::verified_file(root, path, control)? else {
        return Ok(Document {
            state: ContentState::Missing,
            size: None,
            text: String::new(),
            sha: None,
        });
    };
    let metadata = file.metadata().map_err(|_| invalid())?;
    if !metadata.is_file() {
        return Ok(Document {
            state: ContentState::Unsupported,
            size: None,
            text: String::new(),
            sha: None,
        });
    }
    let size = metadata.len();
    if size > MAX_FILE_BYTES as u64 {
        return Ok(Document {
            state: ContentState::TooLarge,
            size: Some(size),
            text: String::new(),
            sha: None,
        });
    }
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        control.check()?;
        let count = file.read(&mut buffer).map_err(|_| invalid())?;
        if count == 0 {
            break;
        }
        if bytes.len() + count > MAX_FILE_BYTES {
            return Err(
                "Repository file grew beyond its preview limit. Refresh it explicitly.".into(),
            );
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    let after = file.metadata().map_err(|_| invalid())?;
    if after.len() != size
        || bytes.len() as u64 != size
        || after.modified().ok() != metadata.modified().ok()
    {
        return Err("Repository file changed during reading. Refresh it explicitly.".into());
    }
    let sha = hash(&bytes);
    if bytes.contains(&0) {
        return Ok(Document {
            state: ContentState::Binary,
            size: Some(size),
            text: String::new(),
            sha: Some(sha),
        });
    }
    match String::from_utf8(bytes) {
        Ok(text) => Ok(Document {
            state: ContentState::Text,
            size: Some(size),
            text,
            sha: Some(sha),
        }),
        Err(_) => Ok(Document {
            state: ContentState::Binary,
            size: Some(size),
            text: String::new(),
            sha: Some(sha),
        }),
    }
}
pub(crate) fn page(params: ContentParams, control: &GitRunControl) -> Result<ContentPage, String> {
    params.validate()?;
    let root = context::resolve_context(&params.context, control)?;
    let inventory = inventory::collect(&root, control)?;
    let selected = inventory
        .files
        .iter()
        .find(|file| file.path == params.path)
        .ok_or_else(|| "Select a file from the current repository inventory.".to_string())?;
    let mut document = read_at(&root, &params.path, control)?;
    if document.state == ContentState::Missing && selected.status == RepositoryFileStatus::Deleted {
        document.state = ContentState::Deleted;
    }
    if params
        .content_sha256
        .as_ref()
        .is_some_and(|expected| document.sha.as_ref() != Some(expected))
    {
        return Err("Repository content changed. Refresh the selected file explicitly.".into());
    }
    let (text, next_offset) = if document.state == ContentState::Text {
        if params.offset > document.text.len() || !document.text.is_char_boundary(params.offset) {
            return Err(invalid());
        }
        let mut end = (params.offset + params.max_bytes).min(document.text.len());
        while !document.text.is_char_boundary(end) {
            end -= 1;
        }
        if end == params.offset && end < document.text.len() {
            return Err(invalid());
        }
        (
            document.text[params.offset..end].to_owned(),
            (end < document.text.len()).then_some(end),
        )
    } else {
        if params.offset != 0 {
            return Err(invalid());
        }
        (String::new(), None)
    };
    if context::resolve_context(&params.context, control)? != root {
        return Err(invalid());
    }
    Ok(ContentPage {
        request: params,
        state: document.state,
        size: document.size,
        content_sha256: document.sha,
        text,
        next_offset,
    })
}
pub(crate) fn legacy_read(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<RepositoryFileContent, String> {
    let document = read_at(root, path, control)?;
    match document.state {
        ContentState::Text => Ok(RepositoryFileContent {
            path: path.into(),
            content: document.text,
            size: document.size.unwrap_or(0),
            truncated: false,
        }),
        ContentState::TooLarge => Err("File is too large to preview (512 KB limit).".into()),
        ContentState::Binary => Err("File is not valid UTF-8 and cannot be previewed.".into()),
        _ => Err("Repository file is unavailable.".into()),
    }
}
