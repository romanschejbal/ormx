//! `ferriorm format` — canonicalize one or more `.ferriorm` files.
//!
//! Behavior:
//! - With no `paths`, formats the schema at `--schema` (default `schema.ferriorm`).
//! - Each path may be a file or a directory; directories are walked for
//!   `*.ferriorm` files (non-recursive — schemas don't usually nest).
//! - `--check` prints a unified diff for any file whose canonical form
//!   differs and exits with code 1; nothing is written.
//! - `--stdin` reads from stdin and writes the formatted output to stdout.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub async fn run(
    default_schema: &str,
    paths: Vec<String>,
    check: bool,
    stdin: bool,
) -> miette::Result<()> {
    if stdin {
        return run_stdin();
    }

    let targets: Vec<PathBuf> = if paths.is_empty() {
        vec![PathBuf::from(default_schema)]
    } else {
        let mut out = Vec::new();
        for p in &paths {
            collect_files(Path::new(p), &mut out)?;
        }
        out
    };

    if targets.is_empty() {
        return Err(miette::miette!("no .ferriorm files found"));
    }

    let mut changed_paths: Vec<PathBuf> = Vec::new();
    for path in &targets {
        let original = fs::read_to_string(path)
            .map_err(|e| miette::miette!("failed to read {}: {e}", path.display()))?;
        let formatted = ferriorm_fmt::format_schema(&original)
            .map_err(|e| miette::miette!("{}: {e}", path.display()))?;

        if formatted == original {
            continue;
        }

        if check {
            print_diff(path, &original, &formatted);
            changed_paths.push(path.clone());
        } else {
            write_atomic(path, &formatted)?;
            println!("formatted {}", path.display());
        }
    }

    if check && !changed_paths.is_empty() {
        eprintln!("\n{} file(s) need formatting", changed_paths.len());
        std::process::exit(1);
    }

    Ok(())
}

fn run_stdin() -> miette::Result<()> {
    let mut buf = String::new();
    io::stdin()
        .read_to_string(&mut buf)
        .map_err(|e| miette::miette!("read stdin: {e}"))?;
    let formatted = ferriorm_fmt::format_schema(&buf).map_err(|e| miette::miette!("{e}"))?;
    io::stdout()
        .write_all(formatted.as_bytes())
        .map_err(|e| miette::miette!("write stdout: {e}"))?;
    Ok(())
}

fn collect_files(path: &Path, out: &mut Vec<PathBuf>) -> miette::Result<()> {
    let meta = fs::metadata(path).map_err(|e| miette::miette!("stat {}: {e}", path.display()))?;
    if meta.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    if meta.is_dir() {
        for entry in
            fs::read_dir(path).map_err(|e| miette::miette!("read_dir {}: {e}", path.display()))?
        {
            let entry = entry.map_err(|e| miette::miette!("read_dir entry: {e}"))?;
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) == Some("ferriorm") {
                out.push(p);
            }
        }
        return Ok(());
    }
    Err(miette::miette!(
        "{} is neither a file nor a directory",
        path.display()
    ))
}

fn write_atomic(path: &Path, contents: &str) -> miette::Result<()> {
    let tmp = path.with_extension("ferriorm.tmp");
    fs::write(&tmp, contents).map_err(|e| miette::miette!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| miette::miette!("rename {}: {e}", path.display()))?;
    Ok(())
}

/// Tiny line-oriented unified-diff renderer. Avoids pulling in a diff crate
/// for a feature that's used only on the rare reformat-needed path.
fn print_diff(path: &Path, original: &str, formatted: &str) {
    println!("--- {}", path.display());
    println!("+++ {} (formatted)", path.display());
    let orig_lines: Vec<&str> = original.lines().collect();
    let fmt_lines: Vec<&str> = formatted.lines().collect();

    // Naive line-by-line diff: prefix unchanged lines with " ", removed with
    // "-", added with "+". Sufficient for short schemas; the LSP exposes
    // proper edits instead.
    let n = orig_lines.len().max(fmt_lines.len());
    for i in 0..n {
        let o = orig_lines.get(i).copied();
        let f = fmt_lines.get(i).copied();
        match (o, f) {
            (Some(a), Some(b)) if a == b => println!(" {a}"),
            (Some(a), Some(b)) => {
                println!("-{a}");
                println!("+{b}");
            }
            (Some(a), None) => println!("-{a}"),
            (None, Some(b)) => println!("+{b}"),
            (None, None) => {}
        }
    }
    println!();
}
