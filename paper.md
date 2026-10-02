---
title: 'MOSOX: a fast model-to-matrix compiler for linear and mixed-integer optimisation'
tags:
- optimisation
- linear programming
- mixed-integer programming
- GMPL
- MathProg
- Rust
- energy systems
authors:
- name: Mark Howells
  affiliation: 1, 2
  orcid: 0000-0001-6419-4957
- name: Lara Dixon
  affiliation: '3'
  corresponding: true
  email: 'lara.dixon22@imperial.ac.uk'
- name: Vedran Kapor
  affiliation: '4'
- name: Iain Staffell
  affiliation: '1'
  orcid: 0000-0003-1012-7075
  corresponding: true
  email: 'i.staffell@imperial.ac.uk'
- name: Nathan Johnson
  affiliation: '1'
  orcid: 0000-0002-9521-4700
  corresponding: true
  email: 'nathan.johnson17@imperial.ac.uk'
- name: Mohamed Bassam Ben-Ticha
  affiliation: '4'
- name: Maelle Baronnet
  affiliation: '5'
- name: Claire Nicolas
  affiliation: '5'
- name: Nolwazi Khumalo
  affiliation: '6'
- name: Daniel Russo
  affiliation: '6'
- name: Larissa Pinheiro Pupo Nogueira
  affiliation: '6'
- name: Simon Benmarraze
  affiliation: '6'
- name: Adam Hawkes
  affiliation: '1'
  orcid: 0000-0001-9720-332X
- name: Steve Pye
  affiliation: '7'
  orcid: 0000-0003-1793-2552
- name: Tom Alfstad
  affiliation: '8'
- name: Mario Tot
  affiliation: '8'
- name: Leigh Martindale
  affiliation: 1, 2
  orcid: 0000-0001-7990-4923
- name: Jairo Quirós-Tortós
  affiliation: '2'
  orcid: 0000-0002-3329-8910
- name: Leonhard Hofbauer
  affiliation: '7'
  orcid: 0000-0002-0520-9984
- name: Pietro Lubello
  affiliation: '7'
  orcid: 0000-0002-8869-9586
- name: Naomi Tan
  affiliation: 1, 2
  orcid: 0000-0001-7957-8451
- name: Fernando Plazas-Niño
  affiliation: '8'
  orcid: 0000-0003-3392-5707
- name: Ariane Millot
  affiliation: '1'
  orcid: 0000-0002-7696-1014
- name: Francesco Gardumi
  affiliation: '9'
  orcid: 0000-0001-8371-9325
- name: Richard Alexander Roehrl
  affiliation: '10'
- name: Chris Arderne
  affiliation: '8'
  orcid: 0000-0002-7904-2216
affiliations:
- name: Imperial College London, United Kingdom
  index: 1
  ror: 041kmwe10
- name: Loughborough University, United Kingdom
  index: 2
  ror: 04vg4w365
- name: International Centre for Theoretical Physics, Italy
  index: 3
  ror: 009gyvm78
- name: International Atomic Energy Agency
  index: 4
  ror: 00gtfax65
- name: World Bank Group, United States
  index: 5
  ror: 02md09461 
- name: International Renewable Energy Agency
  index: 6
  ror: 01dej0523
- name: University College London, United Kingdom
  index: 7
  ror: 02jx3x895
- name: Climate Compatible Growth
  index: 8
- name: KTH Royal Institute of Technology, Sweden
  index: 9
  ror: 026vcq606
- name: United Nations
  index: 10
  ror: 006kxhp52
date: 1 October 2026
bibliography: paper.bib
---

# Summary

Linear and mixed-integer optimisation models help researchers ranging from economics to engineering study resource allocation, infrastructure planning, energy-system transitions, and other decision problems where multiple criteria affect the desirability of outcomes. Algebraic modelling languages let researchers describe these models close to their mathematical form, and use text-based model and data files that can be reviewed with limited coding experience and versioned to improve the transparency of the modelling process. As optimisation models increase in size and complexity, translating algebraic models into solver-ready sparse matrices becomes an increasingly important computational bottleneck that limits the scale of problems that can be solved.

