//! Golden snapshot tests for the formatter.
//!
//! Each pair `tests/snapshots/<name>.input.ferriorm` /
//! `tests/snapshots/<name>.expected.ferriorm` is loaded; the input is
//! formatted and compared against the expected output. To regenerate the
//! expected output after an intentional change, run with
//! `FERRIORM_FMT_BLESS=1` and the harness will rewrite the `.expected.`
//! files in place.

use std::fs;
use std::path::PathBuf;

fn snapshots_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

#[test]
fn snapshots() {
    let bless = std::env::var("FERRIORM_FMT_BLESS").is_ok();
    let dir = snapshots_dir();
    let mut cases: Vec<(String, PathBuf, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&dir).expect("snapshots dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(stem) = name.strip_suffix(".input.ferriorm") {
            let expected = dir.join(format!("{stem}.expected.ferriorm"));
            cases.push((stem.to_string(), path.clone(), expected));
        }
    }
    cases.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!cases.is_empty(), "no snapshot cases discovered");

    let mut failures: Vec<String> = Vec::new();

    for (name, input_path, expected_path) in &cases {
        let input = fs::read_to_string(input_path).expect("read input");
        let formatted = match ferriorm_fmt::format_schema(&input) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("[{name}] parse error: {e}"));
                continue;
            }
        };

        if bless {
            fs::write(expected_path, &formatted).expect("write expected");
            continue;
        }

        let expected = fs::read_to_string(expected_path).unwrap_or_default();
        if formatted != expected {
            failures.push(format!(
                "[{name}] mismatch\n--- expected ---\n{expected}\n--- actual ---\n{formatted}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} snapshot failure(s):\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn idempotent() {
    let dir = snapshots_dir();
    for entry in fs::read_dir(&dir).expect("snapshots dir") {
        let entry = entry.expect("entry");
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.ends_with(".input.ferriorm") && !name.ends_with(".expected.ferriorm") {
            continue;
        }
        let src = fs::read_to_string(&path).expect("read");
        let once =
            ferriorm_fmt::format_schema(&src).unwrap_or_else(|e| panic!("[{name}] parse: {e}"));
        let twice =
            ferriorm_fmt::format_schema(&once).unwrap_or_else(|e| panic!("[{name}] re-parse: {e}"));
        assert_eq!(once, twice, "[{name}] format is not idempotent");
    }
}
