//! Native, std-only analyzer peer; no Git/provider/credential access.
use std::{io::Write, time::Duration};
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("clean") => println!("ok"),
        Some("exit3") => std::process::exit(3),
        Some("exit1") => std::process::exit(1),
        Some("timeout") => {
            let binary = std::env::current_exe().unwrap();
            std::fs::write(
                binary.parent().unwrap().join("child.pid"),
                std::process::id().to_string(),
            )
            .unwrap();
            std::thread::sleep(Duration::from_secs(5));
            println!("late");
        }
        Some("truncated") => std::io::stdout().write_all(&vec![b'a'; 400000]).unwrap(),
        _ => panic!("unregistered owned mode"),
    }
}
