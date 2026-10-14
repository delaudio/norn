//! Std-only owned agent for actual inline-review/cancellation tests on every OS.
use std::{
    io::{Read, Write},
    time::Duration,
};
fn main() {
    let mut payload = String::new();
    std::io::stdin()
        .read_to_string(&mut payload)
        .expect("owned payload");
    if let Some(started) = std::env::var_os("NORN_FAKE_STARTED") {
        std::fs::write(started, "up").expect("owned start receipt");
        std::thread::sleep(Duration::from_secs(10));
        std::fs::write(
            std::env::var_os("NORN_FAKE_DONE").expect("owned done path"),
            "done",
        )
        .expect("owned completion receipt");
    }
    writeln!(
        std::io::stdout(),
        "{{\"result\":\"No review findings were produced.\",\"session_id\":\"fake-session\"}}"
    )
    .expect("owned result");
}
