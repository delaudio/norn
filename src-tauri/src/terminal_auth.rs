use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, IsTerminal, Read, Write};
#[cfg(unix)]
use std::os::fd::AsRawFd;

use serde::Serialize;

use crate::credentials::{self, CredentialProvider, CredentialStatus};

const MAX_SECRET_BYTES: u64 = 32_769;

trait CredentialBackend {
    fn status(&self, provider: CredentialProvider) -> CredentialStatus;
    fn store(
        &mut self,
        provider: CredentialProvider,
        username: Option<&str>,
        token: &str,
    ) -> Result<(), String>;
    fn clear(&mut self, provider: CredentialProvider) -> Result<(), String>;
}

struct OsCredentialBackend;

impl CredentialBackend for OsCredentialBackend {
    fn status(&self, provider: CredentialProvider) -> CredentialStatus {
        credentials::credential_status(provider)
    }

    fn store(
        &mut self,
        provider: CredentialProvider,
        username: Option<&str>,
        token: &str,
    ) -> Result<(), String> {
        credentials::store_provider_credential(provider, username, token)
    }

    fn clear(&mut self, provider: CredentialProvider) -> Result<(), String> {
        credentials::clear_provider_credential(provider)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthOutput {
    schema_version: &'static str,
    action: &'static str,
    credentials: Vec<CredentialStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthFormat {
    Human,
    Json,
}

pub fn run(args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        let _ = writeln!(stdout, "{}", usage());
        return 0;
    }
    let mut backend = OsCredentialBackend;
    run_with_backend(args, stdout, stderr, &mut backend, None)
}

fn run_with_backend(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    backend: &mut dyn CredentialBackend,
    scripted_input: Option<&str>,
) -> i32 {
    let Some(action) = args.first().map(String::as_str) else {
        let _ = writeln!(stderr, "Missing auth action.\n\n{}", usage());
        return 2;
    };
    match action {
        "status" => run_status(&args[1..], stdout, stderr, backend),
        "login" => run_login(&args[1..], stdout, stderr, backend, scripted_input),
        "logout" => run_logout(&args[1..], stdout, stderr, backend),
        unknown => {
            let _ = writeln!(stderr, "Unknown auth action `{unknown}`.\n\n{}", usage());
            2
        }
    }
}

fn parse_provider(value: &str) -> Result<CredentialProvider, String> {
    match value {
        "github" => Ok(CredentialProvider::Github),
        "bitbucket" => Ok(CredentialProvider::Bitbucket),
        _ => Err("Provider must be `github` or `bitbucket`.".to_string()),
    }
}

fn parse_format(args: &[String]) -> Result<AuthFormat, String> {
    let mut format = AuthFormat::Human;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => format = AuthFormat::Json,
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("human" | "text") => AuthFormat::Human,
                    Some("json") => AuthFormat::Json,
                    Some(_) => return Err("`--format` must be `human` or `json`.".to_string()),
                    None => return Err("`--format` requires a value.".to_string()),
                };
            }
            unknown => return Err(format!("Unknown auth option `{unknown}`.")),
        }
        index += 1;
    }
    Ok(format)
}

fn run_status(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    backend: &dyn CredentialBackend,
) -> i32 {
    let format = match parse_format(args) {
        Ok(format) => format,
        Err(error) => {
            let _ = writeln!(stderr, "{error}\n\n{}", usage());
            return 2;
        }
    };
    let statuses = [CredentialProvider::Github, CredentialProvider::Bitbucket]
        .into_iter()
        .map(|provider| backend.status(provider))
        .collect::<Vec<_>>();
    match format {
        AuthFormat::Human => {
            let _ = writeln!(stdout, "Norn provider credentials:");
            for status in statuses {
                let state = if status.available {
                    "configured"
                } else {
                    "missing"
                };
                let source = serde_json::to_value(status.source)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| "none".to_string());
                let _ = writeln!(stdout, "- {}: {state} ({source})", status.provider.label());
            }
        }
        AuthFormat::Json => {
            let output = AuthOutput {
                schema_version: "norn.auth.v1",
                action: "status",
                credentials: statuses,
            };
            match serde_json::to_string_pretty(&output) {
                Ok(json) => {
                    let _ = writeln!(stdout, "{json}");
                }
                Err(error) => {
                    let _ = writeln!(stderr, "Failed to serialize credential status: {error}");
                    return 7;
                }
            }
        }
    }
    0
}

