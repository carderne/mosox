# mosox

An LP matrix generator for [GMPL (MathProg)](https://en.wikibooks.org/wiki/GLPK/GMPL_(MathProg)) and (by extension) [AMPL](https://en.wikipedia.org/wiki/AMPL).

Can be used in several ways:
- Compile an MPS file for solving elsewhere
- Solve directly with the [HiGHS](https://highs.dev/) integration, no other binary needed
- From Python to pass data directly from Pandas

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
mosox help

LP matrix generator for GMPL

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
Output will be written to the provided file:
```bash
mosox compile examples/basic/model.mod -o output.mps
```

Or an example with a separate data file, writing MPS to a file:
```bash
mosox compile examples/sets/model.mod examples/sets/data.dat -o output.mps
```

### Solve a model
Results will be saved to the output file:
```bash
mosox solve examples/basic/model.mod -o output.txt

# Loading model from model.mod
# Generating matrix
# Matrix compiled in 666.625µs
# Matrix: 3 rows, 2 cols, 6 nonzero
# Solving matrix with HiGHS
# Solved in 1.939083ms
# Objective value: 2200
# Results output to output.txt
```

### Python
Data can be passed from Python instead of (or on top of) a `.dat` file:
pandas DataFrames or Series, pyarrow Tables, or plain values.
```python
import mosox

model = mosox.Model.from_file("examples/2d_params/model.mod")
solution = model.solve({
    "PLANTS": ["P1", "P2"],
    "supply": {"P1": 100, "P2": 150},
    "cost": df,  # PLANTS, WAREHOUSES, VALUE (matched by name, else read by position)
    ...
})
solution.status, solution.objective
solution["ship"]  # DataFrame with columns PLANTS, WAREHOUSES, VALUE, MARGINAL
```

A param table's index columns are matched by name if they're named for the domain's sets or dummies
(`REGION` or `r` in `{r in REGION}`), and otherwise read by position: the index columns, then the value.
A single value fills an indexed param.

`Data` layers data, eg a `.dat` file (parsed once) or [otoole](https://github.com/OSeMOSYS/otoole) data,
patched or replaced for each scenario:
```python
base = mosox.Data.from_dat("ose.dat")
base = mosox.Data.from_otoole(*otoole.read("config.yaml", "csv", "data/"))
model.solve(base.update({"CapitalCost": cheap_solar}, defaults={"EmissionsPenalty": 0}))
```

Scenarios run with progress events, in parallel, optionally writing each solution to a store:
```python
scenarios = mosox.product(
    carbon={"low": {"EmissionsPenalty": 10}, "high": {"EmissionsPenalty": 100}},
    discount={r: {"DiscountRate": r} for r in [0.03, 0.05]},
)
run = model.run(scenarios, base, workers=4, keep=["NewCapacity"], store="results/")
for event in run:  # Started, Compiled, Finished, Failed, Skipped (also `async for`)
    print(f"{event.done}/{event.total}", type(event).__name__, event.scenario.name)
results = run.results()  # or mosox.Results.open("results/")
results.summary          # a row per scenario: coords, status, objective, timings
results["NewCapacity"]   # all scenarios, with scenario and coord columns first
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
cargo make ci  # fmt-check, lint, test (same as CI)
```

`lint` and `test` enable all features, including the Python bindings, which need Python >= 3.11.
If your default `python3` is older, point PyO3 at a newer one, e.g. `export PYO3_PYTHON=$PWD/.venv/bin/python` after `uv sync`.

### Testing
In addition to other tests, this will run mosox against all the examples under `examples/` and confirm that the output is identical to the existing MPS files.
```bash
cargo make test
```

This will additionally run a regression test against `examples/osemosys_large` if present.
```bash
cargo make testlarge
```

### Python bindings
Requires [uv](https://docs.astral.sh/uv/). `uv run` builds the extension (unoptimised) as needed.
```bash
cargo make py  # ruff format, ruff check, ty check, pytest
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
This is a significant overhead over the 12 bytes per non-zero that would be needed in a format like CSC.
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
- The following functions: `abs`, `atan`, `ceil`, `cos`, `exp`, `floor`, `gmtime`, `length`, `log`, `prod`, `round`, `sin`, `sqrt`, `str2time`, `trunc`, `Irand224`, `Uniform`, `Normal`.

### Expressions
- These arithmetic operators: `less`, `div`, `mod`
- These symbolic operators: `&` (string concatenation)
- These set expressions: conditional set expressions
- String ordering comparisons (`<`, `>` etc. between symbolic values)

### Sets
- `within` and `dimen` are not enforced on computed sets that fail to evaluate at load (eg sets over tuple domains)

### Parameters
- Type specifiers, relational conditions and superset (`in`) expressions are only enforced on explicit data and constant defaults, not on computed params (`:=` expressions) or expression defaults
- Plain-format data for multi-dimensional params without a slice (eg `param p := a x 1 b y 2;`) is not supported (it is caught as out-of-domain data)

### Variables
- Bounds specified as expressions (currently only constant accepted)

### Constraints
- Multiple expressions (comma-separated)

### Other statements
Support for these statements is not planned (results should instead be parsed from the solver output): `display`, `printf`, `for`.
