#[path = "../tests/common/mod.rs"]
mod common;
use common::*;
use pixelstrict_core::{Options, OutputMode, convert};
use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(env::args_os().nth(1).unwrap_or_else(|| "../samples".into()));
    fs::create_dir_all(&dir).unwrap();
    let reference = house();
    let input = noisy(&reference, 6);
    fs::write(dir.join("house-reference.png"), encode(&reference)).unwrap();
    fs::write(dir.join("house-pseudo.png"), encode(&input)).unwrap();
    // Keep reference comparisons at one pixel per logical cell.
    let logical = Options {
        output_mode: OutputMode::Logical,
        ..Options::default()
    };
    for (name, options) in [
        ("auto", logical.clone()),
        (
            "surface-weak",
            Options {
                grid_width: Some(32),
                colors: Some(32),
                smoothing: 1,
                ..logical.clone()
            },
        ),
        (
            "surface-strong",
            Options {
                grid_width: Some(32),
                colors: Some(32),
                smoothing: 3,
                ..logical
            },
        ),
    ] {
        let result = convert(&encode(&input), &options).unwrap();
        fs::write(dir.join(format!("house-{name}.png")), &result.png).unwrap();
        fs::write(
            dir.join(format!("house-{name}.json")),
            serde_json::to_string_pretty(&result.report).unwrap(),
        )
        .unwrap();
        println!(
            "{name}: {}x{}, {} colors, {:.1} ms",
            result.report.grid.width,
            result.report.grid.height,
            result.report.palette.len(),
            result.report.processing_ms
        );
    }
}
