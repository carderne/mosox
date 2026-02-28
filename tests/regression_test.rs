use assert_cmd::prelude::*;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Command;

use mosox::normalize::{compare_mps, normalize_mps};

const EPSILON: f64 = 0.02;

#[test]
fn regression_examples() {
    let examples_dir = PathBuf::from("examples");
    let mut failures: Vec<String> = Vec::new();

    let mut entries: Vec<_> = fs::read_dir(&examples_dir)
        .expect("failed to read examples/")
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let dir_name = entry.file_name().to_string_lossy().to_string();

        if dir_name == "osemosys_large" && std::env::var("MOSOX_TEST_LARGE").is_err() {
            continue;
        }

        let subdir = entry.path();

        // Find expected .mps file
        let mps_file = find_file_by_ext(&subdir, "mps");
        let mps_file = match mps_file {
            Some(f) => f,
            None => continue,
        };

        // Find .mod and optional .dat
        let mod_file = find_file_by_ext(&subdir, "mod").unwrap_or_else(|| {
            panic!("no .mod file found in {}", subdir.display());
        });
        let dat_file = find_file_by_ext(&subdir, "dat");

        // Run mosox compile
        let temp_dir = std::env::temp_dir().join("mosox_regression_test");
        fs::create_dir_all(&temp_dir).unwrap();
        let raw_output = temp_dir.join(format!("{}_raw.mps", dir_name));

        let mut cmd = Command::cargo_bin("mosox").unwrap();
        cmd.arg("compile").arg(&mod_file);
        if let Some(ref dat) = dat_file {
            cmd.arg(dat);
        }
        cmd.arg("-o").arg(&raw_output);
        let output = cmd.output().expect("failed to run mosox");

        if !output.status.success() {
            failures.push(format!(
                "{}: mosox compile failed:\n{}",
                dir_name,
                String::from_utf8_lossy(&output.stderr)
            ));
            continue;
        }

        // Normalize the compiled output
        let normalized_output = temp_dir.join(format!("{}_normalized.mps", dir_name));

        let raw_reader = BufReader::new(File::open(&raw_output).unwrap());
        let writer = File::create(&normalized_output).unwrap();
        normalize_mps(raw_reader, writer);

        // Compare expected vs normalized output with epsilon tolerance
        let expected_reader = BufReader::new(File::open(&mps_file).unwrap());
        let actual_reader = BufReader::new(File::open(&normalized_output).unwrap());
        let diffs = compare_mps(expected_reader, actual_reader, EPSILON);

        if !diffs.is_empty() {
            let shown: String = if diffs.len() > 20 {
                let first_20 = diffs[..20].join("\n  ");
                format!("  {}\n  ... and {} more", first_20, diffs.len() - 20)
            } else {
                format!("  {}", diffs.join("\n  "))
            };
            failures.push(format!(
                "{}: {} differences:\n{}",
                dir_name,
                diffs.len(),
                shown
            ));
        }

        // Clean up
        let _ = fs::remove_file(&raw_output);
        let _ = fs::remove_file(&normalized_output);
    }

    if !failures.is_empty() {
        panic!(
            "\n{} regression failure(s):\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}

fn find_file_by_ext(dir: &PathBuf, ext: &str) -> Option<PathBuf> {
    fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().map(|e| e == ext).unwrap_or(false))
}
