# Instructions

## Task summary
Write Rust structs and `from_entry` methods to convert Pest-parsed Pair<Rule> into the structs.

## Overview
1. This repo is a rust lib that uses pest to parse gnu mathprog gmpl model (.mod) and data (.dat) files.
2. The grammar is at grammar.pest.
3. The data structs that are the target IR for the parsed data will go in src/data.rs (currently there are just one or two).
4. I want you to complete src/data.rs with structs and from_entry methods for all the root rules in the grammar (VAR, PARAM, SET, OBJECTIVE, CONSTRAINT, DATA_SET, DATA_PARAM) (note several are ignored) and all the child and leaf structs that are needed.
5. Please use Enums as needed for when children can be one of several different structs.
6. Please also look at example.rs to see the two main ways of loading data from Pair<Rule> into the struct and follow the instructions in that file.
7. Please also look at tree.txt which will give you an idea of the tree structure. The struct structure should reflect this.
8. Add display methods also that pretty print the struct (recursively through children/leafs). Should be concise but clear, something like this (taking cues from tree.txt) but I'll leave you to make it consistent.
```
Constraint{
  name: "Foo",
  domain: Domain{
    part: { r in REGION }
    part: { t in TECHNOLOGY }
    condition: Expr{
      Factor[r,t,y] < 1
    }
  }
}
```


## Completion
When you have completed src/data.rs, you should update src/loader.rs the function consume with the appropriate structs.

Then you can do the following to check your work:
1. `cargo make fmt`
2. `cargo check`
3. `cargo make lint`
4. `cargo make test`
5. Run `cargo run check examples/osemosys.mod examples/atlantis.dat`. This will print out the final structs using your code. You should check that these are consistent with the input files.

## Strategy
Please use subagents if needed to manage context. Please keep going until the 5 checks above are completed, do not ask for input unless absolutely needed.


