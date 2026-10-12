use super::*;
#[cfg(target_os = "macos")]
use std::process::Command;
use std::{env, fs, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Invocation {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub label: String,
}
#[derive(Clone, PartialEq, Eq)]
pub(super) enum Integration {
    Command(Invocation),
    #[cfg(windows)]
    System,
}

fn executable(path: PathBuf) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    #[cfg(windows)]
    {
        if !matches!(
            path.extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("exe" | "com")
        ) {
            return None;
        }
    }
    Some(path)
}

fn lookup(name: &str, control: &GitRunControl) -> Result<Option<PathBuf>, String> {
    control.check()?;
    let requested = Path::new(name);
    if requested.is_absolute() {
        return Ok(executable(requested.to_owned()));
    }
    if requested.components().count() != 1 {
        return Ok(None);
    }
    if let Some(paths) = env::var_os("PATH") {
        for directory in env::split_paths(&paths) {
            if let Some(path) = executable(directory.join(name)) {
                return Ok(Some(path));
            }
            #[cfg(windows)]
            if requested.extension().is_none() {
                if let Some(path) = executable(directory.join(format!("{name}.exe"))) {
                    return Ok(Some(path));
                }
            }
        }
    }
    // GUI launches on macOS may lack user install paths. The fixed login-shell
    // expression only resolves a positional command name; file data is never
    // interpolated, executed or sent to shell stdin.
    #[cfg(target_os = "macos")]
    {
        let output = crate::local_review::run_command_bounded_with_control(
            Command::new("/bin/zsh").args(["-lc", "command -v -- \"$1\"", "norn-editor", name]),
            None,
            4096,
            "Editor lookup",
            control,
        )
        .map_err(|_| "Editor lookup failed or exceeded its limits.".to_owned())?;
        if output.status.success() {
            let value = std::str::from_utf8(&output.stdout)
                .map_err(|_| invalid())?
                .trim_end_matches(['\r', '\n']);
            if Path::new(value).is_absolute() {
                return Ok(executable(PathBuf::from(value)));
            }
        }
    }
    Ok(None)
}

#[cfg(windows)]
fn gui_executable(name: &str) -> Option<PathBuf> {
    let filename = if name == "code" {
        "Code.exe"
    } else if name == "cursor" {
        "Cursor.exe"
    } else {
        return None;
    };
    // Preserve the installed CLI integration without executing its .cmd shim.
    // VS Code/Cursor's bin/<name>.cmd delegates to this adjacent native app.
    for directory in env::split_paths(&env::var_os("PATH")?) {
        if directory.join(format!("{name}.cmd")).is_file() {
            if let Some(parent) = directory.parent() {
                if let Some(path) = executable(parent.join(filename)) {
                    return Some(path);
                }
            }
        }
    }
    None
}

