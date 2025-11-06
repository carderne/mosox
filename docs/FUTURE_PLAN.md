# Development Plan: Phases 2-3

## Work Completed
Phase 1 implemented a complete GMPL parser that converts model and data files into a strongly-typed intermediate representation in Rust. The parser successfully handles complex real-world models like OSeMOSYS and is published on crates.io with continuous integration.

## Phase 2: MPS Conversion Foundation
**Timeline: November 2025 - January 2026**

Phase 2 focuses on implementing MPS output capabilities for a subset of GMPL constructs. The goal is to convert simple models and a meaningful subset of OSeMOSYS to valid MPS format that can be solved by standard solvers.

### Open Questions
- How should multi-dimensional GMPL domains be flattened into MPS row and column names while staying within the 8 character naming limit?
- What subset of OSeMOSYS constraints can be handled without full expression evaluation, and which require deferred implementation?
- Should the MPS generator support extended MPS features (like quadratic programming extensions) or stick to the classical fixed-format specification?

### Key Tasks
- Research MPS format specification and create a technical writeup mapping GMPL constructs through the IR to MPS output, addressing fixed-column format requirements, naming conventions (8 character limit), and semantic translation challenges
- Implement MPS generator for simple linear programs with basic constraints and objectives, validating that generated files can be read and solved by GLPK, CPLEX, or Gurobi
- Add support for interpreting GMPL aggregation functions (sum, min, max) and arithmetic operations, generating correct MPS constraint rows with appropriate coefficients
- Convert a subset of OSeMOSYS to MPS, focusing on commonly used patterns in energy system models while deferring more complex domain expressions and conditional constraints to Phase 3

## Phase 3: Complete Implementation and Validation
**Timeline: February 2026 - April 2026**

Phase 3 completes the MPTK implementation and establishes confidence in correctness through comprehensive testing. The MOSAIC MVP milestone in March 2026 will deliver a production-ready tool with validated correctness and documented performance characteristics.

### Open Questions
- What numerical tolerance thresholds are acceptable when comparing GLPK and MPS solver results, given potential differences in solver algorithms and floating-point arithmetic?
- Should the random model generator target specific GMPL patterns found in OSeMOSYS, or generate a broader range of syntactically valid models?
- How should performance benchmarks be structured to provide meaningful comparisons between GMPL and MPS approaches for different model characteristics?
- What are the most valuable LLM integration use cases for the MOSAIC ecosystem, and which can be realistically prototyped within the Phase 3 timeline?

### Key Tasks
- Complete full OSeMOSYS conversion to MPS, extending the generator to handle all GMPL constructs including complex domain expressions, conditional constraints, and multi-dimensional parameter table integration
- Build test infrastructure that compares GLPK solutions of original GMPL models against solver results from translated MPS files, verifying that objective values and variable solutions match within acceptable numerical tolerances
- Implement performance benchmarks measuring parsing time, translation time, and end-to-end solve time across various OSeMOSYS data files with different scales and complexities
- Explore random model generator for additional test coverage, creating valid GMPL models with known properties to identify edge cases and subtle translation errors that fixed test cases might miss
- Optional: develop LLM tool integration for quick model validation and CSV-to-data file generation with immediate validity checking
