use assert_cmd::prelude::*;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct BenchCase {
    name: &'static str,
    mod_file: &'static str,
    dat_file: Option<&'static str>,
    iterations: usize,
    needs_large: bool,
}

const CASES: &[BenchCase] = &[
    BenchCase {
        name: "basic",
        mod_file: "examples/basic/model.mod",
        dat_file: None,
        iterations: 200,
        needs_large: false,
    },
    BenchCase {
        name: "2d_params",
        mod_file: "examples/2d_params/model.mod",
        dat_file: None,
        iterations: 200,
        needs_large: false,
    },
    BenchCase {
        name: "sets",
        mod_file: "examples/sets/model.mod",
        dat_file: None,
        iterations: 200,
        needs_large: false,
    },
    BenchCase {
        name: "osemosys_small",
        mod_file: "examples/osemosys_small/osemosys.mod",
        dat_file: Some("examples/osemosys_small/ose_small.dat"),
        iterations: 50,
        needs_large: false,
    },
    BenchCase {
        name: "osemosys_atlantis",
        mod_file: "examples/osemosys_atlantis/osemosys.mod",
        dat_file: Some("examples/osemosys_atlantis/atlantis.dat"),
        iterations: 10,
        needs_large: false,
    },
    BenchCase {
        name: "osemosys_large",
        mod_file: "examples/osemosys_large/model.v.5.3.mod",
        dat_file: Some("examples/osemosys_large/turkey.dat"),
        iterations: 4,
        needs_large: true,
    },
];

fn fmt_duration(d: Duration) -> String {
    let micros = d.as_micros();
    if micros < 1_000 {
        format!("{}µs", micros)
    } else if micros < 1_000_000 {
        format!("{:.1}ms", micros as f64 / 1_000.0)
    } else {
        format!("{:.1}s", micros as f64 / 1_000_000.0)
    }
}

fn run_mosox(case: &BenchCase) -> Duration {
    let start = Instant::now();
    let mut cmd = Command::cargo_bin("mosox").unwrap();
    cmd.arg("generate").arg(case.mod_file);
    if let Some(dat) = case.dat_file {
        cmd.arg(dat);
    }
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    let status = cmd.status().expect("failed to run mosox");
    let elapsed = start.elapsed();
    assert!(status.success(), "mosox failed for {}", case.name);
    elapsed
}

fn run_glpsol(case: &BenchCase) -> Duration {
    let tmp = std::env::temp_dir().join(format!("mosox_bench_{}.mps", case.name));
    let start = Instant::now();
    let mut cmd = Command::new("glpsol");
    cmd.arg("--model").arg(case.mod_file);
    if let Some(dat) = case.dat_file {
        cmd.arg("--data").arg(dat);
    }
    cmd.arg("--wfreemps")
        .arg(&tmp)
        .arg("--check")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = cmd.status().expect("failed to run glpsol");
    let elapsed = start.elapsed();
    assert!(status.success(), "glpsol failed for {}", case.name);
    let _ = std::fs::remove_file(&tmp);
    elapsed
}

struct Stats {
    min: Duration,
    median: Duration,
    max: Duration,
}

fn compute_stats(durations: &mut Vec<Duration>) -> Stats {
    durations.sort();
    let n = durations.len();
    let median = if n % 2 == 1 {
        durations[n / 2]
    } else {
        (durations[n / 2 - 1] + durations[n / 2]) / 2
    };
    Stats {
        min: durations[0],
        median,
        max: durations[n - 1],
    }
}

#[test]
#[ignore]
fn bench_generate() {
    let run_large = std::env::var("MOSOX_TEST_LARGE").is_ok();
    let bench_glpsol = std::env::var("MOSOX_BENCH_GLPSOL").is_ok();

    struct Result {
        name: &'static str,
        iterations: usize,
        mosox: Stats,
        glpsol: Option<Stats>,
    }

    let mut results: Vec<Result> = Vec::new();

    for case in CASES {
        if case.needs_large && !run_large {
            continue;
        }

        // Warmup
        run_mosox(case);

        let mut durations: Vec<Duration> = (0..case.iterations).map(|_| run_mosox(case)).collect();
        let mosox_stats = compute_stats(&mut durations);

        let glpsol_stats = if bench_glpsol {
            // Warmup
            run_glpsol(case);
            let mut durations: Vec<Duration> =
                (0..case.iterations).map(|_| run_glpsol(case)).collect();
            Some(compute_stats(&mut durations))
        } else {
            None
        };

        results.push(Result {
            name: case.name,
            iterations: case.iterations,
            mosox: mosox_stats,
            glpsol: glpsol_stats,
        });
    }

    // Print human-readable table to stdout
    println!();
    if bench_glpsol {
        println!(
            "{:<22} {:>5}  {:>10} {:>10} {:>10}  {:>10} {:>10} {:>10}  {:>7}",
            "Example", "N", "Min", "Median", "Max", "gMin", "gMedian", "gMax", "Speedup"
        );
        println!("{}", "-".repeat(105));
        for r in &results {
            let g = r.glpsol.as_ref().unwrap();
            let speedup = g.median.as_secs_f64() / r.mosox.median.as_secs_f64();
            println!(
                "{:<22} {:>5}  {:>10} {:>10} {:>10}  {:>10} {:>10} {:>10}  {:>6.1}x",
                r.name,
                r.iterations,
                fmt_duration(r.mosox.min),
                fmt_duration(r.mosox.median),
                fmt_duration(r.mosox.max),
                fmt_duration(g.min),
                fmt_duration(g.median),
                fmt_duration(g.max),
                speedup,
            );
        }
    } else {
        println!(
            "{:<22} {:>5}  {:>10} {:>10} {:>10}",
            "Example", "N", "Min", "Median", "Max"
        );
        println!("{}", "-".repeat(65));
        for r in &results {
            println!(
                "{:<22} {:>5}  {:>10} {:>10} {:>10}",
                r.name,
                r.iterations,
                fmt_duration(r.mosox.min),
                fmt_duration(r.mosox.median),
                fmt_duration(r.mosox.max),
            );
        }
    }
    println!();

    // Print markdown table to stderr for README copy-paste
    if bench_glpsol {
        eprintln!("| Example | N | mosox | glpsol | Speedup |");
        eprintln!("|---------|---|-------|--------|---------|");
        for r in &results {
            let g = r.glpsol.as_ref().unwrap();
            let speedup = g.median.as_secs_f64() / r.mosox.median.as_secs_f64();
            eprintln!(
                "| {} | {} | {} | {} | {:.1}x |",
                r.name,
                r.iterations,
                fmt_duration(r.mosox.median),
                fmt_duration(g.median),
                speedup,
            );
        }
    } else {
        eprintln!("| Example | N | Median | Min | Max |");
        eprintln!("|---------|---|--------|-----|-----|");
        for r in &results {
            eprintln!(
                "| {} | {} | {} | {} | {} |",
                r.name,
                r.iterations,
                fmt_duration(r.mosox.median),
                fmt_duration(r.mosox.min),
                fmt_duration(r.mosox.max),
            );
        }
    }
}
