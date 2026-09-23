use image::{Rgba, RgbaImage};

pub fn building_like() -> RgbaImage {
    let mut im = RgbaImage::from_fn(48, 48, |x, y| {
        Rgba(if (8..40).contains(&x) && (12..40).contains(&y) {
            [144, 151, 162, 255]
        } else {
            [72, 92, 128, 255]
        })
    });
    // Low-contrast roof rim, columns, stair boundaries, and window frames.
    for y in 12..40 {
        for x in 8..40 {
            if y == 13
                || y == 37
                || x == 9
                || x == 38
                || ((14..34).contains(&x) && [20, 27].contains(&y))
                || ((20..28).contains(&y) && [14, 23, 33].contains(&x))
            {
                im.put_pixel(x, y, Rgba([160, 167, 178, 255]));
            }
        }
    }
    for (x, y) in [(5, 35), (5, 36), (6, 36), (6, 37), (7, 37), (7, 38)] {
        im.put_pixel(x, y, Rgba([48, 120, 72, 255]));
    }
    im.put_pixel(29, 32, Rgba([176, 165, 115, 255]));
    im.put_pixel(29, 33, Rgba([176, 165, 115, 255]));
    im.put_pixel(18, 32, Rgba([151, 158, 169, 255]));
    im
}

pub fn line_heavy() -> RgbaImage {
    RgbaImage::from_fn(48, 48, |x, y| {
        Rgba(
            if x == 13 || x == 34 || y == 11 || y == 36 || (x == y && (16..30).contains(&x)) {
                [24, 28, 32, 255]
            } else {
                [200, 184, 160, 255]
            },
        )
    })
}

pub fn flat_with_noise() -> RgbaImage {
    let mut im = RgbaImage::from_pixel(48, 48, Rgba([144, 151, 162, 255]));
    for (x, y) in [(4, 4), (13, 9), (35, 13), (7, 39), (40, 35)] {
        im.put_pixel(x, y, Rgba([151, 158, 169, 255]));
    }
    for y in 23..26 {
        for x in 23..25 {
            im.put_pixel(x, y, Rgba([160, 167, 178, 255]));
        }
    }
    im
}

pub fn vertical_lines(shaded: bool) -> RgbaImage {
    let mut im = RgbaImage::from_pixel(96, 96, Rgba([200, 184, 160, 255]));
    for y in 8..88 {
        for base in [21, 43, 67] {
            let start = base + u32::from(!shaded && y % 12 < 4);
            let shade = if shaded { (y / 4 % 3) as u8 * 16 } else { 0 };
            for x in start..start + 2 {
                im.put_pixel(x, y, Rgba([24 + shade, 28 + shade, 32 + shade, 255]));
            }
        }
    }
    im
}
