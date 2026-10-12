//! Real terminal handles are separate from backend protocol pipes.
use super::*;
use std::{
    fs::File,
    io,
    process::{Command, Stdio},
};

pub(super) struct Terminal {
    input: File,
    output: File,
    #[cfg(unix)]
    previous_group: i32,
    #[cfg(windows)]
    attached: bool,
}

#[cfg(unix)]
fn foreground(fd: i32, group: i32) -> io::Result<()> {
    // Only this thread blocks SIGTTOU around a background-process tty ioctl.
    // The original mask is restored; no global signal disposition is changed.
    unsafe {
        let mut block = std::mem::zeroed();
        let mut previous = std::mem::zeroed();
        libc::sigemptyset(&mut block);
        libc::sigaddset(&mut block, libc::SIGTTOU);
        let result = libc::pthread_sigmask(libc::SIG_BLOCK, &block, &mut previous);
        if result != 0 {
            return Err(io::Error::from_raw_os_error(result));
        }
        let changed = libc::tcsetpgrp(fd, group);
        let failure = if changed != 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        libc::pthread_sigmask(libc::SIG_SETMASK, &previous, std::ptr::null_mut());
        failure.map_or(Ok(()), Err)
    }
}

impl Terminal {
    pub(super) fn open() -> Result<Self, String> {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let input = File::options()
                .read(true)
                .write(true)
                .open("/dev/tty")
                .map_err(|_| "An attached terminal is required for this editor.".to_owned())?;
            let output = input.try_clone().map_err(|_| invalid())?;
            let previous_group = unsafe { libc::tcgetpgrp(input.as_raw_fd()) };
            if previous_group <= 0 {
                return Err("An attached foreground terminal is required.".into());
            }
            Ok(Self {
                input,
                output,
                previous_group,
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::System::Console::{
                AttachConsole, GetConsoleMode, SetConsoleCtrlHandler, ATTACH_PARENT_PROCESS,
            };
            let mut attached = false;
            if File::options()
                .read(true)
                .write(true)
                .open("CONIN$")
                .is_err()
            {
                if unsafe { AttachConsole(ATTACH_PARENT_PROCESS) } == 0 {
                    return Err("An attached console is required for this editor.".into());
                }
                attached = true;
            }
            let handles = (|| {
                let input = File::options()
                    .read(true)
                    .write(true)
                    .open("CONIN$")
                    .map_err(|_| invalid())?;
                let output = File::options()
                    .read(true)
                    .write(true)
                    .open("CONOUT$")
                    .map_err(|_| invalid())?;
                let mut mode = 0;
                if unsafe { GetConsoleMode(input.as_raw_handle().cast(), &mut mode) } == 0
                    || unsafe { GetConsoleMode(output.as_raw_handle().cast(), &mut mode) } == 0
                {
                    return Err(invalid());
                }
                // The private child restores its own console controls. The
                // protocol host never handles the editor's broadcast Ctrl-C.
                if unsafe { SetConsoleCtrlHandler(None, 1) } == 0 {
                    return Err(invalid());
                }
                Ok(Self {
                    input,
                    output,
                    attached,
                })
            })();
            if handles.is_err() && attached {
                unsafe {
                    windows_sys::Win32::System::Console::FreeConsole();
                }
            }
            handles
        }
    }
    pub(super) fn configure(&self, command: &mut Command) -> Result<(), String> {
        command
            .stdin(Stdio::from(self.input.try_clone().map_err(|_| invalid())?))
            .stdout(Stdio::from(self.output.try_clone().map_err(|_| invalid())?))
            .stderr(Stdio::from(self.output.try_clone().map_err(|_| invalid())?));
        crate::local_review::configure_owned_command(command);
        #[cfg(unix)]
        {
            use std::os::{fd::AsRawFd, unix::process::CommandExt};
            let fd = self.input.as_raw_fd();
            unsafe {
                command.pre_exec(move || foreground(fd, libc::getpid()));
            }
        }
        Ok(())
    }
    pub(super) fn previous_group(&self) -> Option<i32> {
        #[cfg(unix)]
        {
            Some(self.previous_group)
        }
        #[cfg(windows)]
        {
            None
        }
    }
    pub(super) fn restore(&self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let _ = foreground(self.input.as_raw_fd(), self.previous_group);
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::System::Console::SetConsoleCtrlHandler(None, 0);
            if self.attached {
                windows_sys::Win32::System::Console::FreeConsole();
            }
        }
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        self.restore();
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ChildParams {
    pub invocation: integration::Invocation,
    pub parent: u32,
    pub previous_group: Option<i32>,
}

#[cfg(unix)]
extern "C" fn child_interrupt(_: i32) {}
#[cfg(windows)]
unsafe extern "system" fn child_interrupt(event: u32) -> i32 {
    i32::from(event <= 1)
}

/// Private wrapper: exact Rust-authorized argv, real console and parent-death
/// cleanup. It is not a stdio protocol host and cannot contaminate its frames.
pub(crate) fn run_child(value: &str) -> Result<i32, String> {
    if value.len() > 32768 {
        return Err(invalid());
    }
    let params: ChildParams = serde_json::from_str(value).map_err(|_| invalid())?;
    if params.parent == 0
        || params.invocation.args.len() > 34
        || !params.invocation.program.is_absolute()
    {
        return Err(invalid());
    }
    #[cfg(unix)]
    let mut terminal = Terminal::open()?;
    #[cfg(windows)]
    let terminal = Terminal::open()?;
    if !parent_is_backend(params.parent) {
        #[cfg(unix)]
        if let Some(group) = params.previous_group.filter(|g| *g > 0) {
            terminal.previous_group = group;
        }
        return Err("The editor's workspace process could not be verified.".into());
    }
    #[cfg(unix)]
    {
        let previous = params
            .previous_group
            .filter(|group| *group > 0)
            .ok_or_else(invalid)?;
        terminal.previous_group = previous;
        // A caught handler resets on editor exec; ignored dispositions would
        // instead incorrectly disable the editor's own Ctrl-C handling.
        unsafe {
            libc::signal(
                libc::SIGINT,
                child_interrupt as *const () as libc::sighandler_t,
            );
        }
        if unsafe { libc::getppid() } as u32 != params.parent {
            return Err("The editor's workspace process exited.".into());
        }
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
        SetConsoleCtrlHandler(None, 0);
        SetConsoleCtrlHandler(Some(child_interrupt), 1);
    }
    let mut child = Command::new(&params.invocation.program)
        .args(&params.invocation.args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|_| "The terminal editor could not start.".to_owned())?;
    loop {
        #[cfg(unix)]
        if unsafe { libc::getppid() } as u32 != params.parent {
            terminal.restore();
            // The wrapper and editor share the owned foreground group. Parent
            // death cannot leave an editor alive or hold the UI's foreground.
            unsafe {
                libc::kill(-libc::getpgrp(), libc::SIGKILL);
            }
            return Ok(125);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                #[cfg(unix)]
                {
                    use std::os::unix::process::ExitStatusExt;
                    return Ok(status.code().unwrap_or(128 + status.signal().unwrap_or(1)));
                }
                #[cfg(windows)]
                return Ok(status.code().unwrap_or(1));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("The terminal editor exited unexpectedly.".into());
            }
        }
    }
}

fn parent_is_backend(expected: u32) -> bool {
    let current = std::env::current_exe()
        .ok()
        .and_then(|p| p.canonicalize().ok());
    #[cfg(unix)]
    if unsafe { libc::getppid() } as u32 != expected {
        return false;
    }
    #[cfg(target_os = "macos")]
    let parent = {
        let mut bytes = vec![0u8; 4096];
        let length = unsafe {
            libc::proc_pidpath(
                expected as i32,
                bytes.as_mut_ptr().cast(),
                bytes.len() as u32,
            )
        };
        if length <= 0 {
            return false;
        }
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        std::str::from_utf8(&bytes[..end])
            .ok()
            .and_then(|s| PathBuf::from(s).canonicalize().ok())
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let parent = std::fs::read_link(format!("/proc/{expected}/exe"))
        .ok()
        .and_then(|p| p.canonicalize().ok());
    #[cfg(windows)]
    let parent = {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
            System::{
                Diagnostics::ToolHelp::{
                    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                    TH32CS_SNAPPROCESS,
                },
                Threading::{
                    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
                },
            },
        };
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE {
            return false;
        }
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut present = unsafe { Process32FirstW(snapshot, &mut entry) } != 0;
        let mut verified = false;
        while present {
            if entry.th32ProcessID == std::process::id() {
                verified = entry.th32ParentProcessID == expected;
                break;
            }
            present = unsafe { Process32NextW(snapshot, &mut entry) } != 0;
        }
        unsafe {
            CloseHandle(snapshot);
        }
        if !verified {
            return false;
        }
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, expected) };
        if handle.is_null() {
            return false;
        }
        let mut buffer = vec![0u16; 32768];
        let mut length = buffer.len() as u32;
        let success =
            unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) } != 0;
        unsafe {
            CloseHandle(handle);
        }
        if !success {
            return false;
        }
        String::from_utf16(&buffer[..length as usize])
            .ok()
            .and_then(|s| PathBuf::from(s).canonicalize().ok())
    };
    current.is_some() && current == parent
}
