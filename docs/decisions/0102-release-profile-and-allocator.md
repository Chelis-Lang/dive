# 0102: Release build profile and global allocator

- **Status:** accepted
- **Date:** 2026-10-19

## Context

ADR 0006 targets at least a 2x geometric-mean speed-up over TLC; dive is far
from it. The release profile set only `debug = "line-tables-only"`: no
link-time optimisation, 16 codegen units and the system allocator. The checker
allocates heavily (values, states, frontier records), and its hot paths cross
crate boundaries (`dive-eval`, `dive-value`, `dive-store`, `dive-check`) that
only LTO can inline across.

`panic = "abort"` is not an option: `dive-check` (`pool.rs`, `engine.rs`) uses
`catch_unwind` to report worker panics as internal errors.

## Measurements

The machine (32 threads, AMD Ryzen AI MAX+ 395, Linux, rustc 1.99.0) was shared
with other heavy jobs (load average 50 to 120), so the primary signal is the
instruction count from `valgrind --tool=cachegrind --cache-sim=no`. Every run
is `dive check --compat=tlc --seed 0 --fp 0 --workers 1` on the pinned Examples
model, as `dive-bench` runs it. Every variant reported the same verdict,
distinct and generated counts and depth.

Instructions executed, relative to the previous profile (`base`):

| Variant | SimpleRegular | MCbtree |
|---|---|---|
| base | 130.98 G | 585.68 G |
| thin LTO, 1 codegen unit | -6.1% | -5.1% |
| fat LTO, 1 codegen unit | -6.1% | -5.6% |
| mimalloc | -10.4% | -9.0% |
| thin LTO, 1 codegen unit, mimalloc | -15.6% | -13.2% |
| **fat LTO, 1 codegen unit, mimalloc** | **-16.7%** | **-15.4%** |

Wall clock compares `base` with the chosen variant: one process at a time,
three interleaved repetitions, the order reversed in the second. The machine
was loaded, so treat wall ratios as indicative; user CPU time is steadier.

| Model | median paired wall ratio | median user CPU | peak RSS |
|---|---|---|---|
| SimpleRegular | 1.39x faster | 37.0 s to 27.7 s (-25%) | 225 MB, unchanged |
| MCbtree | 1.45x faster | 157.9 s to 119.2 s (-25%) | 63 MB to 70 MB |

The small conformance tier gives identical outcome counts with the previous and
the new binary.

Cost: a clean `cargo build --release -p dive-cli` went from about 33 s to about
81 s on this machine (thin LTO: about 63 s). The dev and test profiles are
unchanged, so the fast gate's compile time is unaffected.

## Decision

- `[profile.release]` sets `lto = "fat"` and `codegen-units = 1`. The
  `profiling` profile inherits them. `panic` stays `unwind`.
- The `dive` and `tlc` binaries (`dive-cli`) declare
  `#[global_allocator] static GLOBAL: mimalloc::MiMalloc`. Library crates stay
  allocator-neutral, and no `unsafe` code is needed.
- `mimalloc` joins `[workspace.dependencies]` without default features. It
  brings `libmimalloc-sys`, `cc`, `shlex` and `find-msvc-tools`, all MIT or
  MIT/Apache-2.0 and accepted by `deny.toml`. It earns its place with the
  largest single reduction measured: about 10% of instructions on its own, and
  most of the wall-time gain.

This is result-preserving: no observable checker behaviour depends on the
allocator or on inlining, so there is no `--compat=tlc` setting and no
divergence entry.

## Consequences

- Building from source needs a C compiler, because `libmimalloc-sys` compiles
  mimalloc's C sources.
- Release builds, including CI's conformance-gate build, take longer.
- Revisit when a profile-guided or compiled tier lands, and re-measure the
  allocator under multi-worker runs on a quiet machine.
