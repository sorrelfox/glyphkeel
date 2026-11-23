// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! `gk-seed --set S|L|hostile --seed N --out <dir>`

use std::io::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use gk_seed::DataSet;

/// Writes a deterministic test data set as an unpacked export archive.
#[derive(Debug, Parser)]
#[command(version)]
struct Args {
    /// Which data set: S (function tests), L (performance) or hostile.
    #[arg(long, value_parser = parse_set)]
    set: DataSet,
    /// The same seed always gives byte-identical output.
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Target folder; must not exist yet or be empty.
    #[arg(long)]
    out: PathBuf,
}

fn parse_set(s: &str) -> Result<DataSet, String> {
    DataSet::parse(s).ok_or_else(|| format!("unknown data set {s:?}; use S, L or hostile"))
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let started = Instant::now();
    let archive = gk_seed::generate(args.set, args.seed)?;
    let built = started.elapsed();
    archive.write_to(&args.out)?;
    let mut out = std::io::stdout().lock();
    writeln!(
        out,
        "data set {} (seed {}): {} files, {:.1} MB in {}; generated in {:.2} s, written in {:.2} s",
        args.set.name(),
        args.seed,
        archive.files.len(),
        archive.size() as f64 / 1_000_000.0,
        args.out.display(),
        built.as_secs_f64(),
        (started.elapsed() - built).as_secs_f64(),
    )?;
    Ok(())
}
