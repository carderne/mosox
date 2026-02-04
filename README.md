# mosox

An LP matrix generator for GMPL (GNU MathProg).

**Currently a work-in-progress.**

It works for a subset of GMPL (specifically the subset required to run [Osemosys](https://osemosys.github.io/)). The goal is to full coverage of GMPL and compatibility with GLPK, but this may take a while.

## Known limitations
Hopefully this list is shorter then the "supported feature" list!

This list of limitations is made with reference to the GNU MathProg Language Reference which can be viewed [here](./docs/gmpl.pdf) or downloaded from the original [here](https://www.gnu.org/software/glpk).

It is intended that all of these will ultimately be supported, and most of them are "trivial" to add.

### Functions
Not yet implemented:
- The following functions: `abs`, `atan`, `card`, `ceil`, `cos`, `exp`, `floor`, `gmtime`, `length`, `log`, `prod`, `round`, `sin`, `sqrt`, `str2time`, `trunc`, `Irand224`, `Uniform`, `Normal`.

### Expressions
Not yet implemented:
- These arithmetic operators: `less` (positive difference operator), `div` (quotient), `mod`
- These symbolic operators: `&` (string concatenation)
- These set expressions: "arithmetic" set, conditional set expressions, parenthesized set expressions
- These set operators: `union`, `diff`, `symdiff`.
- These logical expressions: iterated expressions (`forall`, `exists`)
- These logical relational expressions: `in`, `not in`, `within`, `not within`
- These logical operators: `not`

### Sets
Not yet supported:
- Elements larger than two-tuple.
- `within` is supported but not enforced

### Parameters
Not yet supported:
- Alias
- Relational condition (parsed but not enforced)
- Superset expression (parsed but not enforced)
- Type specifier (integer, binary, symbolic) (parsed but not enforced)

### Variables
Not yet supported:
- Fixed value
- Type specifier (integer, binary, symbolic) (parsed but not enforced)
- Relational condition (parsed but not enforced)

### Constraints
Not yet supported:
- Alias
- Multiple expressions (comma-separated)

### Objective function
Not yet supported:
- Alias

### Other statements
These are parsed but are otherwise ignored:
- Check statement
- Display statement
- Printf statement
- For statement

## Quickstart

### Installation
#### Using Cargo
```bash
cargo install mosox
```

#### Using Homebrew (macOS)
```bash
brew tap carderne/mosox-tap
brew install mosox
```

#### Pre-built binaries (macOS, Windows, Linux)
Binaries are built for a small set of systems and architectures.
Available to download (compressed) from the [Releases page](https://github.com/carderne/mosox/releases).
Please choose the appropriate archive.

### Usage overview

Usage overview:
```bash
> mosox help

MathProg Translation Kit

Usage: mosox <COMMAND>

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
mosox generate model.mod data.dat > output_file.mps
```

## Development
Please install [cargo-make](https://github.com/sagiegurari/cargo-make):
```bash
cargo install cargo-make
```

The most useful dev commands are listed in `Makefile.toml`.

You can view available commands by running `cargo make`.

### Formatting, linting
```bash
cargo make fmt
cargo make lint
```

### Testing
In addition to other tests, this will run mosox against all the examples under `examples/` and confirm that the output is identical to the existing MPS files.
```bash
cargo make test
```

This will additionally run a regression test against `examples/osemosys_large` if present.
```bash
cargo make testlarge
```

## Docs

- [Grammar](docs/GRAMMAR.md) - GMPL grammar specification and coverage

## Todo

- [ ] Support more than two-tuples in sets
- [ ] Add regression test suite
- [ ] Add fully worked examples
- [ ] Add performance comparison suite
- [ ] Add Highs integration
