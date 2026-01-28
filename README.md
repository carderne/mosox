# MPTK (MathProg Translation Kit)

## Quickstart
Install:
```bash
cargo install mptk
```

Usage overview:
```bash
> mptk help

MathProg Translation Kit

Usage: mptk <COMMAND>

Commands:
  check     Check for errors and quit
  generate  Load and output to MPS
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

Generate MPS for a model and data file pair:
```bash
mptk generate model.mod data.dat > output_file.mps
```

## Development
Please install [cargo-make](https://github.com/sagiegurari/cargo-make):
```bash
cargo install cargo-make
```

The most useful dev commands are listed in `Makefile.toml`.

You can view available commands by running `cargo make`.

Run fmt, lint, check, test:
```bash
cargo make ci
```

Run against the full Osemosys model and Atlantic data:
```bash
cargo make run
```

## Docs

- [Grammar](docs/GRAMMAR.md) - GMPL grammar specification and coverage
