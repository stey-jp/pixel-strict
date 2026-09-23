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
fn straight_subcell_lines_do_not_grow_teeth_or_break_at_shade_changes() {
    for shaded in [false, true] {
        let reference = shapes::vertical_lines(shaded);
        for horizontal in [false, true] {
            let source = if horizontal {
                image::imageops::rotate90(&reference)
            } else {
                reference.clone()
            };
            for output_mode in [OutputMode::Preserve, OutputMode::Logical] {
                let result = convert(
                    &encode(&source),
                    &Options {
                        output_mode,
                        ..settings(24)
                    },
                )
                .unwrap();
                let out = decoded(&result.png);
                let out = if horizontal {
                    image::imageops::rotate270(&out)
                } else {
                    out
                };
                let pitch = if output_mode == OutputMode::Preserve {
                    4
                } else {
                    1
                };
                let mut columns = None;
                for y in 2..22 {
                    let ink: Vec<_> = (0..24)
                        .filter(|x| out.get_pixel(x * pitch, y * pitch)[0] < 100)
                        .collect();
                    assert_eq!(
                        ink.len(),
                        3,
                        "shaded {shaded}, horizontal {horizontal}, row {y}: {ink:?}"
                    );
                    if let Some(expected) = &columns {
                        assert_eq!(&ink, expected, "line shifted at row {y}");
                    }
                    columns = Some(ink);
                }
                for y in [0, 1, 22, 23] {
                    assert!((0..24).all(|x| out.get_pixel(x * pitch, y * pitch)[0] >= 100));
                }
                if pitch == 4 {
                    for (x, y, p) in out.enumerate_pixels() {
                        assert_eq!(p, out.get_pixel(x / 4 * 4, y / 4 * 4));
                    }
                }
                assert_eq!(
                    result.png,
                    convert(&encode(&source), &result.report.options)
                        .unwrap()
                        .png
                );
            }
        }
    }
}

#[test]
fn straight_face_boundaries_keep_source_position_across_shading() {
    let reference = shapes::straight_facade();
    for horizontal in [false, true] {
        let source = if horizontal {
            image::imageops::rotate90(&reference)
        } else {
            reference.clone()
        };
        for output_mode in [OutputMode::Preserve, OutputMode::Logical] {
            let options = Options {
                output_mode,
                ..settings(32)
            };
            let result = convert(&encode(&source), &options).unwrap();
            let out = decoded(&result.png);
            let out = if horizontal {
                image::imageops::rotate270(&out)
            } else {
                out
            };
            let pitch = if output_mode == OutputMode::Preserve {
                3
            } else {
                1
            };
            for y in 0..32 {
                // Source boundary x=31 belongs at grid boundary x=30: two
                // pixels of this mixed cell are on the light face in every row.
                assert!(out.get_pixel(9 * pitch, y * pitch)[0] < 180);
                assert!(
                    out.get_pixel(10 * pitch, y * pitch)[0] > 200,
                    "edge shifted at row {y}"
                );
            }
            for (x, y, pixel) in out.enumerate_pixels() {
                assert_eq!(pixel, out.get_pixel(x / pitch * pitch, y / pitch * pitch));
            }
            assert_eq!(result.png, convert(&encode(&source), &options).unwrap().png);
        }
    }
}

#[test]
fn straight_alignment_keeps_separate_lines_wide_columns_and_junctions() {
    let source = RgbaImage::from_fn(96, 96, |x, y| {
        let ink = (8..88).contains(&y) && ((16..24).contains(&x) || [43, 45, 67, 68].contains(&x))
            || ((48..52).contains(&y) && (60..76).contains(&x));
        Rgba(if ink {
            [24, 28, 32, 255]
        } else {
            [200, 184, 160, 255]
        })
    });
    let result = convert(
        &encode(&source),
        &Options {
            output_mode: OutputMode::Logical,
            ..settings(24)
        },
    )
    .unwrap();
    let out = decoded(&result.png);
    for y in 2..22 {
        for x in [4, 5, 10, 11] {
            assert!(
                out.get_pixel(x, y)[0] < 100,
                "lost separate/wide line at {x}, {y}"
            );
        }
        assert!(
            [16, 17].into_iter().any(|x| out.get_pixel(x, y)[0] < 100),
            "junction line broke at {y}"
        );
    }
    for x in 15..19 {
        assert!(out.get_pixel(x, 12)[0] < 100, "lost crossbar at {x}");
    }
}

#[test]
fn blended_frame_uses_source_geometry_across_palette_sizes() {
    let reference = shapes::blended_frame();
    for horizontal in [false, true] {
        let input = if horizontal {
            image::imageops::rotate90(&reference)
        } else {
            reference.clone()
        };
        for colors in [16, 24, 32] {
            for output_mode in [OutputMode::Preserve, OutputMode::Logical] {
                let options = Options {
                    colors: Some(colors),
                    output_mode,
                    ..settings(32)
                };
                let result = convert(&encode(&input), &options).unwrap();
                let out = decoded(&result.png);
                let out = if horizontal {
                    image::imageops::rotate270(&out)
                } else {
                    out
                };
                let pitch = if output_mode == OutputMode::Preserve {
                    3
                } else {
                    1
                };
                for y in 0..32 {
                    assert!(
                        out.get_pixel(10 * pitch, y * pitch)[0] > 200,
                        "frame expanded into glass at {y}"
                    );
                    assert!(
                        out.get_pixel(11 * pitch, y * pitch)[0] < 120,
                        "frame erased at {y}"
                    );
                }
                for (x, y, pixel) in out.enumerate_pixels() {
                    assert_eq!(pixel, out.get_pixel(x / pitch * pitch, y / pitch * pitch));
                }
                assert_eq!(result.png, convert(&encode(&input), &options).unwrap().png);
            }
        }
    }
}

