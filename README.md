# MPTK (MathProg Translation Kit)

## Objectives:
- [x] A grammar capable of parsing [GMPL](https://en.wikibooks.org/wiki/GLPK/GMPL_(MathProg)) .mod and .dat files
- [x] Complete internal representation (IR) of parsed models and data
- [x] Collate model and data sections
- [x] Interpret functions, domains etc in the model 
- [z] Output to [MPS](https://en.wikipedia.org/wiki/MPS_(format))

## Quickstart
Install
```bash
cargo install mptk
```

Usage
```bash
mptk osemosys.mod atlantis.dat
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

- [Code Overview](docs/CODE_OVERVIEW.md) - Architecture and implementation details
- [Grammar](docs/GRAMMAR.md) - GMPL grammar specification and coverage
- [Example IR](docs/EXAMPLE_IR.md) - Sample intermediate representation output
- [Future Plan](docs/FUTURE_PLAN.md) - Development roadmap for Phases 2-3

## Notes
### Note on indexing

There are three concepts in domain indexing.

#### SET (generically, dimension)
- type: String
- Upper-case name for a dimension
- `YEAR`
- (The set directive supplies all the values)

#### INDEX (set_index, con_index etc)
- type: String
- A single index is a single lower-case letter
- `y`
- That a var/param/constraint uses to index a given set/dimension

#### VALUE
- type: `IndexVal`
- represents a single actual value in this set/dimension
- `2014`
