use super::*;
pub(crate) fn run(
    root: &Path,
    args: &[&str],
    maximum: usize,
    control: &GitRunControl,
) -> Result<crate::local_review::BoundedGitOutput, String> {
    control.check()?;
    let mut literal = vec!["--literal-pathspecs"];
    literal.extend_from_slice(args);
    crate::local_review::run_git_bounded_with_control(root, &literal, None, maximum, "Repository exploration", control)
        .map_err(|_| "Repository command failed or exceeded its preview limits. Refresh the repository or use an external editor.".into())
}
pub(crate) fn checked(
    root: &Path,
    args: &[&str],
    maximum: usize,
    control: &GitRunControl,
) -> Result<Vec<u8>, String> {
    let output = run(root, args, maximum, control)?;
    if !output.status.success() {
        return Err("The selected repository operation is unavailable.".into());
    }
    Ok(output.stdout)
}
pub(crate) fn head(root: &Path, control: &GitRunControl) -> Result<Option<String>, String> {
    let output = run(root, &["rev-parse", "--verify", "-q", "HEAD"], 256, control)?;
    if output.status.code() == Some(1) {
        return Ok(None);
    }
    if !output.status.success() {
        return Err(invalid());
    }
    let oid = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid())?
        .trim();
    if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(Some(oid.into()))
}