fn run_login(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    backend: &mut dyn CredentialBackend,
    scripted_input: Option<&str>,
) -> i32 {
    let Some(provider_value) = args.first() else {
        let _ = writeln!(stderr, "Login requires a provider.\n\n{}", usage());
        return 2;
    };
    let provider = match parse_provider(provider_value) {
        Ok(provider) => provider,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 2;
        }
    };
    let mut username = None::<String>;
    let mut token_stdin = false;
    let mut format_args = Vec::new();
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--username" => {
                index += 1;
                username = args.get(index).cloned();
                if username.is_none() {
                    let _ = writeln!(stderr, "`--username` requires a value.");
                    return 2;
                }
            }
            "--token-stdin" => token_stdin = true,
            "--json" => format_args.push("--json".to_string()),
            "--format" => {
                format_args.push("--format".to_string());
                index += 1;
                let Some(value) = args.get(index) else {
                    let _ = writeln!(stderr, "`--format` requires a value.");
                    return 2;
                };
                format_args.push(value.clone());
            }
            "--token" => {
                let _ = writeln!(stderr, "Tokens are not accepted as command arguments. Use hidden input or `--token-stdin`.");
                return 2;
            }
            unknown => {
                let _ = writeln!(stderr, "Unknown login option `{unknown}`.");
                return 2;
            }
        }
        index += 1;
    }
    let format = match parse_format(&format_args) {
        Ok(format) => format,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 2;
        }
    };

    if provider == CredentialProvider::Bitbucket && username.is_none() {
        username = match scripted_input {
            Some(input) => input.lines().next().map(str::to_string),
            None => match read_visible_line("Bitbucket username: ") {
                Ok(value) => Some(value),
                Err(error) => {
                    let _ = writeln!(stderr, "{error}");
                    return 2;
                }
            },
        };
    }
    let token = if let Some(input) = scripted_input {
        if provider == CredentialProvider::Bitbucket && args.iter().all(|arg| arg != "--username") {
            input.lines().nth(1).unwrap_or_default().to_string()
        } else {
            input.lines().next().unwrap_or_default().to_string()
        }
    } else if token_stdin {
        match read_bounded(io::stdin().lock()) {
            Ok(value) => value,
            Err(error) => {
                let _ = writeln!(stderr, "{error}");
                return 2;
            }
        }
    } else {
        if !io::stdin().is_terminal() {
            let _ = writeln!(stderr, "Interactive secret input requires a terminal. Pipe the token with `--token-stdin`.");
            return 2;
        }
        match read_hidden_line(&format!("{} token: ", provider.label())) {
            Ok(value) => value,
            Err(error) => {
                let _ = writeln!(stderr, "{error}");
                return 2;
            }
        }
    };
    if let Err(error) = backend.store(provider, username.as_deref(), token.trim()) {
        let _ = writeln!(stderr, "{error}");
        return 7;
    }
    write_mutation_output(format, "login", provider, backend, stdout, stderr)
}

fn run_logout(
    args: &[String],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    backend: &mut dyn CredentialBackend,
) -> i32 {
    let Some(provider_value) = args.first() else {
        let _ = writeln!(stderr, "Logout requires a provider.\n\n{}", usage());
        return 2;
    };
    let provider = match parse_provider(provider_value) {
        Ok(provider) => provider,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 2;
        }
    };
    let format = match parse_format(&args[1..]) {
        Ok(format) => format,
        Err(error) => {
            let _ = writeln!(stderr, "{error}");
            return 2;
        }
    };
    if let Err(error) = backend.clear(provider) {
        let _ = writeln!(stderr, "{error}");
        return 7;
    }
    write_mutation_output(format, "logout", provider, backend, stdout, stderr)
}

fn write_mutation_output(
    format: AuthFormat,
    action: &'static str,
    provider: CredentialProvider,
    backend: &dyn CredentialBackend,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let status = backend.status(provider);
    match format {
        AuthFormat::Human => {
            let verb = if action == "login" {
                "Stored"
            } else {
                "Removed"
            };
            let _ = writeln!(
                stdout,
                "{verb} {} credential in the OS keychain.",
                provider.label()
            );
        }
        AuthFormat::Json => {
            let output = AuthOutput {
                schema_version: "norn.auth.v1",
                action,
                credentials: vec![status],
            };
            match serde_json::to_string_pretty(&output) {
                Ok(json) => {
                    let _ = writeln!(stdout, "{json}");
                }
                Err(error) => {
                    let _ = writeln!(stderr, "Failed to serialize auth result: {error}");
                    return 7;
                }
            }
        }
    }
    0
}

fn read_bounded(mut reader: impl Read) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(MAX_SECRET_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|_| "Failed to read token input.".to_string())?;
    if bytes.len() as u64 >= MAX_SECRET_BYTES {
        return Err("Token exceeds the supported length.".to_string());
    }
    String::from_utf8(bytes).map_err(|_| "Token input must be valid UTF-8.".to_string())
}

fn read_visible_line(prompt: &str) -> Result<String, String> {
    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|_| "Interactive input requires a terminal.".to_string())?;
    tty.write_all(prompt.as_bytes())
        .and_then(|_| tty.flush())
        .map_err(|_| "Failed to write terminal prompt.".to_string())?;
    let mut value = String::new();
    BufReader::new(&tty)
        .read_line(&mut value)
        .map_err(|_| "Failed to read terminal input.".to_string())?;
    Ok(value.trim().to_string())
}

