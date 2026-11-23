//! Stream concrete exact Core graph rows and their source-bound action arguments.

use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mut read = || {
        args.next().map(PathBuf::from).ok_or(
        "expected MODULE CFG NEW_OUTPUT LIBRARY SECONDS MAX_STATES MAX_GENERATED WORKERS [STATE_FIELD...]"
            .to_owned())
    };
    let module = read()?;
    let config = read()?;
    let output = read()?;
    let library_path = read()?;
    let mut number = || {
        read()?
            .to_str()
            .ok_or("expected limit")?
            .parse::<u64>()
            .map_err(|_| "expected a positive integer limit".to_owned())
    };
    let seconds = number()?;
    let max_states = number()?;
    let max_generated = number()?;
    let workers =
        usize::try_from(number()?).map_err(|_| "worker count exceeds platform capacity")?;
    if seconds == 0 || max_states == 0 || max_generated == 0 || workers == 0 {
        return Err("graph limits and worker count must be positive".into());
    }
    let fields = args
        .map(|name| {
            name.into_string()
                .map_err(|_| "invalid field name".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = dive_api::labeled_graph::export(
        &module,
        &config,
        &output,
        &dive_api::labeled_graph::Request {
            library_paths: vec![library_path],
            fields,
            time_limit: Duration::from_secs(seconds),
            max_states,
            max_generated,
            workers,
        },
    )?;
    println!("{result}");
    Ok(())
}
