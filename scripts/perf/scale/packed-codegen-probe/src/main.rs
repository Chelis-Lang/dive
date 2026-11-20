#![forbid(unsafe_code)]

use std::hint::black_box;
use std::time::Instant;

use wasmtime::{Engine, Instance, Module, Store};

const WORDS: usize = 97;
const ROW_BYTES: usize = 8 * WORDS;
const ROWS: usize = 64;
const REQ: usize = 3 + 3 + 2;
const CHANNEL: usize = 16 + (2 * 3 + 1) * 9;
const RELEASE: u64 = 3;

fn receive_release(rows: &mut [u8]) -> i32 {
    let mut emitted = 0;
    for row in rows.chunks_exact_mut(ROW_BYTES) {
        let read = |word| u64::from_le_bytes(row[word * 8..word * 8 + 8].try_into().unwrap());
        let length = read(CHANNEL);
        if length > 3 {
            return -1;
        }
        if length != 0 && read(CHANNEL + 1) == RELEASE {
            row[REQ * 8..REQ * 8 + 8].fill(0);
            row[CHANNEL * 8..CHANNEL * 8 + 8].copy_from_slice(&(length - 1).to_le_bytes());
            row.copy_within((CHANNEL + 3) * 8..(CHANNEL + 7) * 8, (CHANNEL + 1) * 8);
            row[(CHANNEL + 5) * 8..(CHANNEL + 7) * 8].fill(0);
            emitted += 1;
        }
    }
    emitted
}

fn wat() -> String {
    let ch = CHANNEL * 8;
    let req = REQ * 8;
    let mut shift = String::new();
    for word in 1..=4 {
        let to = ch + word * 8;
        let from = to + 16;
        shift.push_str(&format!(
            "(i64.store offset={to} (local.get $at) (i64.load offset={from} (local.get $at)))"
        ));
    }
    let kind = ch + 8;
    let last_kind = ch + 40;
    let last_clock = ch + 48;
    format!(
        r#"(module
          (memory (export "rows") 1 1)
          (func (export "expand") (param $end i32) (result i32)
            (local $at i32) (local $len i64) (local $emitted i32)
            (block $done (loop $row
              (br_if $done (i32.ge_u (local.get $at) (local.get $end)))
              (local.set $len (i64.load offset={ch} (local.get $at)))
              (if (i64.gt_u (local.get $len) (i64.const 3)) (then (return (i32.const -1))))
              (if (i32.and (i64.ne (local.get $len) (i64.const 0))
                           (i64.eq (i64.load offset={kind} (local.get $at)) (i64.const {RELEASE})))
                (then
                  (i64.store offset={req} (local.get $at) (i64.const 0))
                  (i64.store offset={ch} (local.get $at) (i64.sub (local.get $len) (i64.const 1)))
                  {shift}
                  (i64.store offset={last_kind} (local.get $at) (i64.const 0))
                  (i64.store offset={last_clock} (local.get $at) (i64.const 0))
                  (local.set $emitted (i32.add (local.get $emitted) (i32.const 1)))))
              (local.set $at (i32.add (local.get $at) (i32.const {ROW_BYTES})))
              (br $row)))
            (local.get $emitted)))"#
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repeats: usize = std::env::args().nth(1).map_or(Ok(100_000), |s| s.parse())?;
    let mut rows = vec![0_u8; ROW_BYTES * ROWS];
    for (i, row) in rows.chunks_exact_mut(ROW_BYTES).enumerate() {
        let length = (i % 4) as u64;
        row[CHANNEL * 8..CHANNEL * 8 + 8].copy_from_slice(&length.to_le_bytes());
        for j in 0..length as usize {
            let kind: u64 = if (i + j) % 4 == 1 { RELEASE } else { 1 };
            let offset = (CHANNEL + 1 + 2 * j) * 8;
            row[offset..offset + 8].copy_from_slice(&kind.to_le_bytes());
        }
        row[REQ * 8] = 8;
    }
    let compile = Instant::now();
    let engine = Engine::default();
    let module = Module::new(&engine, wat())?;
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[])?;
    let memory = instance
        .get_memory(&mut store, "rows")
        .ok_or("rows export")?;
    let evaluate = instance.get_typed_func::<i32, i32>(&mut store, "expand")?;
    println!(
        "compile_and_instantiate_ns={}",
        compile.elapsed().as_nanos()
    );
    let mut native = rows.clone();
    let mut result = rows.clone();
    let end = i32::try_from(rows.len())?;
    let expected = receive_release(&mut native);
    memory.write(&mut store, 0, &rows)?;
    assert_eq!(expected, evaluate.call(&mut store, end)?);
    memory.read(&store, 0, &mut result)?;
    assert_eq!(result, native);
    for rep in 0..6 {
        if rep % 2 == 0 {
            let timer = Instant::now();
            for _ in 0..repeats {
                native.copy_from_slice(&rows);
                black_box(receive_release(black_box(&mut native)));
                black_box(&native);
            }
            println!("native_ns={}", timer.elapsed().as_nanos());
        } else {
            let mut transfer = 0;
            let mut execute = 0;
            for _ in 0..repeats {
                let begin = Instant::now();
                memory.write(&mut store, 0, &rows)?;
                let ready = Instant::now();
                black_box(evaluate.call(&mut store, end)?);
                let done = Instant::now();
                memory.read(&store, 0, &mut result)?;
                black_box(&result);
                transfer += (ready - begin + done.elapsed()).as_nanos();
                execute += (done - ready).as_nanos();
            }
            println!(
                "wasm_ns={} wasm_call_ns={} transfer_ns={}",
                execute + transfer,
                execute,
                transfer
            );
        }
    }
    Ok(())
}
