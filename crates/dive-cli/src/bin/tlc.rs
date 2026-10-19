//! TLC checking argument entrypoint.
#![forbid(unsafe_code)]

/// mimalloc replaces the system allocator for faster state-space exploration (ADR 0102).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> std::process::ExitCode {
    dive_cli::main()
}
