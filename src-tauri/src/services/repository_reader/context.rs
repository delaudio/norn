use super::*;
use crate::services::local_snapshot::{resolve_source, Source};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub repository: RepositoryIdentity,
    pub root_id: String,
}
impl Context {
    pub fn validate(&self) -> Result<(), String> {
        self.repository.validate()?;
        if !valid_hash(&self.root_id) {
            return Err(invalid());
        }
        Ok(())
    }
}
pub(crate) fn resolve(
    repository: &RepositoryIdentity,
    control: &GitRunControl,
) -> Result<crate::services::local_snapshot::ResolvedSource, String> {
    resolve_source(
        &Source::Configured {
            repository: repository.clone(),
        },
        Path::new(""),
        control,
    )
}
pub(crate) fn resolve_context(
    context: &Context,
    control: &GitRunControl,
) -> Result<PathBuf, String> {
    context.validate()?;
    let source = resolve(&context.repository, control)?;
    if source.root_id != context.root_id {
        return Err("The selected repository checkout changed. Refresh it explicitly.".into());
    }
    Ok(source.root)
}
pub(crate) fn validate_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 4096
        || path.contains('\0')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err("Select a safe repository-relative file path.".into());
    }
    #[cfg(windows)]
    if path.contains(['\\', ':']) {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn verified_file(
    root: &Path,
    path: &str,
    control: &GitRunControl,
) -> Result<Option<std::fs::File>, String> {
    validate_path(path)?;
    control.check()?;
    let candidate = root.join(path);
    let canonical = match candidate.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("This repository file is unavailable.".into()),
    };
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| "Repository file leaves the selected checkout.".to_string())?;
    let relative = relative.to_str().ok_or_else(invalid)?;
    #[cfg(windows)]
    let relative = relative.replace('\\', "/");
    #[cfg(windows)]
    let relative = relative.as_str();
    let file = crate::services::local_fs::open_file(root, relative, control)?;
    // Following a legitimate internal link does not permit reading a changed
    // link destination after canonicalization; the opened handle is confined.
    if candidate.canonicalize().map_err(|_| invalid())? != canonical {
        return Err("Repository file changed while being read. Refresh it.".into());
    }
    Ok(file)
}
