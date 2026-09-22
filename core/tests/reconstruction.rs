mod common;
use common::*;
use image::{Rgba, RgbaImage};
use pixelstrict_core::{Options, OutputMode, convert, grid};

fn logical() -> Options {
    Options {
        output_mode: OutputMode::Logical,
        ..Options::default()
    }
}

fn options(width: u32) -> Options {
    Options {
        grid_width: Some(width),
        ..logical()
    }
}
fn decoded(bytes: &[u8]) -> RgbaImage {
    image::load_from_memory(bytes).unwrap().to_rgba8()
}

#[test]
fn candidates_include_integer_divisors() {
    let grids = grid::candidates(1254, 1254);
    for n in [627, 418, 209, 114] {
        assert!(grids.iter().any(|g| g.width == n));
    }
}
#[test]
fn deterministic_png_and_binary_alpha() {
    let input = RgbaImage::from_fn(63, 47, |x, y| {
        Rgba([(x * 4) as u8, (y * 5) as u8, 120, ((x + y) * 3) as u8])
    });
    let bytes = encode(&input);
    let a = convert(&bytes, &options(13)).unwrap();
    let b = convert(&bytes, &options(13)).unwrap();
    assert_eq!(a.png, b.png);
    let result = decoded(&a.png);
    assert_eq!(result.dimensions(), (13, 10));
    assert!(result.pixels().all(|p| p[3] == 0 || p[3] == 255));
    assert!(a.report.palette.len() <= 32);
    assert!(
        result
            .pixels()
            .filter(|p| p[3] == 0)
            .all(|p| p.0 == [0, 0, 0, 0])
    );
}
#[test]
fn surface_noise_removed_but_contrast_detail_preserved() {
    let mut input = RgbaImage::from_pixel(12, 12, Rgba([144, 151, 162, 255]));
    input.put_pixel(5, 5, Rgba([151, 158, 169, 255]));
    input.put_pixel(2, 2, Rgba([240, 200, 80, 255]));
    let out = decoded(
        &convert(
            &encode(&input),
            &Options {
                colors: Some(32),
                smoothing: 3,
                ..options(12)
            },
        )
        .unwrap()
        .png,
    );
    assert_eq!(out.get_pixel(5, 5), out.get_pixel(6, 5));
    assert_ne!(out.get_pixel(2, 2), out.get_pixel(3, 2));
}
#[test]
fn minority_continuous_line_beats_majority() {
    let input = RgbaImage::from_fn(24, 32, |x, _| {
        if x == 9 {
            Rgba([30, 30, 40, 255])
        } else {
            Rgba([210, 180, 130, 255])
        }
    });
    let result = convert(
        &encode(&input),
        &Options {
            edge_protection: 3,
            ..options(6)
        },
    )
    .unwrap();
    let out = decoded(&result.png);
    for y in 1..7 {
        assert!(
            out.get_pixel(2, y)[0] < 80,
            "minority vertical line disappeared at {y}"
        );
    }
}
#[test]
fn diagonal_and_window_frame_survive() {
    let input = RgbaImage::from_fn(16, 16, |x, y| {
        if x == y
            || ((4..12).contains(&x) && (y == 4 || y == 11))
            || ((4..12).contains(&y) && (x == 4 || x == 11))
        {
            Rgba([24, 28, 32, 255])
        } else {
            Rgba([180, 190, 200, 255])
        }
    });
    let out = decoded(
        &convert(
            &encode(&input),
            &Options {
                smoothing: 3,
                ..options(16)
            },
        )
        .unwrap()
        .png,
    );
    for i in 0..16 {
        assert!(out.get_pixel(i, i)[0] < 80);
    }
    for i in 4..12 {
        assert!(out.get_pixel(i, 4)[0] < 80);
        assert!(out.get_pixel(4, i)[0] < 80);
    }
}
#[test]
fn auto_finds_fixture_grid_at_multiple_scales() {
    for scale in [3, 4, 6, 8] {
        let input = noisy(&house(), scale);
        let result = convert(&encode(&input), &logical()).unwrap();
        assert_eq!(
            result.report.grid.width, 32,
            "scale {scale}: {:?}",
            result.report.candidates
        );
    }
}

#[test]
fn auto_handles_non_divisible_resampling() {
    let source = noisy(&house(), 6);
    for size in [197, 1254] {
        let input =
            image::imageops::resize(&source, size, size, image::imageops::FilterType::Nearest);
        let result = convert(&encode(&input), &logical()).unwrap();
        assert_eq!(
            result.report.grid.width, 32,
            "size {size}: {:?}",
            result.report.candidates
        );
    }
}
#[test]
fn supported_limits_and_invalid_inputs() {
    assert!(convert(b"not an image", &Options::default()).is_err());
    let bytes = encode(&house());
    for opt in [
        Options {
            grid_width: Some(0),
            ..Options::default()
        },
        Options {
            grid_width: Some(33),
            ..Options::default()
        },
        Options {
            colors: Some(15),
            ..Options::default()
        },
        Options {
            smoothing: 0,
            ..Options::default()
        },
    ] {
        assert!(convert(&bytes, &opt).is_err());
    }
    assert!(serde_json::from_str::<Options>(r#"{"unexpected":1}"#).is_err());
}
#[test]
fn tiny_and_transparent_inputs() {
    for (w, h) in [(1, 1), (1, 33), (43, 1), (7, 11)] {
        let im = RgbaImage::from_pixel(w, h, Rgba([127, 200, 50, 0]));
        let out = convert(&encode(&im), &Options::default()).unwrap();
        assert!(decoded(&out.png).pixels().all(|p| p.0 == [0, 0, 0, 0]));
    }
}
#[test]
fn ffi_reports_error_and_owns_buffers() {
    use pixelstrict_core::ffi::*;
    unsafe {
        let result = ps_convert(std::ptr::null(), 0, std::ptr::null(), 0);
        let bytes = std::slice::from_raw_parts(ps_result_json(result), ps_result_json_len(result));
        assert!(String::from_utf8_lossy(bytes).contains("error"));
        assert_eq!(ps_result_png_len(result), 0);
        ps_result_free(result);
        let data = ps_alloc(8);
        assert!(!data.is_null());
        ps_dealloc(data, 8);
    }
}

#[test]
fn formats_median_and_palette_caps() {
    let source = noisy(&house(), 3);
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
    ] {
        let mut bytes = std::io::Cursor::new(Vec::new());
        let image = image::DynamicImage::ImageRgba8(source.clone());
        if format == image::ImageFormat::Jpeg {
            image.to_rgb8().write_to(&mut bytes, format).unwrap();
        } else {
            image.write_to(&mut bytes, format).unwrap();
        }
        for colors in [16, 24, 32] {
            let result = convert(
                bytes.get_ref(),
                &Options {
                    colors: Some(colors),
                    median: true,
                    ..options(32)
                },
            )
            .unwrap();
            assert!(result.report.palette.len() <= colors as usize);
            assert!(decoded(&result.png).pixels().all(|p| p[3] == 255));
        }
    }
}
