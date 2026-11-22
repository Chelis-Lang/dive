//! Enumerate concrete finite predicates and certify their source obligations.

mod engine;

fn main() -> Result<(), String> {
    engine::entry(&std::env::args_os().skip(1).collect::<Vec<_>>())
}
