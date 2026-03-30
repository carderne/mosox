# mosox: Future Development Plan
## Enabling Large-Scale Sensitivity Analysis for Energy System Models: A Programmable Python Interface for mosox

---

## Background

`mosox` is a high-performance GMPL (MathProg) parser and linear-programming matrix generator, developed by and for [Climate Compatible Growth](https://climatecompatiblegrowth.com/). In its current form, it operates as a command-line tool: it reads a model file (`.mod`) and an optional data file (`.dat`), compiles the algebraic structure into a sparse constraint matrix, and solves it using the embedded [HiGHS](https://highs.dev/) solver — without any external binary dependency.

Performance against the reference implementation (`glpsol`) is strong:

| Example           | Size (rows/cols/nonzero) | mosox  | glpsol | Speedup |
|-------------------|--------------------------|--------|--------|---------|
| osemosys_small    | 6k / 7k / 15k            | 17 ms  | 68 ms  | 4×      |
| osemosys_atlantis | 180k / 230k / 510k       | 373 ms | 2.4 s  | 6.5×    |
| osemosys_large    | 1M / 5M / 12M            | 20 s   | 130 s  | 6.5×    |

*Benchmarks: median of N iterations (50, 10, 4 respectively), release build with `lto = "fat"`, MacBook Air M2, 16 GB RAM, macOS, Rust 1.85.*

The next phase of development transforms `mosox` from a one-shot CLI tool into a **programmable, embeddable modelling library** — accessible from both Rust and Python — that enables systematic exploration of model parameters at scale.

---

## Motivation

Energy system models such as [OSeMOSYS](https://osemosys.github.io/) and similar frameworks are increasingly used to perform:

- **Sensitivity analysis** — how does the optimal solution shift as a single parameter (e.g. discount rate, fuel cost) varies?
- **Scenario sweeps** — what are the outcomes across a structured grid of assumptions?
- **Ensemble modelling** — large batches of runs covering uncertainty distributions, used to quantify model robustness.

Today, each of these requires the researcher to manually script file manipulation, invoke the solver repeatedly, and aggregate outputs. This is error-prone, slow (model files are re-parsed on every run), and puts a high barrier in front of non-specialist users.

`mosox` is uniquely positioned to address this: because it already owns the full pipeline from GMPL text to solver, it can load a model once, hold it in memory, apply lightweight parameter overrides, and dispatch many solves — without redundant parsing or file I/O.

**A concrete example:** a researcher studying decarbonisation pathways for Southeast Asia currently takes ~3 hours to run 50 scenarios, each requiring manual file editing, `glpsol` invocation, and output aggregation. With mosox's sweep API on an Atlantis-scale model, 50 solves complete in under 20 seconds (50 × 373 ms). A 10,000-sample Monte Carlo ensemble — currently impractical — becomes an overnight run on a single compute node (~1 hour wall-clock with parallel dispatch).

---

## Landscape: Existing Tools and the Gap mosox Fills

Several mature tools exist in the energy modelling and mathematical programming ecosystems. mosox is not intended to replace them, but to fill a specific gap that none currently addresses.

| Tool | Language | Approach | Limitation for this use case |
|------|----------|----------|------------------------------|
| **glpsol** | C (CLI) | Reference GMPL solver | No library API; file-based; no parameter override without re-parsing |
| **Pyomo** | Python | Algebraic modelling language | Requires rewriting models in Python; no GMPL compatibility |
| **JuMP** | Julia | Algebraic modelling language | Requires rewriting models in Julia; different ecosystem |
| **linopy** / **CVXPY** | Python | LP/MIP construction APIs | Matrix-level; no GMPL support; models must be rebuilt from scratch |
| **otoole** | Python | OSeMOSYS data manipulation | Manages CSV data files, but still invokes `glpsol` externally for solving |

The key differentiator for mosox is: **it works with existing GMPL model files without rewriting them**. The OSeMOSYS community has a large corpus of `.mod` and `.dat` files representing years of modelling work. mosox enables programmatic exploration of these models directly, preserving the investment in existing model code while unlocking capabilities that currently require migration to a different modelling language.

---

## Proposed Architecture

### Core concept: model loading is separated from solving

The central design principle is a **load-once, solve-many** architecture:

```
GMPL .mod file          GMPL .dat file (optional)
       │                        │
       └──────────┬─────────────┘
                  ▼
           Parse & intern           ← done once, held in memory
                  │
                  ▼
          ModelWithData             ← base model object
         /            \
   override         override        ← lightweight parameter patches
        │                │
     compile          compile       ← full matrix generation (parallelisable)
        │                │
      solve            solve        ← HiGHS invocations (parallelisable)
        │                │
     results          results
```

The existing Rust internals already separate parsing (`load_model`, `load_data`), merging (`merge_model`), matrix compilation (`generate_matrix`), and solving (`solve_matrix`). The new library layer is built on top of these primitives.

### Where the performance savings come from — and don't

It is important to be precise about what "load once" saves. The current pipeline has three expensive stages:

1. **GMPL parsing and string internment** — reading `.mod` and `.dat` files, running the PEG parser, and interning all symbol names via `lasso`. For `osemosys_large`, this is a meaningful fraction of total time.
2. **Matrix compilation** — walking the parsed AST (`Expr`, `Domain`, etc.), evaluating set memberships, expanding constraint domains, and building the sparse coefficient matrix. This is the most expensive stage for large models.
3. **HiGHS solving** — the LP/MIP solve itself.

The load-once architecture eliminates stage (1) for every variant after the first. **Stage (2) — matrix compilation — is re-run in full for each variant.** The current `gen_matrix` function consumes the `ModelWithData` and walks the entire AST to produce the `Compiled` matrix; parameter overrides change the data that the AST evaluates against, but do not short-circuit the compilation.

This is still a substantial improvement over the status quo (re-parsing from files on every run), and for many models parsing is a significant fraction of total cost. But the proposal does not claim to patch compiled matrices — each variant requires a full matrix compilation pass.

A future optimisation could introduce incremental matrix updates for parameters that affect only a subset of constraints (e.g. right-hand-side coefficients), but this is architecturally complex and is not part of the current proposal.

### Engineering prerequisite: non-consuming matrix generation

The current Rust function signature is:

```rust
pub fn gen_matrix(model: ModelWithData) -> Result<Compiled>
```

This *moves* (consumes) the `ModelWithData`, meaning it cannot be called twice on the same object. For the sweep architecture to work, one of the following must be implemented:

- **Option A:** Change `gen_matrix` to take `&ModelWithData` (borrow instead of move), which requires modifying the matrix compilation internals to avoid taking ownership of AST nodes.
- **Option B:** `Clone` the `ModelWithData` for each variant. This is correct but has memory cost proportional to model size per variant.
- **Option C:** Use `Arc`-based sharing for the immutable AST, with per-variant overlay structures for overridden data.

The recommended approach is **Option A** (refactor to borrow), falling back to **Option B** (clone) for the initial implementation if the refactor proves too invasive. This is scoped as a prerequisite task in Phase 1.

---

## GMPL Coverage

mosox currently implements the subset of GMPL required to run [OSeMOSYS](https://osemosys.github.io/) models. The following GMPL features are **not yet supported**:

- **Functions:** `abs`, `ceil`, `floor`, `round`, `exp`, `log`, `sin`, `cos`, `atan`, `sqrt`, `trunc`, `length`, `gmtime`, `str2time`, `prod`, `Irand224`, `Uniform`, `Normal`
- **Operators:** `less`, `div`, `mod`, `&` (string concatenation)
- **Set operations:** `diff`, `symdiff`, conditional set expressions
- **Logical expressions:** `forall`, `exists`, `not`
- **Enforcement:** `within`, `dimen`, relational conditions, and type specifiers are parsed but not enforced
- **Variable bounds:** only constants accepted, not expressions
- **Statements:** `display`, `printf`, `for` (not planned — results should be parsed from solver output)

For the OSeMOSYS ecosystem — the primary target — this coverage is sufficient. All standard OSeMOSYS models compile and solve correctly. However, researchers using GMPL features outside this subset (e.g. custom models with `abs` or `mod` in constraints) would need to wait for expanded coverage.

The phasing section below includes a dedicated task for extending GMPL coverage based on user demand. Most of the missing features are individually straightforward to add (each is a new case in the expression evaluator), but collectively they represent non-trivial effort.

---

## Python API (Primary Interface)

The Python bindings are implemented via [PyO3](https://pyo3.rs/), exposing a clean, ergonomic interface to Python users without requiring any understanding of Rust.

### 1. Loading a model

```python
import mosox

# Load model and optional data
model = mosox.from_files(model="osemosys.mod", data="atlantis.dat")

# Or load model only — data supplied later
model = mosox.from_files(model="osemosys.mod")
```

`from_files` parses both files and interns all symbol names, but does **not** generate the matrix or solve. The result is a `Model` object that lives in memory until explicitly released.

### 2. Inspecting undeclared sets and parameters

Before solving, the user may want to check what data the model still requires:

```python
model.missing()
# Returns:
# {
#   "sets": ["YEAR", "REGION", "TECHNOLOGY"],
#   "params": ["DiscountRate", "CapacityFactor", ...]
# }
```

**Design note:** Determining what is "missing" is non-trivial. GMPL parameters can have default values (constant or expression-based), inline data, or data supplied via `.dat` files. A parameter with a default is technically not "missing" even if no explicit data is provided — it will evaluate to the default for any index not otherwise assigned. Sets can be defined via expressions (e.g. `set YEAR := 2020..2030;`) rather than explicit enumeration.

The `missing()` implementation will use a conservative definition: a set or parameter is reported as missing if it has no data assignment (inline, `.dat`, or via `assign_set`/`assign_param`) **and** no default value or defining expression. This covers the common case (data-driven models like OSeMOSYS where sets and parameters are populated entirely from `.dat` files) without requiring full expression evaluation. An optional `strict=True` mode could additionally flag parameters where data is partial (some index combinations are unassigned and would fall through to a default).

### 3. Assigning sets and scalar parameters

```python
# Assign a set
model.assign_set("YEAR", [2020, 2021, 2022, 2023, 2024])
model.assign_set("REGION", ["UK", "DE", "FR"])

# Assign a scalar parameter
model.assign_param("DiscountRate", 0.08)

# Override a parameter that was already in the .dat file
model.assign_param("DiscountRate", 0.10)
```

### 4. Assigning higher-dimensional parameters

1-D parameters take a plain Python list; 2-D and above accept NumPy arrays or xarray DataArrays, with dimension labels used to match the model's index sets:

```python
import numpy as np
import xarray as xr

# 2-D parameter: CapacityFactor[TECHNOLOGY, YEAR]
cf = xr.DataArray(
    data=np.random.rand(3, 5),
    dims=["TECHNOLOGY", "YEAR"],
    coords={"TECHNOLOGY": ["Coal", "Wind", "Solar"], "YEAR": list(range(2020, 2025))},
)
model.assign_param("CapacityFactor", cf)

# Plain numpy also accepted; dimension order must match model declaration
model.assign_param("CapacityFactor", np.array([[...], [...], [...]]))
```

xarray is preferred for higher-dimensional data because coordinate labels are validated against the model's already-assigned set members — a mismatch raises a clear error rather than producing silently wrong results. NumPy arrays are also accepted; dimension order must match the model declaration.

**Design note: xarray/NumPy integration complexity.** Accepting xarray DataArrays requires:

- Mapping xarray dimension names to GMPL set names (which may differ in case or naming convention)
- Validating coordinate labels against interned set members (including handling GMPL's mixed string/integer set values)
- Flattening multi-dimensional arrays into the sparse key→value representation used internally by the parameter IR
- Handling edge cases: missing coordinates, extra coordinates, NaN values, non-contiguous index subsets

This is a meaningful design and implementation surface, scoped as a dedicated task within Phase 2.

### 5. Solving a single model

```python
result = model.solve()

print(result.status)             # "optimal" | "infeasible" | "unbounded" | ...
print(result.objective)          # float — optimal objective value
print(result.variables)          # dict[str, float] — primal variable values
print(result.duals)              # dict[str, float] — dual values (shadow prices) per constraint
print(result.reduced_costs)      # dict[str, float] — reduced costs per variable
result.to_csv("output.csv")
result.to_dataframe()            # pandas DataFrame — all of the above in tidy form
```

All solver outputs are surfaced: primal variable values, dual values (shadow prices on constraints), and reduced costs. These are relevant for post-optimality analysis — for example, dual values on capacity constraints directly indicate the marginal value of additional generation capacity.

### 6. Infeasibility diagnostics

For sensitivity analysis and ensemble runs, a significant fraction of parameter combinations may produce infeasible models. Returning `status: "infeasible"` alone is insufficient for research use — the user needs to understand *why* the model is infeasible.

```python
result = model.solve()

if result.status == "infeasible":
    iis = result.iis()  # Irreducible Infeasible Set
    print(iis.constraints)  # list of constraint names in the IIS
    print(iis.variables)    # list of variable bounds in the IIS
    print(iis.to_dataframe())
```

HiGHS supports IIS computation natively. The `iis()` method exposes this, identifying the minimal subset of constraints and variable bounds that together cause infeasibility. For ensemble results, a summary view reports the feasibility rate and groups infeasible results by common IIS patterns:

```python
results = ensemble.solve_all()
print(results.feasibility_summary())
# { "optimal": 9420, "infeasible": 580, "unbounded": 0 }

infeasible = results.filter(status="infeasible")
```

### 7. Parameter sweeps and ensemble runs

The sweep API supports two modes: **Cartesian product** (the default) and **zip**.

**Cartesian product** — every combination of the provided values is solved:

```python
ensemble = model.sweep(
    DiscountRate=[0.05, 0.06, 0.07, 0.08, 0.09, 0.10],
    CoalCost=[800, 1200, 1600, 2000],
)
# 6 × 4 = 24 solves, run in parallel
results = ensemble.solve_all()

# Results indexed by the parameter combination
df = results.to_dataframe()
# Columns: DiscountRate, CoalCost, objective, <variable columns...>
```

Large sweeps grow combinatorially — 5 parameters with 10 values each yield 100,000 solves. Users should be mindful of this.

**Zip** — parameters are paired element-wise, like Python's `zip()`. All lists must be the same length; an error is raised if they are not:

```python
ensemble = model.sweep(
    DiscountRate=[0.05, 0.06, 0.07],
    CoalCost=[800, 1200, 1600],
    mode="zip",
)
# 3 solves: (0.05, 800), (0.06, 1200), (0.07, 1600)
results = ensemble.solve_all()
```

Zip mode is useful for scenario analysis where parameter combinations have a specific meaning (e.g. paired assumptions from a narrative scenario set) rather than being an exhaustive grid.

Higher-dimensional parameters (NumPy arrays or xarray DataArrays) are treated as atomic values in both modes — each array in a list is one "slab" to be assigned as a whole:

```python
# Each entry in the list is a full 2-D assignment for CapacityFactor
ensemble = model.sweep(
    CapacityFactor=[cf_low, cf_mid, cf_high],  # each is an xarray DataArray
    mode="zip",
)
```

### 8. Memory management

The parsed model occupies memory proportional to its size (see benchmarks in Background section). For large models or long-running Python processes, explicit release is available:

```python
model.drop()
# After this point the model object is invalid; attempting to use it raises an error.
```

Reference counting via Python's garbage collector will also trigger cleanup when the last reference to a `Model` is dropped, but `drop()` provides deterministic release — important when memory is constrained and the next solve needs to begin immediately.

---

## Rust API

The Rust library exposes the same logical structure with idiomatic types:

```rust
use mosox::{Model, SweepBuilder};

// Load
let model = Model::from_files("osemosys.mod", Some("atlantis.dat"))?;

// Assign
model.assign_set("YEAR", &[2020, 2021, 2022])?;
model.assign_param("DiscountRate", mosox::Value::Scalar(0.08))?;

// Solve
let result = model.solve()?;
println!("Objective: {}", result.objective);

// Sweep
let results = SweepBuilder::new(&model)
    .param("DiscountRate", &[0.05, 0.06, 0.07])
    .param("CoalCost", &[800.0, 1200.0, 1600.0])
    .solve_all()?;
```

The Python bindings are a thin PyO3 wrapper over these Rust types — there is no separate Python implementation.

---

## Performance Design

Memory efficiency and throughput are first-class concerns throughout.

### Load-once internment

The GMPL parser uses string internment (via [`lasso`](https://crates.io/crates/lasso)) so that every symbol name — set members, parameter names, constraint labels — is stored once and referenced by an integer handle everywhere else. A large model with tens of thousands of repeated identifiers uses the same memory as one with unique names.

### Parameter overrides as data patches

The parsed `ModelWithData` is treated as immutable after load. Parameter overrides for a sweep variant do **not** copy the entire model — they are stored as a small patch structure (`HashMap<SymbolId, Value>`) that is applied during matrix compilation. This means:

- The base model is loaded once: one parse, one intern pass.
- A 1,000-variant sweep stores 1,000 small patch objects, not 1,000 full model copies.
- **Matrix compilation is re-run in full for each variant** — the patch changes the data the AST evaluates against, but the full constraint expansion and coefficient computation is repeated.
- Matrix compilation and solving for each variant are independent and fully parallelisable.

For context, on `osemosys_atlantis`, parsing takes ~50 ms and matrix compilation takes ~150 ms. A 1,000-variant sweep saves ~50 seconds of redundant parsing while spending ~150 seconds on matrix compilation — a meaningful but not transformative saving at this model size. The larger win is architectural: eliminating file I/O and enabling in-process parallel dispatch.

### Parallel solve dispatch

`ensemble.solve_all()` uses Rayon's work-stealing thread pool to dispatch matrix generation and HiGHS invocations across all available CPU cores. Each variant is independent — there is no shared mutable state between workers.

For very large models (e.g. osemosys_large at ~5 GB per matrix), the user can control concurrency to respect available RAM:

```python
results = ensemble.solve_all(max_workers=2)  # limit concurrent solves
```

### Memory budget for large ensembles

Matrix memory scales with model size (roughly 2,000 bytes per non-zero element, per the current implementation). For ensemble runs, `mosox` can operate in two modes:

- **Eager** (default for small models): compile all matrices, then solve all.
- **Streaming** (default for large models, or when `max_memory` is set): compile → solve → discard one variant at a time, keeping only the result in memory.

```python
results = ensemble.solve_all(max_memory="8GB")
```

### Reproducibility and determinism

HiGHS is deterministic for a given input matrix — identical parameter values will always produce identical solutions regardless of thread scheduling in the parallel dispatch. Sweep results are collected and indexed by their parameter combination, not by completion order. A sweep configuration can be serialised (as JSON or similar) and re-run to reproduce results exactly.

---

## Incremental / Background Solving

For interactive use (e.g. in a Jupyter notebook), solving can be dispatched to a background thread and results retrieved later:

```python
future = model.solve_async()
# ... do other work ...
result = future.get()  # blocks until done
```

For ensembles:

```python
future = ensemble.solve_all_async()
for result in future:   # yields results as they complete
    print(result.params, result.objective)
```

---

## Result Formats

All result objects (`Result`, `EnsembleResults`) expose:

| Method | Output |
|---|---|
| `.objective` | `float` |
| `.variables` | `dict[str, float]` |
| `.duals` | `dict[str, float]` |
| `.reduced_costs` | `dict[str, float]` |
| `.status` | `str` |
| `.iis()` | `IIS` (if infeasible) |
| `.to_dataframe()` | `pandas.DataFrame` |
| `.to_csv(path)` | writes CSV |
| `.to_netcdf(path)` | writes NetCDF (via xarray, ensemble results only) |
| `.to_dict()` | plain Python dict |

For ensemble results, the DataFrame is indexed by the sweep parameter values, making it straightforward to slice, plot, or feed into downstream analysis.

---

## Distribution

### Python package

The Python package (`mosox`) will be distributed via PyPI, built using [maturin](https://www.maturin.rs/). Pre-built wheels will be published for:

- Linux (x86-64, aarch64)
- macOS (x86-64, Apple Silicon)
- Windows (x86-64)

No compiler is required by end users.

```bash
pip install mosox
```

### Rust crate

The Rust library will be published to [crates.io](https://crates.io) as `mosox`, versioned separately from the CLI binary.

---

## Proposed Development Phases

### Phase 1: Rust Library API (estimated: 3 person-months)

**Scope:** Stabilise the public Rust library API for load-once, solve-many usage.

**Tasks:**
- Refactor `gen_matrix` to borrow `&ModelWithData` instead of consuming it (or implement `Clone`-based fallback). This is the critical prerequisite — the current move semantics prevent reuse.
- Define stable public types: `Model`, `Value` (scalar, 1-D, N-D), `Result`, `SolveStatus`.
- Implement the parameter override layer: `assign_set`, `assign_param` methods that mutate the `ModelWithData`'s data sections without re-parsing.
- Implement single-solve path through the new API.
- Add integration tests against all existing examples.

**Deliverable:** A Rust crate that can load a model, override parameters, and solve — callable from Rust code.

**Dependencies:** None (builds on existing codebase).

### Phase 2: PyO3 Bindings (estimated: 3 person-months)

**Scope:** Expose the Rust API to Python with ergonomic type conversions.

**Tasks:**
- Set up PyO3 project structure and maturin build configuration.
- Implement Python `Model` class wrapping the Rust `Model`.
- Implement `from_files`, `assign_set`, `assign_param`, `solve`, `drop` methods.
- NumPy array conversion: accept `numpy.ndarray` for N-D parameters, map to internal sparse representation. Handle dimension ordering, dtype conversion, NaN handling.
- xarray DataArray conversion: map dimension names to GMPL set names, validate coordinate labels against interned set members, handle mixed string/integer sets. This is a distinct task from NumPy support due to the label validation requirements.
- Implement `missing()` with the conservative definition described above.
- CI pipeline for cross-platform wheel building (manylinux, macOS universal2, Windows).
- Publish to PyPI (test index first, then production).

**Deliverable:** `pip install mosox` works on all major platforms; single-model load/override/solve works from Python.

**Dependencies:** Phase 1.

### Phase 3: Sweep / Ensemble (estimated: 2 person-months)

**Scope:** Cartesian product and zip sweep modes with parallel dispatch.

**Tasks:**
- Implement `SweepBuilder` in Rust with Cartesian product and zip expansion.
- Parallel dispatch via Rayon's work-stealing pool; each variant gets its own patch + compile + solve cycle.
- Implement streaming mode (compile → solve → discard) to bound memory usage for large ensembles.
- Implement `max_workers` and `max_memory` controls.
- Expose sweep API to Python via PyO3.
- Implement `EnsembleResults` with parameter-indexed access.

**Deliverable:** `model.sweep(...).solve_all()` works from Python with parallel execution.

**Dependencies:** Phases 1 and 2.

### Phase 4: Infeasibility Diagnostics (estimated: 1 person-month)

**Scope:** Expose HiGHS IIS computation and integrate with ensemble results.

**Tasks:**
- Implement `result.iis()` wrapping HiGHS IIS extraction.
- Map IIS constraint/variable indices back to named GMPL entities.
- Implement `results.feasibility_summary()` and `results.filter(status=...)` for ensemble results.

**Deliverable:** Researchers can diagnose why specific parameter combinations produce infeasible models.

**Dependencies:** Phases 1 and 3.

### Phase 5: Async Interface and Result Formats (estimated: 2 person-months)

**Scope:** Background solving for interactive use; comprehensive output formats.

**Tasks:**
- Implement `solve_async` and `solve_all_async` using Rust async runtime (tokio or similar), exposed to Python via PyO3's async support or a simple future/polling API.
- Implement `to_dataframe()`, `to_csv()`, `to_netcdf()`, `to_dict()` for both single and ensemble results.
- Ensure ensemble DataFrames are properly indexed by sweep parameters.

**Deliverable:** Full interactive workflow in Jupyter notebooks; results in all advertised formats.

**Dependencies:** Phases 1–3.

### Phase 6: GMPL Coverage Extension (estimated: 2 person-months, ongoing)

**Scope:** Expand GMPL support beyond the OSeMOSYS subset, prioritised by user demand.

**Tasks:**
- Implement missing mathematical functions (`abs`, `ceil`, `floor`, `round`, `exp`, `log`, `sqrt`). Each is a new case in the expression evaluator — individually simple, collectively ~2 weeks.
- Implement missing operators (`less`, `div`, `mod`).
- Implement missing set operations (`diff`, `symdiff`).
- Implement `not`, `forall`, `exists` in logical expressions.
- Enforce `within`, `dimen`, type specifiers (currently parsed but ignored).
- Expression-based variable bounds.

**Deliverable:** Broader GMPL compatibility enabling non-OSeMOSYS models.

**Dependencies:** None (can proceed in parallel with other phases).

### Total estimated effort: 13 person-months

---

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| **`gen_matrix` refactor proves invasive.** The move-to-borrow change in Phase 1 may require significant restructuring of the matrix compilation internals, which currently destructure the `ModelWithData`. | Medium | High (blocks all subsequent phases) | Fall back to `Clone`-based approach, which is correct but uses more memory. Profile to confirm memory impact is acceptable for target model sizes. |
| **HiGHS API changes or regressions.** mosox depends on the `highs` Rust crate (currently v2.0.0). | Low | Medium | Pin the HiGHS version. The `highs` crate wraps a stable C API. Upgrade on a deliberate schedule with regression tests. |
| **Cross-platform wheel building fails for some targets.** PyO3 + maturin + HiGHS (which includes a C++ library) across Linux/macOS/Windows is a complex CI matrix. | Medium | Medium | Start with Linux x86-64 and macOS ARM (the two most common research platforms). Add other targets incrementally. Use maturin's docker-based manylinux builds. |
| **xarray/NumPy type mapping edge cases.** GMPL's mixed string/integer set members don't map cleanly to NumPy dtypes. | Medium | Low | Accept only homogeneous-type coordinates per dimension. Document the constraint. Fall back to Python list-of-tuples for pathological cases. |
| **GMPL coverage gaps block adoption beyond OSeMOSYS.** Users with custom GMPL models may hit unsupported features. | High | Low (for CCG's core use case) | Scope the library explicitly for OSeMOSYS-compatible models in Phase 1–5. Phase 6 extends coverage based on demand. Maintain a clear list of supported/unsupported features. |
| **Performance regression in library mode.** Adding the override layer or changing ownership semantics may slow down the single-solve path. | Low | Medium | Benchmark the library path against the CLI path on every PR. The existing benchmark suite covers models from 15k to 12M non-zeros. |

---

## Summary

This next phase of `mosox` development addresses a clear gap in the energy modelling ecosystem: the absence of a fast, memory-efficient, scriptable library for systematic LP/MILP exploration — one that works with existing GMPL models rather than requiring them to be rewritten.

By building on an already-proven parser and solver integration, the development risk is low and the performance baseline is high. The principal engineering challenges are well-understood: adapting Rust ownership semantics for model reuse, bridging Rust/Python type systems for multi-dimensional parameter data, and ensuring cross-platform binary distribution.

The resulting Python library will enable researchers to run sensitivity analyses, scenario sweeps, and ensemble studies that are today impractical — either because they are too slow, too memory-intensive, or require too much manual orchestration. For the OSeMOSYS community specifically, it provides a path from manual file-based workflows to programmatic, reproducible, large-scale model exploration — without leaving the GMPL modelling language behind.