MOSOX [@mosox_repo] is a Rust command-line tool and library that compiles a supported subset of GNU MathProg/GMPL model and data files into sparse matrices. It supports all GMPL constructs required by the OSeMOSYS model family while not implementing the full GMPL language. Unsupported constructs generate an error, apart from documented parsed-but-unenforced declarations and ignored output or control statements [@mosox_repo]. MOSOX expands indexed sets, parameters, variables, objectives, and constraints into matrices, then writes the resulting linear or mixed-integer programme in MPS (Mathematical Programming System) format and can solve the compiled model directly using the HiGHS optimisation solver [@huangfu_hall_2018; @highs_docs]. Benchmarking on the OSeMOSYS model family reports matrix compilation up to 6.5 times faster than GLPK's glpsol while reducing peak memory use on the largest benchmarked model [@mosox_repo].

MOSOX combines faster matrix compilation with transparent algebraic models and solver-independent output, supporting reproducible and automated optimisation workflows while preserving human review of equations, assumptions and input data [@mathprog_manual; @sages_2026].

# Statement of need

Optimisation projects often benefit from algebraic model files that remain close to the underlying mathematics, avoiding the need to embed models in Python, Julia or another general-purpose programming language. The most established open-source framework is GNU MathProg (GMPL), implemented in GLPK's glpsol, which includes a GNU MathProg translator and a stand-alone LP/MIP solver [@glpk_gnu; @mathprog_manual]. MOSOX is designed for researchers working with large GMPL models or conducting large Monte Carlo or sensitivity analyses where compilation efficiency is important.

MOSOX is a stripped-down, high-performance compiler written in Rust. It prioritises:

* Rapid matrix generation for large optimisation models;
* Accessibility through the simple, human-readable syntax of GNU MathProg; and
* Auditability through transparent matrix generation and deterministic regression testing.

This design supports transparent optimisation workflows because model structure remains visible throughout compilation. Participatory energy-modelling research shows that democratising energy planning depends not only on model access, but also on processes that allow stakeholders to understand, challenge and shape model assumptions [@mcgookin_2021]. IMPACCT similarly argues that accessible, modular and open-source modelling frameworks help reduce exclusion in developing-country energy planning and support cross-disciplinary collaboration [@impacct_2025]. GMPL-style model files keep sets, parameters, variables, objectives and constraints close to their mathematical formulation, rather than requiring users to interpret model logic through a host-language API [@mathprog_manual; @mosox_repo]. These characteristics also support automated optimisation workflows, including AI-assisted model development, while allowing analysts to inspect the algebra, input data and solver-ready outputs before results inform policy or investment decisions [@sages_2026; @mosox_repo].

The compile command writes standard MPS files, allowing generated matrices to be archived, inspected or passed to external solvers. The solve command provides direct integration with HiGHS, allowing models to be solved from the command line without writing intermediate files. The normalize and compare commands perform deterministic regression tests against reference outputs generated by GLPK's glpsol [@mosox_repo].

MOSOX is developed within the Climate Compatible Growth (CCG) programme with international partners including the International Renewable Energy Agency (IRENA), International Atomic Energy Agency (IAEA) and World Bank Group (WBG) through the MOSAIC initiative. CCG's open-source transition modelling ecosystem supports energy and resource planning in low- and middle-income countries [@ccg_ecosystem]. The OSeMOSYS energy systems model family provides MOSOX's primary validation domain because of its widespread use in long-term energy planning and the computational demands of large-scale optimisation models [@osemosys_home; @osemosys_global_2022].

# State of the field

MOSOX occupies a distinct position between full algebraic modelling systems and host-language optimisation libraries. Its closest comparator is GLPK's glpsol, which provides both a GNU MathProg (GMPL) translator and a stand-alone LP/MIP solver [@glpk_gnu]. Like glpsol, MOSOX compiles GMPL models into solver-ready sparse matrices, but focuses on high-performance matrix generation, MPS export, optional integration with the HiGHS solver, and deterministic regression testing [@mosox_repo; @highs_docs].

