use anyhow::{Context, Result};
use pest::Parser;
use pest::error::LineColLocation;
use pest::iterators::Pairs;

use crate::{
    gmpl::grammar::{ModelParser, Rule},
    ir::{self, Entry},
};

/// Parse the text using Pest
pub fn parse(data: &str) -> Result<Pairs<'_, Rule>> {
    let mut entries = ModelParser::parse(Rule::root, data).map_err(|e| {
        let (line, col) = match e.line_col {
            LineColLocation::Pos((l, c)) => (l, c),
            LineColLocation::Span((l, c), _) => (l, c),
        };
        anyhow::anyhow!("Syntax error at line {}, column {}:\n{}", line, col, e)
    })?;

    let entry = entries
        .next()
        .context("File did not even contain an EOI, didn't think this was possible.")?;
    Ok(entry.into_inner())
}

/// Convert the AST Pest Pairs into a IR
pub fn consume(entries: Pairs<'_, Rule>) -> Vec<Entry> {
    entries
        .into_iter()
        .filter_map(|entry| match entry.as_rule() {
            Rule::VAR => Some(Entry::Var(ir::Var::from_entry(entry))),
            Rule::PARAM => Some(Entry::Param(ir::Param::from_entry(entry))),
            Rule::SET => Some(Entry::Set(Box::new(ir::Set::from_entry(entry)))),
            Rule::OBJECTIVE => Some(Entry::Objective(ir::Objective::from_entry(entry))),
            Rule::CONSTRAINT => Some(Entry::Constraint(ir::Constraint::from_entry(entry))),
            Rule::SET_DATA => Some(Entry::DataSet(ir::SetData::from_entry(entry))),
            Rule::PARAM_DATA => Some(Entry::DataParam(ir::ParamData::from_entry(entry))),
            Rule::CHECK => Some(Entry::Check(ir::Check::from_entry(entry))),
            Rule::END
            | Rule::EOI
            | Rule::PRINT
            | Rule::DISPLAY
            | Rule::SOLVE
            | Rule::FOR
            | Rule::TABLE
            | Rule::COMMENT => None,
            _ => {
                let (line, _) = entry.line_col();
                unreachable!(
                    "unexpected: {line} rule: {:?}\ntext: {}",
                    entry.as_rule(),
                    entry.as_str()
                );
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse() {
        let text = r#"set YEAR;"#;
        let _entries = parse(&text);
    }

    #[test]
    #[should_panic]
    fn test_bad_consume() {
        let text = r#"
            INVALID MODEL STUFF
        "#;
        let entries = parse(&text).unwrap();
        assert!(entries.len() == 1);
    }

    #[test]
    fn test_consume() {
        let text = r#"
            param DiscountRate{r in REGION};
        "#;
        let entries = parse(&text).unwrap();
        consume(entries);
    }
}
