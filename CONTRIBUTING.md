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

## Releasing

A release is a commit that changes a version. Once every CI job has passed on main, CI's release job
(`.github/release.py`) publishes each crate whose version crates.io does not have yet, in dependency order (trusted
publishing: no token is stored anywhere), tags the commit `v<version>` (`<package>-v<version>` for a package with a
version of its own) and creates the GitHub Release with that version's CHANGELOG section as its notes.
A commit that changes no version only runs CI.

1. Bump the version when a change is worth releasing: a patch (0.x.Y) for fixes and data or manifest additions, a minor
   (0.X.0) for new API or any breaking change before 1.0. Run `cargo check` so `Cargo.lock` follows.
2. In the same commit, move the change's CHANGELOG lines under a `## <version> (<date>)` heading.
3. Push to main. Never publish by hand; a new crate's first release is the one exception (the token holder makes it,
   then the crate gets its trusted-publishing config).

## Reporting a file the reader cannot read

Open an issue with the "File the reader cannot read" template. Please include:

- the full error message (it names a byte offset and what was expected),
- the game, platform and version the file comes from, and its name inside the game's archive,
- the few lines of code you used, or `cargo run --example triangles -- <file>`,
- if you can, the first 256 bytes of the file as hex, and its SDK version (a tagfile's `SDKV` section, a packfile's contents version).

Please do **not** attach game files, archives or anything extracted from them: they are copyrighted and will be removed from the issue. The error offset and a hex excerpt are usually enough to reproduce the problem with a hand-built fixture.
