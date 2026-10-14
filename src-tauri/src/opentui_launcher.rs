//! Launch only the client shipped beside this command, never one from the cwd.
use std::path::Path;
use std::process::Command;

fn sibling(directory: &Path, name: &str) -> std::path::PathBuf {
    directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

pub fn run() -> Result<i32, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let directory = executable
        .parent()
        .ok_or("Cannot locate Norn installation.")?;
    let client = sibling(directory, "norn-ui");
    let backend = sibling(directory, "norn-backend");
    if !client.is_file() || !backend.is_file() {
        return Err("OpenTUI client/backend are missing beside norn. Run `pnpm run install:local` from a source checkout, or reinstall the complete command package. For development use `pnpm run dev`.".into());
    }
    let mut command = Command::new(client);
    command.args(std::env::args_os().skip(1));
    command.env("NORN_BACKEND_BIN", backend);
    // exec preserves terminal ownership, signals and the UI's exit status.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec().to_string())
    }
    #[cfg(not(unix))]
    {
        command
            .status()
            .map(|s| s.code().unwrap_or(1))
            .map_err(|e| e.to_string())
    }
}
