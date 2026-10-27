# Contributing

## Build and test

Rust 1.99 (`rust-toolchain.toml` selects it).

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

All four must pass; CI runs the first three on Linux and Windows. The tests that read real game files skip without a `GAME_DATA` configuration (see [tests/inputs/README.md](tests/inputs/README.md)). If you add a real-data input, declare it in `tests/inputs/mod.rs` and update the table in `tests/inputs/README.md`; the `real_data_inputs` test prints the table it expects when they differ.

The reader modules `#![deny]` unchecked arithmetic, indexing and truncating casts: use checked operations and return an `Error` naming the offset. A new refusal needs a test with a hand-built fixture; no game data goes into the repository.

## Reporting a file the reader cannot read

Open an issue with the "File the reader cannot read" template. Please include:

- the full error message (it names a byte offset and what was expected),
- the game, platform and version the file comes from, and its name inside the game's archive,
- the few lines of code you used, or `cargo run --example triangles -- <file>`,
- if you can, the first 256 bytes of the file as hex, and its SDK version (a tagfile's `SDKV` section, a packfile's contents version).

Please do **not** attach game files, archives or anything extracted from them: they are copyrighted and will be removed from the issue. The error offset and a hex excerpt are usually enough to reproduce the problem with a hand-built fixture.
