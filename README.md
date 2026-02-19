# mosox

An LP matrix generator for [GMPL (MathProg)](https://en.wikibooks.org/wiki/GLPK/GMPL_(MathProg)) and (by extension) [AMPL](https://en.wikipedia.org/wiki/AMPL).

Can be used in two ways:
- Compile an MPS file for solving elsewhere
- Solve directly with the [HiGHS](https://highs.dev/) integration, no other binary needed

**Currently a work-in-progress.**

It works for a subset of GMPL (specifically the subset required to run [Osemosys](https://osemosys.github.io/)).
See [Known limitations](#known-limitations) section below.

There are a number of examples of varying complexity in the [examples/](./examples/) directory.

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
  compile    Load and output to MPS
  solve      Solve with HiGHS
  normalize  Normalize an MPS file for diffing
  compare    Compare two normalized MPS files with epsilon tolerance
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

### Compile MPS
Compile MPS for one of the examples in this repo.
Output will be written to stdout.
```bash
mosox compile examples/basic/model.mod
```

Or an example with a separate data file, piping MPS to a file:
```bash
mosox compile examples/sets/model.mod examples/sets/data.dat > output.mps
```

### Solve a model
Results summary will be printed to stdout.
```bash
mosox solve examples/basic/model.mod

# output
OBJECTIVE VALUE
2200.000

CONSTRAINTS
labor = 100.000; marginal = 16.667
material = 80.000; marginal = 6.667

VARS
x1 = 40.000; marginal = -0.000
x2 = 20.000; marginal = -0.000
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

## Benchmarks
Run performance benchmarks across all examples (except `osemosys_large`):
```bash
cargo make bench
```

Include `osemosys_large` (requires the example to be present):
```bash
cargo make benchlarge
```

Run with `glpsol` comparison (requires `glpsol` on `PATH`):
```bash
cargo make benchglpsol
```

#### Performance vs glpsol (median, release build, Macbook Air M2)
Note that some of the performance advantage over glpsol may be caused by the limitations enumerated below.

| Example          | rows/cols/nonzero | N   | glpsol | mosox  | Speedup |
|---------         | ----------------- | --- |--------|------- |---------|
| osemosys_small   | 6k/7k/15k         | 50  | 68ms   | 17ms   | 4.0x    |
| osemosys_atlantis| 180k/230k/510k    | 10  | 2.4s   | 373ms  | 6.5x    |
| osemosys_large   | 1M/5M/12M         | 4   | 130s   | 20s    | 6.5x    |

### Memory usage

The matrix generator uses very roughly 2,000 bytes per non-zero.
This is a significant overhead over the 12 bytes bytes per non-zero that would be needed in a format like CSC.
However, as the table below shows, the matrix generator will rarely (if ever) use more memory than the solver.

| Example          | Mosox matrix | Glpsol matrix | HiGHS solver | Glpsol solver |
|---------         | ------------ | ------------- | ------------ | ------------- |
| osemosys_small   | 13 MB        | 12 MB         | 24 MB        | 16 MB         |
| osemosys_atlantis| 244 MB       | 287 MB        | 386 MB       | 416 MB        |
| osemosys_large   | 5 GB         | 5.6 GB        | 6.5 GB       | ?             |

## Known limitations
This list of limitations is made with reference to the GNU MathProg Language Reference which can be viewed [here](./docs/gmpl.pdf) or downloaded from the original [here](https://www.gnu.org/software/glpk).

It is intended that all of these will ultimately be supported, and most of them are "trivial" to add.

### Functions
- The following functions: `abs`, `atan`, `card`, `ceil`, `cos`, `exp`, `floor`, `gmtime`, `length`, `log`, `prod`, `round`, `sin`, `sqrt`, `str2time`, `trunc`, `Irand224`, `Uniform`, `Normal`.

### Expressions
- These arithmetic operators: `less`, `div`, `mod`
- These symbolic operators: `&` (string concatenation)
- These set expressions: "arithmetic" set, conditional set expressions, parenthesized set expressions
- These set operators: `union`, `diff`, `symdiff`.
- These logical iterated expressions: `forall`, `exists`
- These logical operators: `not`

### Sets
- Elements larger than two-tuple.
- `within` (parsed, not enforced)

### Parameters
- Relational condition (parsed, not enforced)
- Superset expression (parsed, not enforced)
- Type specifier (integer, binary, symbolic) (parsed, not enforced)

### Variables
- Bounds specified as expressions (currently only constant accepted)

### Constraints
- Multiple expressions (comma-separated)

### Other statements
Support for these statements is not planned (results should instead be parsed from the solver output): `display`, `printf`, `for`.
