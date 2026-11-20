#![forbid(unsafe_code)]
//! Compile a dumped Core rank program into a resident Wasm kernel and compare
//! it with a Rust kernel over the same bounded rank table.

use std::fmt::Write;
use std::hint::black_box;
use std::time::Instant;

use serde::Deserialize;
use serde_json::json;
use wasmtime::{Engine, Instance, Module, Store};

#[derive(Deserialize)]
struct Action {
    lane: usize,
    shift: u32,
    next: Vec<u8>,
}

#[derive(Deserialize)]
struct Program {
    width: u32,
    cells: Vec<u32>,
    allowed: Vec<u64>,
    transitions: Vec<Action>,
}

impl Program {
    fn bits(&self) -> u32 {
        self.width * self.cells.len() as u32
    }

    fn capacity(&self) -> usize {
        1 << self.bits()
    }

    fn mask(&self) -> u32 {
        (1 << self.width) - 1
    }

    fn root(&self) -> u32 {
        self.cells
            .iter()
            .enumerate()
            .map(|(i, cell)| cell << (i as u32 * self.width))
            .sum()
    }

    fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !(1..=6).contains(&self.width)
            || self.cells.is_empty()
            || self.cells.len() > 24 / self.width as usize
            || self.allowed.len() != self.cells.len()
            || self
                .cells
                .iter()
                .zip(&self.allowed)
                .any(|(&v, &valid)| v > self.mask() || valid & (1_u64 << v) == 0)
            || self.transitions.is_empty()
            || self.transitions.len() > 256
            || self.transitions.iter().any(|action| {
                action.lane >= self.cells.len()
                    || action.shift != action.lane as u32 * self.width
                    || action.next.len() <= self.mask() as usize
                    || action.next.len() > 64
            })
        {
            return Err("rank program shape".into());
        }
        Ok(())
    }
}

fn rust(program: &Program) -> Result<(i64, i32, i32), &'static str> {
    let mut seen = vec![0_u8; program.capacity().div_ceil(8)];
    let root = program.root();
    seen[root as usize / 8] |= 1 << (root % 8);
    let mut queue = Vec::with_capacity(program.capacity());
    queue.push(root);
    let mut read = 0;
    let mut generated = 1;
    let mut depth = 0;
    while read != queue.len() {
        let boundary = queue.len();
        depth += 1;
        while read < boundary {
            let state = queue[read];
            let before = generated;
            for action in &program.transitions {
                let mask = program.mask();
                let replacement = u32::from(action.next[((state >> action.shift) & mask) as usize]);
                if replacement == 255 {
                    continue;
                }
                if replacement > mask || program.allowed[action.lane] & (1_u64 << replacement) == 0
                {
                    return Err("rank program would leave its admitted shape or invariant");
                }
                generated += 1;
                let dest = (state & !(mask << action.shift)) | (replacement << action.shift);
                let bit = 1_u8 << (dest % 8);
                let byte = &mut seen[dest as usize / 8];
                if *byte & bit == 0 {
                    *byte |= bit;
                    queue.push(dest);
                }
            }
            if generated == before {
                return Err("rank program would deadlock");
            }
            read += 1;
        }
    }
    Ok((
        generated,
        i32::try_from(queue.len()).map_err(|_| "rank count")?,
        depth,
    ))
}

