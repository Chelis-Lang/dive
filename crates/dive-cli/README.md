# dive-cli

**A faster TLA+ toolchain.** Same specs, same results as TLC, about 6x faster.

- **6x faster than TLC** on average across 169 models from tlaplus/Examples. 124 of them run at least 3x faster, and 34 at least 10x faster.
- **Instant on small specs.** `DieHard` checks in about 5.5 ms. TLC takes about 450 ms.
- **A fraction of the memory.** 24–102 MiB where TLC uses 0.8–1.2 GiB.
- **Drop-in.** Reads your existing `.tla` and `.cfg` files. Includes a `tlc` command that accepts TLC's arguments.
- **One binary** for model checking, simulation, parsing, PlusCal translation and trace validation.

```sh
cargo install dive-cli --locked
dive check MySpec.tla
```

Installing needs Rust 1.99 or later and a C compiler. See the
[dive repository](https://github.com/Chelis-Lang/dive) for the user guide and
how the numbers were measured. Rust callers can embed the `dive-api` library
instead.