Broader optimisation frameworks provide richer modelling environments. AMPL and GAMS are mature algebraic modelling systems with extensive solver ecosystems [@ampl_book; @gams_official]. Pyomo, JuMP, Linopy and CVXPY embed optimisation models within Python or Julia, supporting a broad range of mathematical programming problems while integrating naturally with host-language software ecosystems [@pyomo_paper; @jump_paper; @linopy_joss; @cvxpy_jmlr]. For example, Linopy represents optimisation variables as labelled arrays before assembling the corresponding sparse matrix representation [@linopy_joss].

These tools solve similar optimisation problems but make different design choices. AMPL and GAMS provide comprehensive modelling environments, while Pyomo, JuMP, Linopy and CVXPY prioritise flexibility through host-language APIs. MOSOX instead preserves the equation-first style of GNU MathProg while providing a lightweight compiler focused on rapid matrix generation, interoperability through standard MPS export, deterministic regression testing, and optional direct integration with HiGHS. Rather than replacing broader optimisation frameworks, MOSOX provides a specialised workflow for GMPL-family linear and mixed-integer models where transparency, reproducibility and interoperability are priorities [@mosox_repo; @mathprog_manual].

# Software design

MOSOX follows the architecture of parser to matrix to output. The command-line interface features compile, solve, normalize, and compare subcommands. Internally, model and optional data files are parsed into a representation of GMPL constructs and merged into a common data structure. This is expanded into sets, parameters, variables, objectives, and constraints, and then converted into a sparse matrix representation. The MPS writer serialises this compiled matrix to a solver-independent file, or the HiGHS integration solves the same matrix directly in-memory, avoiding the need to write intermediate files. The normalize and compare commands perform deterministic regression tests by comparing MOSOX-generated MPS files against reference outputs generated by GLPK's glpsol [@mosox_repo].

The implementation uses Rust, the pest parser generator, string interning, sparse internal maps, and parallel constraint expansion with rayon. These choices improve compiler performance while preserving the user-facing GMPL workflow. The current benchmark code compares MOSOX and glpsol on OSeMOSYS examples, reporting a factor of 6.5 speed-up on the osemosys\_large model (approximately one million rows and five million columns), reducing compilation time from 130 to 20 seconds and peak memory use by 11% (from 5.6 to 5.0 GB). Benchmark scripts and methodology are available in the project repository [@mosox_repo].

The code below gives an example from “Advanced Problems and Elegant Solutions”, a set covering problem using binary decision variables to choose five players from a team of seven to maximize defensive ability. This model would be solved through the command: `mosox solve example.mod > solution.txt`

**example.mod:**
```
/* Basketball Lineup Model */
set PLAYERS := 1..7;
/* Abilities: Assisting, Throwing, Rebounding, Defense */
param A{PLAYERS}; param T{PLAYERS}; param R{PLAYERS}; param D{PLAYERS};
/* Decision variable: 1 if player plays, 0 otherwise */
var y{i in PLAYERS} binary;
/* Objective: Maximize total defensive ability */
maximize total_defense: sum{i in PLAYERS} D[i] * y[i];
/* Constraints */
s.t. five_players: sum{i in PLAYERS} y[i] = 5;
/* Position constraints based on player capabilities */
s.t. backfield: y[1] + y[3] + y[5] + y[7] >= 3;
s.t. frontfield: y[4] + y[5] + y[6] + y[7] >= 2;
s.t. midfield: y[2] + y[3] + y[4] + y[6] >= 1;
/* Average ability constraints (Avg >= 2 across 5 players) */
s.t. avg_assist: sum{i in PLAYERS} A[i] * y[i] >= 10;
s.t. avg_throw: sum{i in PLAYERS} T[i] * y[i] >= 10;
s.t. avg_rebound: sum{i in PLAYERS} R[i] * y[i] >= 10;
/* Logical constraints */
s.t. exclusivity: y[3] + y[6] <= 1; /* If 3 plays, 6 cannot */
s.t. requirement_4: y[1] <= y[4]; /* If 1 plays, 4 must play */
s.t. requirement_5: y[1] <= y[5]; /* If 1 plays, 5 must play */
s.t. mandatory_choice: y[2] + y[3] >= 1; /* Either 2 or 3 must play */
data;
param A := 1 3 2 2 3 2 1 1 3 2 3 6 3 7 3;
param T := 1 3 2 1 3 3 4 3 5 3 6 1 7 2;
param R := 1 1 2 3 3 2 4 3 5 1 6 2 7 2;
param D := 1 3 2 2 3 2 4 1 5 2 6 3 7 1;
end;
```

