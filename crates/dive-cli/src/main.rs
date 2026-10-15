//! The native dive command-line entrypoint.
#![forbid(unsafe_code)]

fn main() -> std::process::ExitCode {
    dive_cli::main()
}
