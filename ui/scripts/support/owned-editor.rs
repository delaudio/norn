//! Real editor surrogate, run only in an owned PTY/ConPTY.
use std::io::{self, IsTerminal, Write};
#[cfg(unix)]
unsafe extern "C" {
    fn getppid() -> i32;
}
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(kind: u32) -> *mut std::ffi::c_void;
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: *mut std::ffi::c_void, mode: u32) -> i32;
}
fn main() {
    let root = std::path::PathBuf::from(
        std::env::var_os("NORN_OWNED_EDITOR_DIR").expect("owned editor directory"),
    );
    let graphical = std::env::current_exe().unwrap().file_stem().unwrap() == "zed";
    assert!(
        graphical
            || (io::stdin().is_terminal()
                && io::stdout().is_terminal()
                && io::stderr().is_terminal()),
        "Editor received protocol pipes instead of the real terminal"
    );
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::fs::write(root.join("argv.txt"), args.join("\n")).unwrap();
    std::fs::write(root.join("editor.pid"), std::process::id().to_string()).unwrap();
    #[cfg(unix)]
    std::fs::write(root.join("wrapper.pid"), unsafe { getppid() }.to_string()).unwrap();
    if graphical {
        assert!(!io::stdin().is_terminal() && !io::stdout().is_terminal());
        std::process::exit(
            std::env::var("NORN_OWNED_EDITOR_FAILURE")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0),
        );
    }
    // Leave a real mode change behind. The production renderer, not this
    // surrogate, must restore its terminal after success/error/interrupt/death.
    #[cfg(unix)]
    {
        let capture = || {
            std::process::Command::new("stty")
                .arg("-g")
                .stdin(std::process::Stdio::inherit())
                .output()
                .unwrap()
        };
        let before = capture();
        assert!(before.status.success());
        assert!(std::process::Command::new("stty")
            .args(["-echo", "-icanon", "min", "1", "time", "0"])
            .status()
            .unwrap()
            .success());
        let after = capture();
        assert!(after.status.success());
        assert_ne!(before.stdout, after.stdout);
        std::fs::write(
            root.join("changed-modes.txt"),
            [before.stdout, after.stdout].concat(),
        )
        .unwrap();
    }
    #[cfg(windows)]
    unsafe {
        let handle = GetStdHandle(-10i32 as u32);
        let mut before = 0;
        assert_ne!(GetConsoleMode(handle, &mut before), 0);
        let changed = before ^ 4;
        assert_ne!(SetConsoleMode(handle, changed), 0);
        let mut after = 0;
        assert_ne!(GetConsoleMode(handle, &mut after), 0);
        assert_eq!(after, changed);
        std::fs::write(root.join("changed-modes.txt"), format!("{before} {after}")).unwrap();
    }
    println!("OWNED_EDITOR_READY");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    std::fs::write(root.join("input.txt"), &input).unwrap();
    if input.trim() == "fail" {
        std::process::exit(7);
    }
    assert_eq!(input.trim(), "finish", "Unexpected editor input");
    println!("OWNED_EDITOR_DONE");
}
