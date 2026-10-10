use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn run_load() {
    let mut cmd = Command::cargo_bin("mosox").unwrap();
    cmd.arg("compile")
        .arg("examples/osemosys_small/osemosys.mod");
    cmd.assert().success();
}

#[test]
fn run_bad_file() {
    let mut cmd = Command::cargo_bin("mosox").unwrap();
    cmd.arg("balance").arg("doesntexist.mod");
    cmd.assert().failure();
}

fn compile_prune_example(extra_args: &[&str]) -> String {
    let out = std::env::temp_dir().join(format!("mosox_prune_{}.mps", extra_args.len()));
    let mut cmd = Command::cargo_bin("mosox").unwrap();
    cmd.arg("compile")
        .arg("examples/prune/model.mod")
        .arg("-o")
        .arg(&out)
        .args(extra_args);
    cmd.assert().success();
    let mps = std::fs::read_to_string(&out).unwrap();
    let _ = std::fs::remove_file(&out);
    mps
}

#[test]
fn prune_zero_vars() {
    let mps = compile_prune_example(&[]);
    assert!(!mps.contains(" y "));
    assert!(!mps.contains("x[b]"));
    assert!(mps.contains("x[a]"));
}

#[test]
fn no_prune_keeps_zero_vars() {
    let mps = compile_prune_example(&["--no-prune"]);
    assert!(mps.contains(" y "));
    assert!(mps.contains("x[b]"));
}
