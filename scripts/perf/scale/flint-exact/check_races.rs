//! Concurrent growth must assign each mathematical row exactly one stable code.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier};
use tlc_rs::state_codes::state;
use tlc_rs::value::{Func, Value};

fn check() -> Result<(), String> {
    let wait = Arc::new(Barrier::new(16));
    let tables = std::thread::scope(|scope| {
        let mut threads = Vec::new();
        for worker in 0..16 {
            let wait = Arc::clone(&wait);
            threads.push(scope.spawn(move || -> Result<BTreeMap<_, _>, String> {
                wait.wait();
                let mut table = BTreeMap::new();
                for step in 0..4096 {
                    let index = (step * 31 + worker * 71) % 4096;
                    let val = Value::Int(index % 64);
                    let seq = if (worker + step) % 2 == 0 {
                        Value::Seq(Arc::from([val]))
                    } else {
                        Value::Func(Arc::new(Func {
                            keys: Arc::from([Value::Int(1)]),
                            vals: Box::from([val]),
                        }))
                    };
                    let row = [Value::Int(index / 64), seq];
                    table.insert(index, state(&row)?);
                }
                Ok(table)
            }));
        }
        threads
            .into_iter()
            .map(|thread| thread.join().map_err(|_| "worker panicked")?)
            .collect::<Result<Vec<_>, String>>()
    })?;
    let first = &tables[0];
    assert_eq!(first.len(), 4096);
    assert_eq!(first.values().copied().collect::<BTreeSet<_>>().len(), 4096);
    assert!(tables.iter().all(|table| table == first));
    Ok(())
}

fn main() -> Result<(), String> {
    check()
}