fn wasm(program: &Program) -> String {
    let seen_bytes = program.capacity().div_ceil(8);
    let queue = seen_bytes.next_multiple_of(4);
    let table = queue + program.capacity() * 4;
    let total = table + program.transitions.len() * 64;
    let pages = total.div_ceil(65_536);
    let root = program.root();
    let mut data = String::new();
    let mut actions = String::new();
    for (i, action) in program.transitions.iter().enumerate() {
        let address = table + i * 64;
        let shift = action.shift;
        let mask = program.mask();
        let keep = !(mask << shift);
        let valid = program.allowed[action.lane];
        write!(data, r#"(data (i32.const {address}) ""#).unwrap();
        for replacement in &action.next {
            write!(data, "\\{replacement:02x}").unwrap();
        }
        data.push_str("\")");
        write!(
            actions,
            r#"
            (local.set $replacement (i32.load8_u offset={address}
              (i32.and (i32.shr_u (local.get $state) (i32.const {shift})) (i32.const {mask}))))
            (if (i32.ne (local.get $replacement) (i32.const 255)) (then
              (if (i32.gt_u (local.get $replacement) (i32.const {mask})) (then unreachable))
              (if (i64.eqz (i64.and (i64.const 0x{valid:x})
                        (i64.shl (i64.const 1) (i64.extend_i32_u (local.get $replacement)))))
                (then unreachable))
              (local.set $generated (i64.add (local.get $generated) (i64.const 1)))
              (local.set $dest (i32.or (i32.and (local.get $state) (i32.const 0x{keep:x}))
                                    (i32.shl (local.get $replacement) (i32.const {shift}))))
              (if (call $discover (local.get $dest)) (then
                (i32.store offset={queue}
                 (i32.shl (local.get $write) (i32.const 2)) (local.get $dest))
                (local.set $write (i32.add (local.get $write) (i32.const 1)))))))
        "#
        )
        .unwrap();
    }
    format!(
        r#"
        (module (memory {pages} {pages})
          {data}
          (func $discover (param $value i32) (result i32)
            (local $address i32) (local $bit i32) (local $old i32)
            (local.set $address (i32.shr_u (local.get $value) (i32.const 3)))
            (local.set $bit (i32.shl (i32.const 1) (i32.and (local.get $value) (i32.const 7))))
            (local.set $old (i32.load8_u (local.get $address)))
            (if (i32.and (local.get $old) (local.get $bit)) (then (return (i32.const 0))))
            (i32.store8 (local.get $address) (i32.or (local.get $old) (local.get $bit)))
            (i32.const 1))
          (func (export "check") (result i64 i32 i32)
            (local $read i32) (local $write i32) (local $boundary i32)
            (local $state i32) (local $replacement i32) (local $dest i32)
            (local $generated i64) (local $before i64) (local $depth i32)
            (memory.fill (i32.const 0) (i32.const 0) (i32.const {seen_bytes}))
            (drop (call $discover (i32.const {root})))
            (i32.store offset={queue} (i32.const 0) (i32.const {root}))
            (local.set $write (i32.const 1))
            (local.set $generated (i64.const 1))
            (block $done (loop $level
              (br_if $done (i32.eq (local.get $read) (local.get $write)))
              (local.set $boundary (local.get $write))
              (local.set $depth (i32.add (local.get $depth) (i32.const 1)))
              (block $wave (loop $state
                (br_if $wave (i32.eq (local.get $read) (local.get $boundary)))
                (local.set $state (i32.load offset={queue}
                                (i32.shl (local.get $read) (i32.const 2))))
                (local.set $before (local.get $generated))
                {actions}
                (if (i64.eq (local.get $before) (local.get $generated)) (then unreachable))
                (local.set $read (i32.add (local.get $read) (i32.const 1)))
                (br $state)))
              (br $level)))
            (local.get $generated) (local.get $write) (local.get $depth)))
    "#
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args_os()
        .nth(1)
        .ok_or("expected rank program path")?;
    let program: Program = serde_json::from_reader(std::fs::File::open(input)?)?;
    program.validate()?;
    let expected = rust(&program)?;
    let compile = Instant::now();
    let engine = Engine::default();
    let module = Module::new(&engine, wasm(&program))?;
    let mut context = Store::new(&engine, ());
    let instance = Instance::new(&mut context, &module, &[])?;
    let check = instance.get_typed_func::<(), (i64, i32, i32)>(&mut context, "check")?;
    let compile_ns = compile.elapsed().as_nanos();
    let mut native = Vec::new();
    let mut jit = Vec::new();
    for rep in 0..6 {
        if rep % 2 == 0 {
            let start = Instant::now();
            let result = black_box(rust(black_box(&program))?);
            native.push(start.elapsed().as_nanos());
            assert_eq!(result, expected);
        } else {
            let start = Instant::now();
            let result = black_box(check.call(&mut context, ())?);
            jit.push(start.elapsed().as_nanos());
            assert_eq!(result, expected);
        }
    }
    println!(
        "{}",
        json!({
            "generated": expected.0, "distinct": expected.1, "depth": expected.2,
            "bits": program.bits(), "compile_ns": compile_ns, "rust_ns": native, "jit_ns": jit,
        })
    );
    Ok(())
}
