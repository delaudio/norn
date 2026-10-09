//! Private stdio backend host for the OpenTUI workspace. Speaks the versioned
//! protocol on standard input and output; diagnostics go to standard error.

fn main() {
    if let Err(code) = norn_lib::backend::run() {
        std::process::exit(code);
    }
}
