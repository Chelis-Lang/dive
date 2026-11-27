# Install

Each install method gives you two commands: `dive`, and `tlc`, an adapter
that accepts TLC's command-line arguments. Neither needs a JVM.

## From crates.io

With Rust 1.99 or later and a C compiler installed:

```sh
cargo install dive-cli --locked
```

The C compiler (`cc`) is needed because dive links the mimalloc allocator.
Release builds use whole-program link-time optimization, so the install takes
a few minutes.

To install the current `main` from Git instead:

```sh
cargo install --git https://github.com/Chelis-Lang/dive --locked dive-cli
```

## Linux release archives

Each GitHub release has archives for `x86_64-unknown-linux-gnu` and
`aarch64-unknown-linux-gnu`, each with a SHA-256 checksum file. For example,
on x86_64:

```sh
base=https://github.com/Chelis-Lang/dive/releases/download/v0.2.0
name=dive-v0.2.0-x86_64-unknown-linux-gnu
curl -LO "$base/$name.tar.gz" -LO "$base/$name.tar.gz.sha256"
sha256sum -c "$name.tar.gz.sha256"
tar -xzf "$name.tar.gz"
install -m 755 "$name/dive" "$name/tlc" ~/.local/bin/
```

The archive also holds the license files of the bundled TLA+ libraries.

## From a source checkout

Build with the toolchain pinned in `rust-toolchain.toml`:

```sh
cargo build --release -p dive-cli
```

Use `target/release/dive`, or put it on your `PATH`.

## Check the install

```sh
dive --version
dive --help
```

`dive --version` prints the package version and, in parentheses, the commit
the build came from: `dive 0.2.0 (<commit>)`. The commit takes one of these
forms:

- From crates.io (`cargo install dive-cli`), the first 12 hex digits of the
  commit Cargo recorded when the package was published, with `-dirty` if it
  was packaged from uncommitted changes.
- From a Git checkout, including the Linux release archives, the nearest
  release tag in `git describe` form (`v0.2.0-0-g<hash>` on a tagged
  release), or the abbreviated commit hash when no release tag precedes it.
- From a source tree with neither, `unknown`.

`dive --help` lists the commands, and `dive <command> --help` lists a
command's options. Help needs no input files.

The standard modules, TLAPS and the CommunityModules TLA+ sources are built
in, so most specifications need no library path or separate download.
Local modules and explicit search paths take precedence over the bundled
ones. [Modules](modules.md) describes operator support.

Next, check a first model in [getting started](getting-started.md).