pub(super) fn select(
    params: &PrepareParams,
    file: &Path,
    control: &GitRunControl,
) -> Result<Option<Integration>, String> {
    if params.mode == Mode::Terminal {
        let value = env::var("VISUAL")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| env::var("EDITOR").ok().filter(|s| !s.trim().is_empty()));
        let Some(value) = value else {
            return Ok(None);
        };
        if value.len() > 4096 || value.chars().any(char::is_control) {
            return Err("The configured terminal editor command is invalid.".into());
        }
        // An existing absolute path remains exact, including Windows backslashes
        // and spaces. Otherwise parse bounded argv, without expansion or a shell.
        let parts = if Path::new(&value).is_absolute() && Path::new(&value).is_file() {
            vec![value]
        } else {
            configured_argv(&value)?
        };
        if parts.is_empty() || parts.len() > 32 {
            return Err("The configured terminal editor command is invalid.".into());
        }
        let Some(program) = lookup(&parts[0], control)? else {
            return Ok(None);
        };
        let label = Path::new(&parts[0])
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Terminal editor")
            .to_owned();
        if label.len() > 256 {
            return Err("The configured editor name exceeds its limits.".into());
        }
        // Repository file data must not enter command-language interpreters.
        if matches!(
            label.to_ascii_lowercase().as_str(),
            "sh" | "bash"
                | "zsh"
                | "fish"
                | "cmd"
                | "powershell"
                | "pwsh"
                | "python"
                | "python3"
                | "node"
                | "bun"
        ) {
            return Err(
                "Configure an editor executable, rather than a command interpreter.".into(),
            );
        }
        let mut args = parts[1..].to_vec();
        if let Some(line) = params.line {
            if matches!(
                label.to_ascii_lowercase().as_str(),
                "vim" | "nvim" | "vi" | "nano" | "emacs" | "micro"
            ) {
                args.push(format!("+{line}"));
            } else if matches!(label.to_ascii_lowercase().as_str(), "hx" | "helix") {
                args.push(position(file, Some(line), false));
                return Ok(Some(Integration::Command(Invocation {
                    program,
                    args,
                    label,
                })));
            } else if matches!(
                label.to_ascii_lowercase().as_str(),
                "zed" | "cursor" | "code"
            ) {
                if label.eq_ignore_ascii_case("zed") {
                    args.push(position(file, Some(line), true));
                } else {
                    args.extend(["-g".into(), position(file, Some(line), false)]);
                }
                return Ok(Some(Integration::Command(Invocation {
                    program,
                    args,
                    label,
                })));
            } else {
                return Err("This editor's source-line option is unavailable. Choose a supported terminal editor or open without a line position.".into());
            }
        }
        args.push(file.to_str().ok_or_else(invalid)?.to_owned());
        return Ok(Some(Integration::Command(Invocation {
            program,
            args,
            label,
        })));
    }
    for name in ["zed", "cursor", "code"] {
        let program = lookup(name, control)?;
        #[cfg(windows)]
        let program = program.or_else(|| gui_executable(name));
        if let Some(program) = program {
            let args = if name == "zed" {
                vec![position(file, params.line, true)]
            } else {
                vec!["-g".into(), position(file, params.line, false)]
            };
            return Ok(Some(Integration::Command(Invocation {
                program,
                args,
                label: name.into(),
            })));
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(program) = executable(PathBuf::from("/usr/bin/open")) {
        return Ok(Some(Integration::Command(Invocation {
            program,
            args: vec![file.to_str().ok_or_else(invalid)?.into()],
            label: "System application".into(),
        })));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(program) = lookup("xdg-open", control)? {
        return Ok(Some(Integration::Command(Invocation {
            program,
            args: vec![file.to_str().ok_or_else(invalid)?.into()],
            label: "System application".into(),
        })));
    }
    #[cfg(windows)]
    {
        return Ok(Some(Integration::System));
    }
    #[cfg(not(windows))]
    Ok(None)
}

fn position(file: &Path, line: Option<u32>, column: bool) -> String {
    match line {
        Some(line) if column => format!("{}:{line}:1", file.display()),
        Some(line) => format!("{}:{line}", file.display()),
        None => file.display().to_string(),
    }
}
#[cfg(not(windows))]
fn configured_argv(value: &str) -> Result<Vec<String>, String> {
    shlex::split(value)
        .ok_or_else(|| "The configured terminal editor command has invalid quotes.".into())
}
#[cfg(windows)]
fn configured_argv(value: &str) -> Result<Vec<String>, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{Foundation::LocalFree, UI::Shell::CommandLineToArgvW};
    let wide: Vec<u16> = std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut count = 0;
    let args = unsafe { CommandLineToArgvW(wide.as_ptr(), &mut count) };
    if args.is_null() {
        return Err(invalid());
    }
    let result = (|| {
        let mut result = Vec::new();
        for pointer in unsafe { std::slice::from_raw_parts(args, count as usize) } {
            let mut length = 0;
            while unsafe { *pointer.add(length) } != 0 {
                length += 1;
            }
            result.push(
                String::from_utf16(unsafe { std::slice::from_raw_parts(*pointer, length) })
                    .map_err(|_| invalid())?,
            );
        }
        Ok(result)
    })();
    unsafe {
        LocalFree(args.cast());
    }
    result
}

#[cfg(windows)]
pub(super) fn system_open(file: &Path) -> Result<(), String> {
    system_open_with(file, |value| unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            std::ptr::null(),
            value.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        ) as isize
    })
}
#[cfg(windows)]
fn system_open_with(file: &Path, invoke: impl FnOnce(&[u16]) -> isize) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    let value: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
    if value[..value.len() - 1].contains(&0) {
        return Err(invalid());
    }
    if invoke(&value) <= 32 {
        return Err("No system application could open this file.".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_positions_keep_literal_unicode_controls_and_option_like_filenames() {
        let file = Path::new("/owned checkout/--ü & $(owned);\nfile.txt");
        assert_eq!(
            position(file, Some(7), true),
            "/owned checkout/--ü & $(owned);\nfile.txt:7:1"
        );
        assert_eq!(position(file, None, false), file.to_str().unwrap());
        let args = configured_argv("nvim --literal \"two words\"").unwrap();
        assert_eq!(args, ["nvim", "--literal", "two words"]);
    }
    #[cfg(windows)]
    #[test]
    fn native_system_association_receives_literal_utf16_and_truthful_result() {
        let file = Path::new("C:\\owned checkout\\--ü & $(owned);file.txt");
        let check = |value: &[u16]| {
            assert_eq!(value.last(), Some(&0));
            assert_eq!(
                String::from_utf16(&value[..value.len() - 1]).unwrap(),
                file.to_str().unwrap()
            );
            64
        };
        assert!(system_open_with(file, check).is_ok());
        for failure in [0, 2, 5, 31, 32] {
            assert!(system_open_with(file, |_| failure).is_err());
        }
        assert!(system_open_with(Path::new("C:\\bad\0file"), |_| panic!(
            "NUL path reached native association"
        ))
        .is_err());
    }
}
