use image::{ImageEncoder, Rgba, RgbaImage};

pub fn encode(im: &RgbaImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            im.as_raw(),
            im.width(),
            im.height(),
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    bytes
}

pub fn house() -> RgbaImage {
    let mut im = RgbaImage::from_pixel(32, 32, Rgba([39, 51, 68, 255]));
    for y in 0..32 {
        for x in 0..32 {
            let c = if y >= 27 {
                [73, 96, 79, 255]
            } else if (7..=24).contains(&x) && (15..=26).contains(&y) {
                [195, 163, 119, 255]
            } else if (8..=15).contains(&y) && x >= 16 - (y - 7) && x <= 16 + (y - 7) {
                [137, 76, 65, 255]
            } else {
                [39, 51, 68, 255]
            };
            im.put_pixel(x, y, Rgba(c));
        }
    }
    for y in 8..14 {
        for x in 21..24 {
            im.put_pixel(x, y, Rgba([103, 76, 68, 255]));
        }
    }
    for y in 18..23 {
        for x in [10, 11, 12, 19, 20, 21] {
            im.put_pixel(x, y, Rgba([246, 207, 125, 255]));
        }
    }
    for y in 18..23 {
        for x in [11, 20] {
            im.put_pixel(x, y, Rgba([55, 53, 58, 255]));
        }
    }
    for x in [10, 12, 19, 21] {
        im.put_pixel(x, 20, Rgba([55, 53, 58, 255]));
    }
    for y in 21..27 {
        for x in 15..18 {
            im.put_pixel(x, y, Rgba([90, 72, 65, 255]));
        }
    }
    for (x, y) in [(4, 5), (27, 7), (6, 10)] {
        im.put_pixel(x, y, Rgba([239, 216, 170, 255]));
    }
    im
}

pub fn noisy(source: &RgbaImage, scale: u32) -> RgbaImage {
    RgbaImage::from_fn(source.width() * scale, source.height() * scale, |x, y| {
        let gx = x / scale;
        let gy = y / scale;
        let mut p = *source.get_pixel(gx, gy);
        let seed = x.wrapping_mul(73471) ^ y.wrapping_mul(91283) ^ 0x123456;
        let noise = ((seed % 13) as i16) - 6;
        for c in 0..3 {
            p[c] = (p[c] as i16 + noise).clamp(0, 255) as u8;
        }
        if x % scale == 0 && gx > 0 {
            let previous = source.get_pixel(gx - 1, gy);
            for c in 0..3 {
                p[c] = ((p[c] as u16 * 3 + previous[c] as u16) / 4) as u8;
            }
        }
        p
    })
}