#[test]
fn source_boundary_guide_keeps_real_steps_and_gaps() {
    let frame = shapes::blended_frame();
    for stepped in [false, true] {
        let input = RgbaImage::from_fn(96, 96, |x, y| {
            if !stepped && (45..51).contains(&y) && x < 60 {
                return Rgba([226, 208, 184, 255]);
            }
            let shift = if stepped { y / 12 * 3 } else { 0 };
            *frame.get_pixel(x.saturating_sub(shift), y)
        });
        for horizontal in [false, true] {
            let input = if horizontal {
                image::imageops::rotate90(&input)
            } else {
                input.clone()
            };
            let result = convert(&encode(&input), &settings(32)).unwrap();
            let out = decoded(&result.png);
            let out = if horizontal {
                image::imageops::rotate270(&out)
            } else {
                out
            };
            for y in 0..32 {
                if !stepped && (15..17).contains(&y) {
                    assert!(
                        (0..20).all(|x| out.get_pixel(x * 3, y * 3)[0] > 200),
                        "bridged a real gap"
                    );
                } else {
                    let edge = 11 + if stepped { y / 4 } else { 0 };
                    assert!(
                        out.get_pixel((edge - 2) * 3, y * 3)[0] > 200,
                        "straightened a step at {y}"
                    );
                    assert!(
                        out.get_pixel((edge + 1) * 3, y * 3)[0] < 130,
                        "lost a step at {y}"
                    );
                }
            }
        }
    }
}

#[test]
fn source_boundary_guide_leaves_flat_background_uniform() {
    let source = noisy(&house(), 6);
    let result = convert(&encode(&source), &settings(32)).unwrap();
    let out = decoded(&result.png);
    let sky = *out.get_pixel(5 * 6, 16 * 6);
    // The flat strip next to the building must still merge with the sky.
    for y in 16..27 {
        for x in 4..7 {
            assert_eq!(
                *out.get_pixel(x * 6, y * 6),
                sky,
                "surface speckle at {x}, {y}"
            );
        }
    }
}

#[test]
fn stroke_tones_follow_source_brightness_without_erasing_lighting_changes() {
    let source = shapes::shaded_stroke();
    for horizontal in [false, true] {
        let input = if horizontal {
            image::imageops::rotate90(&source)
        } else {
            source.clone()
        };
        for output_mode in [OutputMode::Preserve, OutputMode::Logical] {
            let options = Options {
                output_mode,
                ..settings(32)
            };
            let result = convert(&encode(&input), &options).unwrap();
            let out = decoded(&result.png);
            let out = if horizontal {
                image::imageops::rotate270(&out)
            } else {
                out
            };
            let pitch = if output_mode == OutputMode::Preserve {
                3
            } else {
                1
            };
            for y in (4..14).chain(18..28) {
                let expected = if y < 16 { 80 } else { 112 };
                for x in 10..14 {
                    assert!(
                        out.get_pixel(x * pitch, y * pitch)[1].abs_diff(expected) <= 2,
                        "wrong stroke tone at {x}, {y}"
                    );
                }
            }
            // Tone selection cannot change the stroke width, ends or light step.
            for y in 0..32 {
                for x in 0..20 {
                    let in_stroke = (2..30).contains(&y) && (10..14).contains(&x);
                    assert_eq!(out.get_pixel(x * pitch, y * pitch)[1] < 160, in_stroke);
                }
            }
            assert!(out.get_pixel(11 * pitch, 15 * pitch)[1] < 105);
            assert!(out.get_pixel(11 * pitch, 16 * pitch)[1] > 105);
            for (x, y, pixel) in out.enumerate_pixels() {
                assert_eq!(pixel, out.get_pixel(x / pitch * pitch, y / pitch * pitch));
            }
            assert_eq!(result.png, convert(&encode(&input), &options).unwrap().png);
        }
    }
}

#[test]
fn stroke_tone_selection_keeps_crossbars_and_colored_details() {
    let mut source = shapes::shaded_stroke();
    for y in 27..30 {
        for x in 24..48 {
            source.put_pixel(x, y, Rgba([30, 32, 36, 255]));
        }
    }
    for y in 66..72 {
        for x in 33..36 {
            source.put_pixel(x, y, Rgba([164, 62, 48, 255]));
        }
    }
    let result = convert(&encode(&source), &settings(32)).unwrap();
    let out = decoded(&result.png);
    for x in 8..16 {
        assert!(out.get_pixel(x * 3, 27)[0] < 60, "lost crossbar at {x}");
    }
    for y in 22..24 {
        let p = out.get_pixel(33, y * 3);
        assert!(p[0] > 140 && p[1] < 90, "lost colored detail at {y}");
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
