use assert_cmd::prelude::*;
use std::process::{Command, Output};

const MODEL: &str = "tests/fixtures/validate/model.mod";
const DATA: &str = "tests/fixtures/validate/data.dat";

/// Compile the fixture with `from` replaced by `to` in the data file.
fn compile_with(name: &str, from: &str, to: &str, extra_args: &[&str]) -> Output {
    let data = std::fs::read_to_string(DATA).unwrap();
    assert!(data.contains(from), "fixture has no {from:?}");
    let path = std::env::temp_dir().join(format!("mosox_validate_{name}.dat"));
    std::fs::write(&path, data.replace(from, to)).unwrap();
    let output = Command::cargo_bin("mosox")
        .unwrap()
        .args(["compile", MODEL])
        .arg(&path)
        .args(extra_args)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&path);
    output
}

#[test]
fn valid_data_passes() {
    let output = compile_with("valid", "", "", &[]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn violations_are_reported() {
    let cases = [
        (
            "n := a 1 b 2",
            "n := a 1.5 b 2",
            "param n (line 8): n[a] = 1.5 is not integer",
        ),
        ("n := a 1 b 2", "n := a -1 b 2", "n[a] = -1 violates >= 0"),
        ("n := a 1 b 2", "n := a 11 b 2", "n[a] = 11 violates <= 10"),
        ("n := a 1 b 2", "n := a x b 2", "n[a] = x is not numeric"),
        (
            "n := a 1 b 2",
            "n := a 1 b 2 c 3",
            "n[c] is outside its domain",
        ),
        (
            "b := a 0 b 1",
            "b := a 2 b 1",
            "param b (line 9): b[a] = 2 is not binary",
        ),
        (
            "lim := a 1 b 0",
            "lim := a 2 b 0",
            "lim[a] = 2 violates <= 1",
        ),
        (
            "name := a p b q",
            "name := a z b q",
            "name[a] = z is not in its `in` set",
        ),
        (
            "[a,*] p 1",
            "[a,*] q 1",
            "param r (line 12): r[a,q] is outside its domain",
        ),
        (
            "lim := a 1 b 0",
            "lim := a 1 b 2",
            "check (line 19) failed[b]",
        ),
        (
            "S[a] := p",
            "S[a] := z",
            "set S (line 4): S[a] element (z) is not within",
        ),
        ("S[a] := p", "S[c] := p", "S[c] is outside its domain"),
        (
            "(a,p) (b,q)",
            "(a,p) (p,a)",
            "set P (line 3): P element (p,a) is not within",
        ),
        (
            "(a,p) (b,q)",
            "(a,p,q)",
            "P element (a,p,q) has dimension 3, expected 2",
        ),
        (
            "T := (a,p)",
            "T := (a,z)",
            "set T (line 5): T element (a,z) is not within",
        ),
    ];
    for (i, (from, to, expected)) in cases.into_iter().enumerate() {
        let output = compile_with(&format!("case{i}"), from, to, &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{to:?} should fail");
        assert!(
            stderr.contains(expected),
            "{to:?}: expected {expected:?} in:\n{stderr}"
        );
    }
}

#[test]
fn errors_are_collected() {
    let output = compile_with("many", "n := a 1 b 2", "n := a 1.5 b 2.5", &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("2 validation error(s)"), "{stderr}");
}

#[test]
fn no_check_skips_validation() {
    let output = compile_with("no_check", "b := a 0 b 1", "b := a 2 b 1", &["--no-check"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