#[cfg(unix)]
fn read_hidden_line(prompt: &str) -> Result<String, String> {
    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|_| "Interactive secret input requires a terminal.".to_string())?;
    tty.write_all(prompt.as_bytes())
        .and_then(|_| tty.flush())
        .map_err(|_| "Failed to write terminal prompt.".to_string())?;
    let fd = tty.as_raw_fd();
    let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, original.as_mut_ptr()) } != 0 {
        return Err("Failed to configure hidden terminal input.".to_string());
    }
    let original = unsafe { original.assume_init() };
    let mut hidden = original;
    hidden.c_lflag &= !libc::ECHO;
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &hidden) } != 0 {
        return Err("Failed to configure hidden terminal input.".to_string());
    }
    let mut value = String::new();
    let result = BufReader::new(&tty)
        .read_line(&mut value)
        .map_err(|_| "Failed to read hidden terminal input.".to_string());
    let _ = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &original) };
    let _ = writeln!(tty);
    result.map(|_| value.trim().to_string())
}

#[cfg(not(unix))]
fn read_hidden_line(_prompt: &str) -> Result<String, String> {
    Err(
        "Hidden interactive input is unavailable on this platform; use `--token-stdin`."
            .to_string(),
    )
}

pub fn usage() -> &'static str {
    "Usage:
  norn auth status [--format human|json] [--json]
  norn auth login github [--token-stdin] [--format human|json] [--json]
  norn auth login bitbucket [--username <name>] [--token-stdin]
                             [--format human|json] [--json]
  norn auth logout github|bitbucket [--format human|json] [--json]

Interactive login reads tokens without terminal echo. For scripts, pass the
token through standard input with `--token-stdin`; tokens are never accepted
as command arguments. Credentials are stored only in the OS keychain."
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::CredentialSource;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeBackend {
        values: HashMap<&'static str, (Option<String>, String)>,
    }

    impl CredentialBackend for FakeBackend {
        fn status(&self, provider: CredentialProvider) -> CredentialStatus {
            let available = self.values.contains_key(provider.label());
            CredentialStatus {
                provider,
                available,
                source: if available {
                    CredentialSource::Keychain
                } else {
                    CredentialSource::None
                },
            }
        }

        fn store(
            &mut self,
            provider: CredentialProvider,
            username: Option<&str>,
            token: &str,
        ) -> Result<(), String> {
            if token.trim().is_empty() {
                return Err("Token cannot be empty.".to_string());
            }
            self.values.insert(
                provider.label(),
                (username.map(str::to_string), token.to_string()),
            );
            Ok(())
        }

        fn clear(&mut self, provider: CredentialProvider) -> Result<(), String> {
            self.values.remove(provider.label());
            Ok(())
        }
    }

    fn run_test(
        args: &[&str],
        input: Option<&str>,
        backend: &mut FakeBackend,
    ) -> (i32, String, String) {
        let args = args
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let code = run_with_backend(&args, &mut stdout, &mut stderr, backend, input);
        (
            code,
            String::from_utf8(stdout).expect("stdout"),
            String::from_utf8(stderr).expect("stderr"),
        )
    }

    #[test]
    fn status_json_is_redacted_and_path_free() {
        let mut backend = FakeBackend::default();
        backend
            .values
            .insert("github", (None, "SECRET_DO_NOT_LEAK".to_string()));
        let (code, stdout, stderr) = run_test(&["status", "--json"], None, &mut backend);
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.contains("norn.auth.v1"));
        assert!(stdout.contains("keychain"));
        assert!(!stdout.contains("SECRET_DO_NOT_LEAK"));
        assert!(!stdout.contains("/Users/"));
    }

    #[test]
    fn login_and_logout_mutate_only_selected_provider() {
        let mut backend = FakeBackend::default();
        let (code, stdout, stderr) = run_test(
            &["login", "github", "--token-stdin"],
            Some("secret-token\n"),
            &mut backend,
        );
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(stdout.contains("OS keychain"));
        assert_eq!(
            backend.values.get("github").map(|value| value.1.as_str()),
            Some("secret-token")
        );

        let (code, _, stderr) = run_test(&["logout", "github"], None, &mut backend);
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert!(!backend.values.contains_key("github"));
    }

    #[test]
    fn bitbucket_login_requires_username_and_never_accepts_token_argv() {
        let mut backend = FakeBackend::default();
        let (code, _, stderr) = run_test(
            &["login", "bitbucket", "--token", "secret"],
            None,
            &mut backend,
        );
        assert_eq!(code, 2);
        assert!(stderr.contains("not accepted as command arguments"));
        assert!(!stderr.contains("secret"));

        let (code, _, stderr) = run_test(
            &[
                "login",
                "bitbucket",
                "--username",
                "reviewer",
                "--token-stdin",
            ],
            Some("bb-token\n"),
            &mut backend,
        );
        assert_eq!(code, 0);
        assert!(stderr.is_empty());
        assert_eq!(
            backend
                .values
                .get("bitbucket")
                .and_then(|value| value.0.as_deref()),
            Some("reviewer")
        );
    }
}