# Research impact statement

MOSOX is available under the MIT licence and is part of the MOSAIC ecosystem of energy systems models.

The value of this work is clearest for teams that already maintain GMPL-family models and need to run them reproducibly in automated workflows. A fast command-line compiler supports continuous integration, regression testing, inspection of solver-ready matrices, and large batches of model variants without requiring models to be rewritten in another language. Its command-line interface, explicit inputs and outputs, and comparison tools make MOSOX well suited to scripted research workflows [@mosox_repo].

For energy systems research, development within Climate Compatible Growth (CCG) and the MOSAIC initiative, together with partners including IRENA, the IAEA and the World Bank Group, provides an institutional pathway for ongoing development and application. At this stage, the software's demonstrated impact is based on benchmarked performance, deterministic regression testing and integration within the MOSAIC ecosystem, rather than widespread external adoption [@mosox_repo].

MOSOX also has a resource-efficiency argument, subject to careful qualification [@mosox_repo]. The published benchmark reports up to a 6.5× matrix-generation speed-up over GLPK/glpsol on tested OSeMOSYS examples, including a large case of about 1 million rows and 5 million columns, and lower memory use for the matrix-generation stage in that case [@mosox_repo]. This is benchmark evidence for the tested GMPL/OSeMOSYS cases rather than proof of comparable speed-ups across all optimisation models. Where these gains hold, faster compilation on the same hardware reduces the computational resources required per run, which is a practical benefit for workflows that compile large models repeatedly in continuous integration or across scenario ensembles. For GMPL-family energy models, MOSOX therefore offers a transparent matrix generator in which benchmarked speed and auditability are the priority design criteria [@mosox_repo; @sages_2026].

# AI usage disclosure

Generative AI tools were used to assist in the preparation of this paper. OpenAI ChatGPT 5.5 and 6 were used to support paper proofreading and critique. For the MOSOX codebase, Anthropic Claude Opus version 4.5 was used for limited tasks such as code completion, refactoring suggestions, documentation drafting, test scaffolding and debugging support. AI tools did not determine the software architecture or any core design choices or implementation. The final acceptance of all code and text were made by the human authors.

# Acknowledgements

This work was supported by the Climate Compatible Growth (CCG) programme funded by the UK's Foreign, Commonwealth & Development Office (FCDO). The views expressed in this paper are those of the authors and do not necessarily reflect the official policies of the UK Government, the United Nations or its senior management.

The authors gratefully acknowledge Rodrigo Ceron for making GNU MathProg more accessible to the energy-modelling community through practical examples, guidance and discussion. His support for the development and early adoption of OSeMOSYS demonstrated how clear algebraic syntax can improve the accessibility, transparency and reviewability of optimisation models.

The authors also recognise Andrew Makhorin for developing and maintaining GLPK and GNU MathProg, which have provided a foundation for open-source linear and mixed-integer optimisation and have had a lasting influence on the energy systems modelling community. MOSOX builds on this contribution by extending the GNU MathProg workflow with a modern, high-performance compiler.

The authors further thank Seyram Apeti Gildas Siggini for contributions to the conceptual development of the MOSAIC initiative and community engagement activities and Abhishek Shivakumar for comparative testing of matrix-generation approaches.

# References
