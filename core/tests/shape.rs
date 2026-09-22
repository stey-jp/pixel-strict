mod common;
#[path = "common/shapes.rs"]
mod shapes;
use common::*;
use image::{Rgba, RgbaImage};
use pixelstrict_core::{Options, OutputMode, convert};

fn settings(width: u32) -> Options {
    Options {
        grid_width: Some(width),
        colors: Some(32),
        smoothing: 3,
        ..Options::default()
    }
}

fn decoded(bytes: &[u8]) -> RgbaImage {
    image::load_from_memory(bytes).unwrap().to_rgba8()
}

#[test]
fn transparent_building_silhouette_and_protrusions_survive() {
    let input = RgbaImage::from_fn(32, 32, |x, y| {
        if ((7..25).contains(&x) && (12..28).contains(&y))
            || ((10..13).contains(&x) && (5..12).contains(&y))
            || (x == 25 && (20..23).contains(&y))
        {
            Rgba([180, 168, 152, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    for shape_protection in 1..=3 {
        let result = convert(
            &encode(&input),
            &Options {
                shape_protection,
                ..settings(32)
            },
        )
        .unwrap();
        assert_eq!(decoded(&result.png), input);
        assert!(result.report.silhouette_count > 0);
        assert_eq!(
            result.report.candidates[0].metrics.silhouette_retention,
            1.0
        );
    }
}

#[test]
fn subtle_lines_in_all_eight_directions_survive_surface_merge() {
    for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
        for thickness in 1..=2 {
            let mut input = RgbaImage::from_pixel(32, 32, Rgba([144, 151, 162, 255]));
            let mut points = Vec::new();
            for t in -7..=7 {
                for offset in 0..thickness {
                    let x = (16 + dx * t + if dy != 0 { offset } else { 0 }) as u32;
                    let y = (16 + dy * t + if dy == 0 { offset } else { 0 }) as u32;
                    input.put_pixel(x, y, Rgba([160, 167, 178, 255]));
                    points.push((x, y));
                }
            }
            let result = convert(&encode(&input), &settings(32)).unwrap();
            let out = decoded(&result.png);
            for (x, y) in points {
                assert!(
                    out.get_pixel(x, y)[0] > 152,
                    "lost ({x}, {y}), axis ({dx}, {dy}), width {thickness}"
                );
            }
            assert!(result.report.classes[4] > 0);
            assert_eq!(result.report.line_continuity_score, 1.0);
        }
    }
}

#[test]
fn connected_small_shapes_survive_but_single_surface_noise_is_removed() {
    for size in 2..=6 {
        let mut input = RgbaImage::from_pixel(24, 24, Rgba([144, 151, 162, 255]));
        input.put_pixel(4, 4, Rgba([151, 158, 169, 255]));
        for i in 0..size {
            input.put_pixel(12 + i % 2, 12 + i / 2, Rgba([160, 167, 178, 255]));
        }
        let result = convert(&encode(&input), &settings(24)).unwrap();
        let out = decoded(&result.png);
        assert_eq!(out.get_pixel(4, 4), out.get_pixel(5, 4));
        for i in 0..size {
            assert!(
                out.get_pixel(12 + i % 2, 12 + i / 2)[0] > 152,
                "lost detail of size {size}"
            );
        }
        assert!(result.report.preserved_detail_count >= size as usize);
    }
}

#[test]
fn minority_line_including_endpoints_survives_integer_cells() {
    let input = RgbaImage::from_fn(32, 32, |x, y| {
        if x == 13 && (5..27).contains(&y) {
            Rgba([24, 28, 32, 255])
        } else {
            Rgba([200, 184, 160, 255])
        }
    });
    let result = convert(&encode(&input), &settings(8)).unwrap();
    let out = decoded(&result.png);
    for y in 1..7 {
        assert!(out.get_pixel(12, y * 4)[0] < 80, "lost line cell {y}");
    }
}

#[test]
fn shape_output_remains_deterministic_and_preserve_matches_logical_cells() {
    let input = noisy(&house(), 4);
    let bytes = encode(&input);
    let preserve = convert(&bytes, &settings(32)).unwrap();
    assert_eq!(preserve.png, convert(&bytes, &settings(32)).unwrap().png);
    let logical = convert(
        &bytes,
        &Options {
            output_mode: OutputMode::Logical,
            ..settings(32)
        },
    )
    .unwrap();
    let full = decoded(&preserve.png);
    let small = decoded(&logical.png);
    assert_eq!(full.dimensions(), (128, 128));
    assert_eq!(small.dimensions(), (32, 32));
    for y in 0..128 {
        for x in 0..128 {
            assert_eq!(full.get_pixel(x, y), small.get_pixel(x / 4, y / 4));
        }
    }
    assert_eq!(preserve.report.classes.iter().sum::<usize>(), 1024);
}

#[test]
fn auto_scores_source_shapes_even_when_coarse_candidates_lose_them() {
    let input = shapes::line_heavy();
    let bytes = encode(&input);
    let fine = convert(&bytes, &settings(48)).unwrap();
    let coarse = convert(&bytes, &settings(3)).unwrap();
    assert!(fine.report.shape_score > coarse.report.shape_score + 0.2);
    for mode in [OutputMode::Preserve, OutputMode::Logical] {
        let auto = convert(
            &bytes,
            &Options {
                grid_width: None,
                output_mode: mode,
                ..settings(48)
            },
        )
        .unwrap();
        assert!(
            auto.report.shape_score > 0.85,
            "{:?}",
            auto.report.candidates
        );
        assert!(auto.report.shape_score > coarse.report.shape_score);
        assert_eq!(auto.png, convert(&bytes, &auto.report.options).unwrap().png);
    }
}

#[test]
fn auto_preserves_line_width_and_position_at_multiple_scales() {
    for reference in [
        shapes::line_heavy(),
        image::imageops::flip_vertical(&shapes::line_heavy()),
    ] {
        for scale in [1, 2, 4] {
            let source = image::imageops::resize(
                &reference,
                48 * scale,
                48 * scale,
                image::imageops::FilterType::Nearest,
            );
            let bytes = encode(&source);
            for output_mode in [OutputMode::Preserve, OutputMode::Logical] {
                let result = convert(
                    &bytes,
                    &Options {
                        grid_width: None,
                        output_mode,
                        ..settings(48)
                    },
                )
                .unwrap();
                let expected = if output_mode == OutputMode::Preserve {
                    &source
                } else {
                    &reference
                };
                let output = decoded(&result.png);
                assert_eq!(
                    result.report.candidates[0].metrics.line_width_retention,
                    1.0
                );
                assert_eq!(
                    output.dimensions(),
                    expected.dimensions(),
                    "scale {scale}, {output_mode:?}"
                );
                for (x, y, pixel) in expected.enumerate_pixels() {
                    assert_eq!(
                        output.get_pixel(x, y),
                        pixel,
                        "line width/position changed at ({x}, {y}), scale {scale}, {output_mode:?}, grid {:?}",
                        result.report.grid
                    );
                }
            }
        }
    }
}

#[test]
fn building_and_noise_fixtures_keep_structure_without_surface_speckles() {
    for source in [shapes::building_like(), shapes::flat_with_noise()] {
        let out = decoded(&convert(&encode(&source), &settings(48)).unwrap().png);
        for (x, y, p) in source.enumerate_pixels() {
            if p.0 == [151, 158, 169, 255] {
                assert_eq!(out.get_pixel(x, y).0, [144, 151, 162, 255]);
            } else {
                assert_eq!(out.get_pixel(x, y), p, "changed shape at {x}, {y}");
            }
        }
    }
    let source = shapes::flat_with_noise();
    let auto = convert(
        &encode(&source),
        &Options {
            grid_width: None,
            ..settings(48)
        },
    )
    .unwrap();
    assert!(auto.report.shape_score > 0.85);
    assert!(decoded(&auto.png).pixels().filter(|p| p[0] > 152).count() >= 2);
    let coarse = convert(&encode(&source), &settings(2)).unwrap();
    assert!(coarse.report.candidates[0].metrics.small_detail_retention < 0.5);
}

#[test]
fn minority_opaque_line_on_transparency_keeps_endpoints() {
    let source = RgbaImage::from_fn(32, 32, |x, y| {
        if x == 13 && (5..27).contains(&y) {
            Rgba([24, 28, 32, 255])
        } else {
            Rgba([0, 0, 0, 0])
        }
    });
    let out = decoded(&convert(&encode(&source), &settings(8)).unwrap().png);
    for y in 1..7 {
        assert_eq!(out.get_pixel(12, y * 4)[3], 255, "lost opaque endpoint {y}");
    }
    assert_eq!(out.get_pixel(8, 12)[3], 0);
}

#[test]
fn old_options_default_to_medium_shape_and_invalid_strengths_are_rejected() {
    let old: Options = serde_json::from_str(r#"{"smoothing":3,"edge_protection":2}"#).unwrap();
    assert_eq!(old.shape_protection, 2);
    for shape_protection in [0, 4, 255] {
        assert!(
            convert(
                &encode(&house()),
                &Options {
                    shape_protection,
                    ..settings(32)
                }
            )
            .is_err()
        );
    }
}
