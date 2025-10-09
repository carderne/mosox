# mosaic

Starting point for converting GLPK MathProg .mod and .dat to MPS files.

Rough objectives:
- generate a grammar for .mod and .dat files
- convert to an IR of some sort
- output to MPS

## Quickstart
Install
```bash
# not yet available
```

Usage
```bash
mosaic osemosys.mod atlantis.dat
```

## GMPL
Language reference: https://en.wikibooks.org/wiki/GLPK/GMPL_(MathProg)

## Questions
1. MathProg has built-in functions like `abs(x)`, `max(x1, x2, ..., xn)`. These will need to be manually linearized, and not all will be supported (eg `tan(x)`).


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
