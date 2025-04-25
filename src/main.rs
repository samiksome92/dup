//! A command line utility to detect duplicate files present in directories.
//!
//! Allows for searching across directories instead of within them with `--cross`, and recursive searches using
//! `--recursive`. Once duplicates are found the user is prompted whether to delete them or not.

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
use tabled::{
    Table, Tabled,
    settings::{
        Color, Padding, Style, Width,
        object::{Columns, Rows},
    },
};
use termsize::{self, Size};

const CHUNK_SIZE: usize = 1024 * 1024;

/// Command line arguments.
#[derive(Parser)]
#[command(version, about = None, long_about = None)]
struct Args {
    #[arg(required = true, help = "Directory(s)")]
    dirs: Vec<PathBuf>,
    #[arg(short = 'x', long, help = "Cross check across directories")]
    cross: bool,
    #[arg(short, long, help = "Recursively check directories")]
    recursive: bool,
}

/// Table row.
#[derive(Tabled)]
struct Row {
    #[tabled(rename = "File")]
    dup: String,
    #[tabled(rename = "Duplicate of")]
    dup_to: String,
}

/// Retrieves list of files in a directory.
///
/// If `recursive` is specified, all subdirectories are searched as well. Any error is propagated with added context.
fn get_files<P>(dir: P, recursive: bool) -> Result<Vec<PathBuf>>
where
    P: AsRef<Path>,
{
    let dir = dir.as_ref();
    let mut files = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("Failed to read directory {}", dir.display()))?
    {
        let path = entry
            .with_context(|| format!("Error while reading directory {}", dir.display()))?
            .path();

        if recursive && path.is_dir() {
            files.append(&mut get_files(path, true)?);
        } else if path.is_file() {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Makes pairs from list of all files for comparison.
///
/// If `cross` is `true` files within a single directory are also compared.
fn get_pairs(all_files: &[Vec<PathBuf>], cross: bool) -> Vec<(&PathBuf, &PathBuf)> {
    let mut pairs = Vec::new();

    if !cross {
        pairs.append(
            &mut all_files
                .iter() // Iterate through all directories.
                .flat_map(|files| files.iter().combinations(2).collect_vec()) // Get pairs of files in each directory.
                .map(|f| (f[0], f[1])) // Convert vector to tuple.
                .collect(),
        );
    }

    pairs.append(
        &mut all_files
            .iter() // Iterate through all directories.
            .combinations(2) // Get pairs of directories.
            .flat_map(|dirs| dirs[0].iter().cartesian_product(dirs[1]).collect_vec()) // For each such pair get cartesian product of files.
            .collect(),
    );

    pairs
}

/// Checks if two files are same.
///
/// Performs a byte for byte comparison with early exit in case of differences.
fn is_same<P, Q>(file1: P, file2: Q) -> Result<bool>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    let file1 = file1.as_ref();
    let file2 = file2.as_ref();

    if file1.metadata()?.len() != file2.metadata()?.len() {
        return Ok(false);
    }

    let mut fp1 =
        File::open(file1).with_context(|| format!("Failed to open file {}", file1.display()))?;
    let mut fp2 =
        File::open(file2).with_context(|| format!("Failed to open file {}", file2.display()))?;
    let mut buf1 = vec![0; CHUNK_SIZE];
    let mut buf2 = vec![0; CHUNK_SIZE];

    loop {
        let n1 = fp1
            .read(&mut buf1)
            .with_context(|| format!("Error while reading file {}", file1.display()))?;
        let n2 = fp2
            .read(&mut buf2)
            .with_context(|| format!("Error while reading file {}", file2.display()))?;

        if n1 != n2 {
            return Ok(false);
        }

        if n1 == 0 {
            break;
        }

        if buf1[..n1] != buf2[..n2] {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Finds duplicate files and displays them.
///
/// Also prompts for their removal.
fn dup(dirs: Vec<PathBuf>, cross: bool, recursive: bool) -> Result<()> {
    let dirs = dirs.into_iter();
    let files: Result<Vec<_>> = dirs.map(|d| get_files(d, recursive)).collect();
    let files = files?;
    let pairs = get_pairs(&files, cross);

    let mut dups = HashMap::new();
    let bar = ProgressBar::new(
        pairs
            .len()
            .try_into()
            .with_context(|| format!("Could not convert {} from usize to u64", pairs.len()))?,
    );
    bar.set_style(
        ProgressStyle::with_template("Checking files {bar:40.white/white.dim} {pos}/{len}")
            .context("Failed to set progress bar style")?
            .progress_chars("━╸━"),
    );
    for (file1, file2) in pairs {
        if dups.contains_key(file1) || dups.contains_key(file2) {
            bar.inc(1);
            continue;
        }

        if is_same(file1, file2)? {
            dups.insert(file2, file1);
        }

        bar.inc(1);
    }
    bar.finish();
    eprintln!(); // TODO: indicatiff has a bug where it does not print a new line after finishing. Once it is fixed update indicatiff and remove this line.

    if dups.is_empty() {
        println!("No duplicates found");
    } else {
        let data = dups
            .iter()
            .sorted()
            .map(|r| Row {
                dup: r.0.display().to_string(),
                dup_to: r.1.display().to_string(),
            })
            .collect_vec();
        let mut table = Table::new(data);
        let style = Style::sharp().remove_frame().remove_vertical();
        table.with(style).with(Width::wrap::<usize>(
            termsize::get()
                .unwrap_or(Size { rows: 0, cols: 80 })
                .cols
                .into(),
        ));
        table.modify(Rows::first(), Color::BOLD);
        table.modify(Columns::first(), Padding::new(0, 2, 0, 0));
        table.modify(Columns::last(), Padding::new(2, 0, 0, 0));
        println!("\n{table}");

        print!("\nRemove {} duplicates? [y/N] ", dups.len());
        io::stdout().flush().context("Failed to flush stdout")?;
        let mut choice = String::new();
        io::stdin()
            .read_line(&mut choice)
            .context("Failed to read user input")?;
        choice = choice.to_lowercase();
        let choice = choice.trim();
        if choice == "y" || choice == "yes" {
            println!("Removing duplicates...");
            for dup in dups.keys() {
                fs::remove_file(dup)
                    .with_context(|| format!("Failed to remove file {}", dup.display()))?;
            }
        }
    }

    Ok(())
}

fn main() {
    let args = Args::parse();

    if args.cross && args.dirs.len() < 2 {
        println!("At least two directories are required for cross comparison");
        return;
    }

    if let Err(e) = dup(args.dirs, args.cross, args.recursive) {
        eprintln!("{e}");
    }
}
