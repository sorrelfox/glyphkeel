// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Glyphkeel contributors

//! Timings of the Markdown jobs on a 4 KB description and a 100 KB page.
//!
//! Run with `cargo bench -p gk-markdown`. Prints the median and the 95th
//! percentile of each job.

#![allow(clippy::print_stdout, clippy::unwrap_used)]

use std::hint::black_box;
use std::time::{Duration, Instant};

use gk_markdown::{RenderOptions, analyze, checklist, plain_text, render, toggle};

const TYPICAL: &str = include_str!("../tests/fixtures/typical.md");

/// The typical description repeated, with distinct footnote names, until it
/// is at least `bytes` long.
fn document(bytes: usize) -> String {
    let mut out = String::new();
    let mut i = 0;
    while out.len() < bytes {
        i += 1;
        out.push_str(&TYPICAL.replace("[^1]", &format!("[^{i}]")));
        out.push_str("\n\n");
    }
    out
}

/// A named job to time.
type Job<'a> = (&'static str, Box<dyn FnMut() + 'a>);

fn time(iterations: usize, mut job: impl FnMut()) -> (Duration, Duration) {
    for _ in 0..iterations / 10 + 1 {
        job();
    }
    let mut samples: Vec<Duration> = (0..iterations)
        .map(|_| {
            let start = Instant::now();
            job();
            start.elapsed()
        })
        .collect();
    samples.sort();
    (
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
    )
}

fn ms(d: Duration) -> String {
    format!("{:.3} ms", d.as_secs_f64() * 1000.0)
}

fn main() {
    let known = |key: &str| key.starts_with("TM-");
    println!("| input | size | job | median | p95 |");
    println!("|---|---|---|---|---|");
    for (name, source, iterations) in [
        ("4 KB description", document(3 * 1024), 2000),
        ("100 KB page", document(100 * 1024), 200),
    ] {
        let items = checklist(&source);
        let middle = &items[items.len() / 2];
        let options = RenderOptions::new("bench").with_task_keys(&known);
        let jobs: [Job<'_>; 5] = [
            (
                "render",
                Box::new(|| drop(black_box(render(black_box(&source), &options)))),
            ),
            (
                "checklist",
                Box::new(|| drop(black_box(checklist(black_box(&source))))),
            ),
            (
                "toggle",
                Box::new(|| {
                    drop(black_box(
                        toggle(black_box(&source), middle.number, &middle.text).unwrap(),
                    ));
                }),
            ),
            (
                "plain_text",
                Box::new(|| drop(black_box(plain_text(black_box(&source))))),
            ),
            (
                "analyze",
                Box::new(|| drop(black_box(analyze(black_box(&source))))),
            ),
        ];
        for (job, mut run) in jobs {
            let (median, p95) = time(iterations, &mut run);
            println!(
                "| {name} | {} B | {job} | {} | {} |",
                source.len(),
                ms(median),
                ms(p95)
            );
        }
    }
}
