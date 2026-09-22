use image::RgbaImage;

pub type Lab = [f32; 3];

pub fn lab(rgb: [u8; 3]) -> Lab {
    let linear = rgb.map(|v| {
        let x = v as f32 / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    });
    let [r, g, b] = linear;
    let l = (0.41222146 * r + 0.53633255 * g + 0.051445995 * b).cbrt();
    let m = (0.2119035 * r + 0.6806995 * g + 0.10739696 * b).cbrt();
    let s = (0.08830246 * r + 0.28171885 * g + 0.6299787 * b).cbrt();
    [
        0.21045426 * l + 0.7936178 * m - 0.004072047 * s,
        1.9779985 * l - 2.4285922 * m + 0.4505937 * s,
        0.025904037 * l + 0.78277177 * m - 0.80867577 * s,
    ]
}

pub fn distance(a: Lab, b: Lab) -> f32 {
    (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)
}

fn bin(p: &[u8; 4]) -> usize {
    ((p[0] as usize >> 3) << 10) | ((p[1] as usize >> 3) << 5) | (p[2] as usize >> 3)
}

pub struct Quantized {
    pub colors: Vec<[u8; 4]>,
    pub labels: Vec<u8>,
    pub distances: [[f32; 32]; 32],
}

// Fixed-order histogram + deterministic farthest-point seeds; no random state.
pub fn quantize(image: &RgbaImage, requested: Option<u8>) -> Quantized {
    let mut bins = vec![[0u64; 4]; 32768];
    let mut transparent = false;
    for p in image.pixels() {
        if p[3] == 0 {
            transparent = true;
            continue;
        }
        let b = &mut bins[bin(&p.0)];
        b[0] += 1;
        for c in 0..3 {
            b[c + 1] += p[c] as u64;
        }
    }
    let entries: Vec<_> = bins
        .iter()
        .enumerate()
        .filter(|(_, b)| b[0] > 0)
        .map(|(i, b)| {
            let rgb = [1, 2, 3].map(|c| ((b[c] + b[0] / 2) / b[0]) as u8);
            (i, b[0] as f64, rgb, lab(rgb))
        })
        .collect();
    let mut centers: Vec<Lab> = Vec::new();
    if let Some(first) = entries
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
    {
        centers.push(first.3);
    }
    let max_colors = requested.unwrap_or(32) as usize - usize::from(transparent);
    while centers.len() < max_colors.min(entries.len()) {
        let next = entries
            .iter()
            .map(|e| {
                let d = centers
                    .iter()
                    .map(|c| distance(*c, e.3))
                    .fold(f32::MAX, f32::min);
                (e, d, d as f64 * e.1.sqrt())
            })
            .max_by(|a, b| a.2.total_cmp(&b.2).then_with(|| b.0.0.cmp(&a.0.0)));
        let Some((e, d, _)) = next else { break };
        // Auto stops once remaining variation is perceptually small.
        if d < if requested.is_none() {
            0.0006
        } else {
            0.000025
        } {
            break;
        }
        centers.push(e.3);
    }
    let nearest = |p: Lab, centers: &[Lab]| -> usize {
        centers
            .iter()
            .enumerate()
            .min_by(|a, b| distance(p, *a.1).total_cmp(&distance(p, *b.1)))
            .map_or(0, |(i, _)| i)
    };
    for _ in 0..8 {
        let mut sums = vec![[0.0f64; 4]; centers.len()];
        for e in &entries {
            let s = &mut sums[nearest(e.3, &centers)];
            s[3] += e.1;
            for (c, value) in s.iter_mut().enumerate().take(3) {
                *value += e.3[c] as f64 * e.1;
            }
        }
        for (c, s) in centers.iter_mut().zip(sums) {
            if s[3] > 0.0 {
                *c = [0, 1, 2].map(|i| (s[i] / s[3]) as f32);
            }
        }
    }
    let offset = usize::from(transparent);
    let mut rgb_sums = vec![[0.0f64; 4]; centers.len()];
    let mut lookup = vec![0u8; 32768];
    for e in &entries {
        let k = nearest(e.3, &centers);
        lookup[e.0] = (k + offset) as u8;
        rgb_sums[k][3] += e.1;
        for (c, sum) in rgb_sums[k].iter_mut().enumerate().take(3) {
            *sum += e.2[c] as f64 * e.1;
        }
    }
    let mut colors = if transparent {
        vec![[0, 0, 0, 0]]
    } else {
        vec![]
    };
    for s in rgb_sums {
        colors.push(if s[3] > 0.0 {
            [
                (s[0] / s[3]).round() as u8,
                (s[1] / s[3]).round() as u8,
                (s[2] / s[3]).round() as u8,
                255,
            ]
        } else {
            [0, 0, 0, 255]
        });
    }
    let labs: Vec<_> = colors.iter().map(|c| lab([c[0], c[1], c[2]])).collect();
    let mut distances = [[0.0; 32]; 32];
    for a in 0..colors.len() {
        for b in 0..colors.len() {
            distances[a][b] = if colors[a][3] != colors[b][3] {
                1.0
            } else {
                distance(labs[a], labs[b])
            };
        }
    }
    let labels = image
        .pixels()
        .map(|p| if p[3] == 0 { 0 } else { lookup[bin(&p.0)] })
        .collect();
    Quantized {
        colors,
        labels,
        distances,
    }
}
