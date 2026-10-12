//! Supplied entry point: `class14 [ld options] inputs...`.
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match class14::driver::run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("class14: {e:?}");
            ExitCode::FAILURE
        }
    }
}
