use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "grammars/gmpl.pest"]
pub struct ModelParser;
