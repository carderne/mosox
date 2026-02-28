//! MPS file normalizer for deterministic diffing in regression tests.

use std::io::{BufRead, Write};

const SECTION_HEADERS: &[&str] = &[
    "NAME", "ROWS", "COLUMNS", "RHS", "RANGES", "BOUNDS", "ENDATA",
];

fn is_section_header(word: &str) -> bool {
    SECTION_HEADERS.contains(&word)
}

fn normalize_whitespace(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_columns_line(normalized: &str) -> Vec<String> {
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    match parts.len() {
        5 => vec![
            format!("{} {} {}", parts[0], parts[1], parts[2]),
            format!("{} {} {}", parts[0], parts[3], parts[4]),
        ],
        _ => vec![normalized.to_string()],
    }
}

fn split_rhs_line(normalized: &str) -> Vec<String> {
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    match parts.len() {
        5 => vec![
            format!("{} {} {}", parts[0], parts[1], parts[2]),
            format!("{} {} {}", parts[0], parts[3], parts[4]),
        ],
        _ => vec![normalized.to_string()],
    }
}

fn is_marker_line(normalized: &str) -> bool {
    normalized.contains("'MARKER'")
}

fn flush_section(
    section_lines: &mut Vec<String>,
    integer_lines: &mut Vec<String>,
    current_section: Option<&str>,
    writer: &mut impl Write,
) {
    section_lines.sort();
    for sl in section_lines.iter() {
        writeln!(writer, "{sl}").unwrap();
    }
    section_lines.clear();

    if current_section == Some("COLUMNS") && !integer_lines.is_empty() {
        writeln!(writer, "M0000001 'MARKER' 'INTORG'").unwrap();
        integer_lines.sort();
        for sl in integer_lines.iter() {
            writeln!(writer, "{sl}").unwrap();
        }
        writeln!(writer, "M0000001 'MARKER' 'INTEND'").unwrap();
        integer_lines.clear();
    }
}

/// Normalize an MPS file for deterministic comparison.
///
/// Strips comments, normalizes whitespace, splits packed column/RHS entries,
/// and sorts lines within each section. Does not round numeric values — epsilon
/// comparison should be used when comparing normalized files.
/// Primarily used to produce stable output for regression test diffing.
pub fn normalize_mps(reader: impl BufRead, mut writer: impl Write) {
    let mut current_section: Option<&str> = None;
    let mut section_lines: Vec<String> = Vec::new();
    let mut integer_lines: Vec<String> = Vec::new();
    let mut in_integer_block = false;

    for line in reader.lines() {
        let line = line.expect("failed to read line");
        let line = line.trim_end_matches(['\n', '\r']);

        if line.starts_with('*') {
            continue;
        }

        let stripped = line.trim();
        let first_word = stripped.split_whitespace().next().unwrap_or("");

        if is_section_header(first_word) {
            flush_section(
                &mut section_lines,
                &mut integer_lines,
                current_section,
                &mut writer,
            );

            current_section = SECTION_HEADERS.iter().find(|&&h| h == first_word).copied();
            in_integer_block = false;
            writeln!(writer, "{}", normalize_whitespace(line)).unwrap();
        } else {
            let normalized = normalize_whitespace(line);
            if normalized.is_empty() {
                continue;
            }

            if current_section == Some("COLUMNS") && is_marker_line(&normalized) {
                in_integer_block = normalized.contains("'INTORG'");
                continue;
            }

            match current_section {
                Some("COLUMNS") if in_integer_block => {
                    integer_lines.extend(split_columns_line(&normalized));
                }
                Some("COLUMNS") => section_lines.extend(split_columns_line(&normalized)),
                Some("RHS") => section_lines.extend(split_rhs_line(&normalized)),
                _ => section_lines.push(normalized),
            }
        }
    }

    // Flush final section
    flush_section(
        &mut section_lines,
        &mut integer_lines,
        current_section,
        &mut writer,
    );
}

/// Compare two normalized MPS files line-by-line with epsilon tolerance for
/// numeric values. Returns a list of difference descriptions (empty if files match).
pub fn compare_mps(expected: impl BufRead, actual: impl BufRead, epsilon: f64) -> Vec<String> {
    let expected_lines: Vec<String> = expected.lines().map(|l| l.unwrap()).collect();
    let actual_lines: Vec<String> = actual.lines().map(|l| l.unwrap()).collect();

    let mut diffs = Vec::new();

    if expected_lines.len() != actual_lines.len() {
        diffs.push(format!(
            "line count differs: expected {} vs actual {}",
            expected_lines.len(),
            actual_lines.len()
        ));
    }

    let max_lines = expected_lines.len().max(actual_lines.len());
    for i in 0..max_lines {
        match (expected_lines.get(i), actual_lines.get(i)) {
            (Some(exp), Some(act)) => {
                if !tokens_match(exp, act, epsilon) {
                    diffs.push(format!("line {}: expected '{}' got '{}'", i + 1, exp, act));
                }
            }
            (Some(exp), None) => {
                diffs.push(format!(
                    "line {}: expected '{}' but actual file ended",
                    i + 1,
                    exp
                ));
            }
            (None, Some(act)) => {
                diffs.push(format!("line {}: unexpected extra line '{}'", i + 1, act));
            }
            (None, None) => unreachable!(),
        }
    }

    diffs
}

fn tokens_match(expected: &str, actual: &str, epsilon: f64) -> bool {
    let exp_tokens: Vec<&str> = expected.split_whitespace().collect();
    let act_tokens: Vec<&str> = actual.split_whitespace().collect();

    if exp_tokens.len() != act_tokens.len() {
        return false;
    }

    for (e, a) in exp_tokens.iter().zip(act_tokens.iter()) {
        if e == a {
            continue;
        }
        match (e.parse::<f64>(), a.parse::<f64>()) {
            (Ok(ev), Ok(av)) => {
                if (ev - av).abs() > epsilon {
                    return false;
                }
            }
            _ => return false,
        }
    }

    true
}
