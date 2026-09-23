#[path = "../tests/common/shapes.rs"]
mod shapes;
use pixelstrict_core::{Options, convert};
use std::{env, fs, path::PathBuf};

fn main() {
    let dir = PathBuf::from(
        env::args_os()
            .nth(1)
            .unwrap_or_else(|| "samples/shape".into()),
    );
    fs::create_dir_all(&dir).unwrap();
    for (name, source, manual_width) in [
        ("building-like", shapes::building_like(), 48),
        ("line-heavy", shapes::line_heavy(), 48),
        ("flat-with-noise", shapes::flat_with_noise(), 48),
        ("vertical-boundary", shapes::vertical_lines(false), 24),
        ("vertical-shades", shapes::vertical_lines(true), 24),
        ("straight-facade", shapes::straight_facade(), 32),
        ("blended-frame", shapes::blended_frame(), 32),
        ("shaded-stroke", shapes::shaded_stroke(), 32),
        ("interrupted-stroke", shapes::interrupted_stroke(), 32),
        ("boundary-midpoint", shapes::boundary_midpoint(), 32),
    ] {
        let input = dir.join(format!("{name}-input.png"));
        source.save(&input).unwrap();
        for (suffix, grid_width) in [("manual", Some(manual_width)), ("auto", None)] {
            let result = convert(
                &fs::read(&input).unwrap(),
                &Options {
                    grid_width,
                    colors: Some(32),
                    smoothing: 3,
                    ..Options::default()
                },
            )
            .unwrap();
            fs::write(dir.join(format!("{name}-{suffix}.png")), result.png).unwrap();
            fs::write(
                dir.join(format!("{name}-{suffix}.json")),
                serde_json::to_string_pretty(&result.report).unwrap(),
            )
            .unwrap();
        }
    }
}
