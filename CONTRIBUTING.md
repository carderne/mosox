# Contributing to MOSOX

MOSOX welcomes bug reports, examples, documentation improvements and code contributions. Please use [GitHub Issues](https://github.com/carderne/mosox/issues) for questions, reproducible bug reports and feature proposals.

For a bug report, include the MOSOX version, operating system, command, expected result and actual output. Adding small `.mod` and/or `.dat` files will make problems much easier to reproduce.

To contribute code, fork the repository and create a branch from `main`. Build with a current stable Rust toolchain. The HiGHS dependency also needs a working native C/C++ build environment and CMake. Then run:

```sh
cargo build --locked
cargo fmt --check
cargo clippy --locked --all-targets --all-features
cargo test --locked --all-features
```
