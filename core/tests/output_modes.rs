mod common;
use common::*;
use image::{Rgba, RgbaImage};
use pixelstrict_core::{Grid, Options, OutputMode, convert, grid};

#[test]
fn preserve_defaults_and_json_modes() {
    for json in ["{}", r#"{"grid_width":32,"median":false}"#] {
        let options: Options = serde_json::from_str(json).unwrap();
        assert_eq!(options.output_mode, OutputMode::Preserve);
        assert_eq!(
            serde_json::to_value(options).unwrap()["output_mode"],
            "preserve"
        );
    }
    for mode in ["preserve", "logical"] {
        let options: Options =
            serde_json::from_value(serde_json::json!({"output_mode": mode})).unwrap();
        assert_eq!(serde_json::to_value(options).unwrap()["output_mode"], mode);
    }
    assert!(serde_json::from_str::<Options>(r#"{"output_mode":"resize"}"#).is_err());
}

#[test]
fn preserve_paints_uniform_rectangles_without_new_colors_or_alpha() {
    for (width, height, pitch) in [
        (1254, 1254, 3),
        (1254, 1254, 2),
        (66, 44, 11),
        (36, 24, 6),
        (23, 17, 1),
    ] {
        let colors = [
            [40, 72, 120, 255],
            [200, 120, 40, 128],
            [111, 222, 77, 127],
            [6, 250, 10, 0],
        ];
        let source = RgbaImage::from_fn(width, height, |x, y| {
            Rgba(colors[((x / pitch + 3 * (y / pitch)) % 4) as usize])
        });
        let bytes = encode(&source);
        let options = Options {
            grid_width: Some(width / pitch),
            ..Options::default()
        };
        let result = convert(&bytes, &options).unwrap();
        assert_eq!(result.png, convert(&bytes, &options).unwrap().png);
        let output = image::load_from_memory(&result.png).unwrap().to_rgba8();
        assert_eq!(output.dimensions(), source.dimensions());
        assert_eq!(
            (result.report.output_width, result.report.output_height),
            (width, height)
        );
        assert_eq!(result.report.cell_pitch, pitch);
        assert_eq!(result.report.output_mode, OutputMode::Preserve);
        assert_eq!(
            result.report.grid,
            Grid {
                width: width / pitch,
                height: height / pitch
            }
        );

        // Same cell decisions, with a single output pixel per cell in legacy mode.
        // P=2 exceeds the legacy limit, so compare against the already-decided palette
        // and rectangles there; all other cases also compare every cell to Logical.
        let logical = if result.report.grid.width as u64 * result.report.grid.height as u64
            <= pixelstrict_core::MAX_CELLS
        {
            let legacy = convert(
                &bytes,
                &Options {
                    output_mode: OutputMode::Logical,
                    ..options
                },
            )
            .unwrap();
            assert_eq!(legacy.report.cell_pitch, 1);
            assert_eq!(
                (legacy.report.output_width, legacy.report.output_height),
                (width / pitch, height / pitch)
            );
            Some(image::load_from_memory(&legacy.png).unwrap().to_rgba8())
        } else {
            None
        };
        for cy in 0..height / pitch {
            for cx in 0..width / pitch {
                let color = output.get_pixel(cx * pitch, cy * pitch);
                assert!(result.report.palette.contains(&color.0));
                if let Some(ref logical) = logical {
                    assert_eq!(color, logical.get_pixel(cx, cy));
                }
                for y in cy * pitch..(cy + 1) * pitch {
                    for x in cx * pitch..(cx + 1) * pitch {
                        assert_eq!(output.get_pixel(x, y), color);
                    }
                }
            }
        }
        assert!(output.pixels().any(|p| p[3] == 0));
        assert!(output.pixels().any(|p| p[3] == 255));
        assert!(output.pixels().all(|p| p[3] == 255 || p.0 == [0, 0, 0, 0]));
    }
}

#[test]
fn preserve_candidates_are_exact_gcd_divisors_without_pitch_cap() {
    for (width, height) in [(1254, 1254), (1254, 836), (197, 199), (1, 33)] {
        let candidates = grid::preserve_candidates(width, height);
        let expected: Vec<_> = (1..=width.min(height))
            .filter(|p| width.is_multiple_of(*p) && height.is_multiple_of(*p))
            .map(|p| Grid {
                width: width / p,
                height: height / p,
            })
            .collect();
        assert_eq!(candidates, expected);
    }
    for pitch in [2, 3, 6, 11, 209, 1254] {
        assert_eq!(
            grid::preserve_from_width(1254, 1254, 1254 / pitch).unwrap(),
            Grid {
                width: 1254 / pitch,
                height: 1254 / pitch
            }
        );
    }
    assert!(grid::preserve_from_width(1254, 1254, 314).is_err());
}

#[test]
fn preserve_manual_rejects_non_divisible_axes_and_respects_mode_limits() {
    for (w, h, columns) in [(1254, 1254, 314), (24, 25, 8)] {
        let bytes = encode(&RgbaImage::from_pixel(w, h, Rgba([40, 80, 120, 255])));
        let options = Options {
            grid_width: Some(columns),
            ..Options::default()
        };
        let error = convert(&bytes, &options).err().unwrap();
        assert!(
            error.contains("Preserve output requires equal integer square cells"),
            "{error}"
        );
        assert!(
            convert(
                &bytes,
                &Options {
                    output_mode: OutputMode::Logical,
                    ..options
                }
            )
            .is_ok()
        );
    }
    for (mode, size, limit) in [
        (OutputMode::Preserve, 1025, "1048576"),
        (OutputMode::Logical, 513, "262144"),
    ] {
        let bytes = encode(&RgbaImage::from_pixel(size, size, Rgba([40, 80, 120, 255])));
        let error = convert(
            &bytes,
            &Options {
                output_mode: mode,
                grid_width: Some(size),
                ..Options::default()
            },
        )
        .err()
        .unwrap();
        assert!(error.contains(limit), "{error}");
    }
}

#[test]
fn auto_preserve_selects_only_equal_integer_square_cells() {
    let source = noisy(&house(), 6);
    for (w, h) in [(197, 197), (1254, 1254), (126, 84), (197, 199)] {
        let input = image::imageops::resize(&source, w, h, image::imageops::FilterType::Nearest);
        let result = convert(&encode(&input), &Options::default()).unwrap();
        assert_eq!(
            image::load_from_memory(&result.png)
                .unwrap()
                .to_rgba8()
                .dimensions(),
            (w, h)
        );
        for candidate in &result.report.candidates {
            let g = candidate.grid;
            assert!(w.is_multiple_of(g.width));
            let pitch = w / g.width;
            assert!(h.is_multiple_of(pitch));
            assert_eq!(h / pitch, g.height);
            assert!(g.width as u64 * g.height as u64 <= pixelstrict_core::MAX_PRESERVE_CELLS);
        }
        assert_eq!(result.report.grid.width * result.report.cell_pitch, w);
        assert_eq!(result.report.grid.height * result.report.cell_pitch, h);
    }
    for pitch in [3, 4, 6, 8] {
        let input = noisy(&house(), pitch);
        let result = convert(&encode(&input), &Options::default()).unwrap();
        assert_eq!(
            result.report.grid,
            Grid {
                width: 32,
                height: 32
            }
        );
        assert_eq!(result.report.cell_pitch, pitch);
    }
    // Coprime dimensions have only P=1, which can exceed the bounded cell count.
    let input = RgbaImage::from_pixel(1025, 1026, Rgba([0, 0, 0, 255]));
    assert!(
        convert(&encode(&input), &Options::default())
            .err()
            .unwrap()
            .contains("No valid grid candidates")
    );
}

#[test]
fn logical_pngs_match_existing_regression_samples_byte_for_byte() {
    let bytes = include_bytes!("../../samples/house-pseudo.png");
    for (width, colors, smoothing, png) in [
        (
            None,
            None,
            2,
            include_bytes!("../../samples/house-auto.png").as_slice(),
        ),
        (
            Some(32),
            Some(32),
            1,
            include_bytes!("../../samples/house-surface-weak.png").as_slice(),
        ),
        (
            Some(32),
            Some(32),
            3,
            include_bytes!("../../samples/house-surface-strong.png").as_slice(),
        ),
    ] {
        let result = convert(
            bytes,
            &Options {
                output_mode: OutputMode::Logical,
                grid_width: width,
                colors,
                smoothing,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(result.png, png);
        assert_eq!(
            (result.report.output_width, result.report.output_height),
            (32, 32)
        );
    }
}

#[test]
fn ffi_old_json_defaults_to_preserve_and_returns_additive_report_fields() {
    use pixelstrict_core::ffi::*;
    let bytes = encode(&noisy(&house(), 3));
    let settings = br#"{"grid_width":32}"#;
    unsafe {
        let result = ps_convert(
            bytes.as_ptr(),
            bytes.len(),
            settings.as_ptr(),
            settings.len(),
        );
        let json = std::slice::from_raw_parts(ps_result_json(result), ps_result_json_len(result));
        let report: serde_json::Value = serde_json::from_slice(json).unwrap();
        assert_eq!(report["output_mode"], "preserve");
        assert_eq!(report["output_width"], 96);
        assert_eq!(report["output_height"], 96);
        assert_eq!(report["cell_pitch"], 3);
        assert_eq!(report["grid"]["width"], 32);
        let png = std::slice::from_raw_parts(ps_result_png(result), ps_result_png_len(result));
        assert_eq!(
            image::load_from_memory(png)
                .unwrap()
                .to_rgba8()
                .dimensions(),
            (96, 96)
        );
        ps_result_free(result);
    }
}
