use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn run_load() {
    let mut cmd = Command::cargo_bin("mosox").unwrap();
    cmd.arg("check").arg("examples/osemosys.mod");
    cmd.assert().success();
}

#[test]
fn run_bad_file() {
    let mut cmd = Command::cargo_bin("mosox").unwrap();
    cmd.arg("balance").arg("doesntexist.mod");
    cmd.assert().failure();
}
