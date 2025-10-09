use log::debug;
use pest::Parser;
use pest::iterators::Pairs;

use crate::{
    data::{self, Entry},
    grammar::{ModelParser, Rule},
};

/// Parse the text using Pest
pub fn parse(data: &str) -> Pairs<'_, Rule> {
    // TODO handle error
    let mut entries = ModelParser::parse(Rule::root, data).unwrap();

    // There will always be at least an "EOI", so this will not error
    let entry = entries.next().unwrap();
    entry.into_inner()
}

/// Convert the AST Pest Pairs into a IR
pub fn consume(entries: Pairs<'_, Rule>) -> Vec<Entry> {
    let mut dirs: Vec<Entry> = Vec::new();
    for entry in entries {
        match entry.as_rule() {
            // Model rules
            Rule::VAR => {
                debug!("VAR");
                dirs.push(Entry::Var(data::Var::from_entry(entry)));
            }
            Rule::PARAM => {
                debug!("PARAM");
                dirs.push(Entry::Param(data::Param::from_entry(entry)));
            }
            Rule::SET => {
                debug!("SET");
                dirs.push(Entry::Set(data::Set::from_entry(entry)));
            }
            Rule::OBJECTIVE => {
                debug!("OBJ");
                dirs.push(Entry::Objective(data::Objective::from_entry(entry)));
            }
            Rule::CONSTRAINT => {
                debug!("ST");
                dirs.push(Entry::Constraint(data::Constraint::from_entry(entry)));
            }

            // Data rules
            Rule::DATA_SET => {
                debug!("DATA:SET");
                dirs.push(Entry::DataSet(data::DataSet::from_entry(entry)));
            }
            Rule::DATA_PARAM => {
                debug!("DATA:PARAM");
                dirs.push(Entry::DataParam(data::DataParam::from_entry(entry)));
            }

            // Ignored
            Rule::END => {}
            Rule::EOI => {}
            Rule::PRINT => {}
            Rule::CHECK => {}
            Rule::SOLVE => {}
            Rule::FOR => {}
            Rule::TABLE => {}
            Rule::COMMENT => {}

            // Catch-all
            _ => {
                let (line, _) = entry.line_col();
                let rule = entry.as_rule();
                let text = entry.as_str();
                unreachable!("unexpected: {line} rule: {rule:?}\ntext: {text}");
            }
        };
    }
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse() {
        let text = r#"set YEAR;"#;
        let entries = parse(&text);
        dbg!(entries);
    }

    #[test]
    #[should_panic]
    fn test_bad_consume() {
        let text = r#"
            INVALID MODEL STUFF
        "#;
        let entries = parse(&text);
        assert!(entries.len() == 1);
    }

    #[test]
    fn test_consume() {
        let text = r#"
            param DiscountRate{r in REGION};
        "#;
        let entries = parse(&text);
        consume(entries);
    }
}
