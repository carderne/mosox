# mosox

An LP matrix generator for GMPL (subset of AMPL).

Written in Rust.

Supports two features:
- compile a matrix (can be output to MPS)
- solve directly using HiGHS

## Commands
```bash
cargo make fmt
cargo make lint
cargo make test
cargo make testlarge  # slow
```

mosox usage:
```bash
> cargo run

LP matrix generator for GMPL

Usage: mosox <COMMAND>

Commands:
  compile    Load and output to MPS
  solve      Solve with HiGHS
  normalize  Normalize an MPS file for diffing
  compare    Compare two normalized MPS files with epsilon tolerance
  help       Print this message or the help of the given subcommand(s)
```
