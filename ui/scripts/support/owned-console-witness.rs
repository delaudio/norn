//! Owns a new ConPTY console, checks native modes around the production UI.
use std::{
    io::{self, IsTerminal},
    process::{Command, Stdio},
};
#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(kind: u32) -> *mut std::ffi::c_void;
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
}
#[cfg(windows)]
fn modes() -> (u32, u32) {
    unsafe {
        let mut input = 0;
        let mut output = 0;
        assert_ne!(GetConsoleMode(GetStdHandle(-10i32 as u32), &mut input), 0);
        assert_ne!(GetConsoleMode(GetStdHandle(-11i32 as u32), &mut output), 0);
        (input, output)
    }
}
#[cfg(windows)]
unsafe extern "system" fn witness_interrupt(event: u32) -> i32 {
    i32::from(event <= 1)
}
#[cfg(windows)]
fn main() {
    assert!(io::stdin().is_terminal() && io::stdout().is_terminal());
    let root = std::path::PathBuf::from(std::env::var_os("NORN_OWNED_EDITOR_DIR").unwrap());
    let before = modes();
    // Protect only the owning test supervisor. A caught handler is not inherited
    // by Bun or the editor, so their real interrupt handling stays testable.
    unsafe {
        assert_ne!(SetConsoleCtrlHandler(Some(witness_interrupt), 1), 0);
    }
    let mut args = std::env::args_os().skip(1);
    let mut ui = Command::new(args.next().unwrap())
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    std::fs::write(root.join("ui.pid"), ui.id().to_string()).unwrap();
    let status = ui.wait().unwrap();
    let after = modes();
    std::fs::write(
        root.join("console-modes.txt"),
        format!("{} {}\n{} {}", before.0, before.1, after.0, after.1),
    )
    .unwrap();
    assert_eq!(
        before, after,
        "Production UI did not restore the native console modes"
    );
    std::process::exit(status.code().unwrap_or(125));
}
#[cfg(not(windows))]
fn main() {
    panic!("This owned witness requires Windows ConPTY");
}
