mod cell;
mod color;
pub mod ffi;
pub mod grid;

pub use cell::{Class, Metrics};
pub use grid::Grid;
use image::{ImageEncoder, ImageReader, RgbaImage};
use serde::{Deserialize, Serialize};
use std::{io::Cursor, time::Instant};

pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PIXELS: u64 = 16_777_216;
pub const MAX_CELLS: u64 = 262_144;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub grid_width: Option<u32>,
    pub colors: Option<u8>,
    pub smoothing: u8,
    pub edge_protection: u8,
    pub median: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            grid_width: None,
            colors: None,
            smoothing: 2,
            edge_protection: 2,
            median: false,
        }
    }
}
impl Options {
    fn validate(&self, w: u32, h: u32) -> Result<(), String> {
        if !matches!(self.colors, None | Some(16 | 24 | 32)) {
            return Err("Colors must be Auto, 16, 24 or 32".into());
        }
        if !(1..=3).contains(&self.smoothing) || !(1..=3).contains(&self.edge_protection) {
            return Err("Strength must be 1, 2 or 3".into());
        }
        if let Some(n) = self.grid_width {
            if n == 0 || n > w {
                return Err(format!(
                    "Grid width must be between 1 and source width ({w})"
                ));
            }
            let g = grid::from_width(w, h, n);
            if g.height > h || g.width as u64 * g.height as u64 > MAX_CELLS {
                return Err("Output exceeds 262144 cells; choose a smaller grid width".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateReport {
    pub grid: Grid,
    pub metrics: Metrics,
}
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub source_width: u32,
    pub source_height: u32,
    pub grid: Grid,
    pub palette: Vec<[u8; 4]>,
    pub classes: [usize; 4],
    pub candidates: Vec<CandidateReport>,
    pub processing_ms: f64,
    pub options: Options,
}
pub struct Conversion {
    pub png: Vec<u8>,
    pub report: Report,
}

fn preprocess(image: &mut RgbaImage, median: bool) {
    for p in image.pixels_mut() {
        if p[3] < 128 {
            p.0 = [0, 0, 0, 0];
        } else {
            p[3] = 255;
        }
    }
    if !median {
        return;
    }
    let source = image.clone();
    let (w, h) = source.dimensions();
    for y in 0..h {
        for x in 0..w {
            // Median only within the same opacity class, preserving silhouette topology.
            if source.get_pixel(x, y)[3] == 0 {
                continue;
            }
            let mut values = [[0u8; 9]; 3];
            let mut count = 0;
            for sy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                for sx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                    let p = source.get_pixel(sx, sy);
                    if p[3] == 0 {
                        continue;
                    }
                    for c in 0..3 {
                        values[c][count] = p[c];
                    }
                    count += 1;
                }
            }
            for (c, v) in values.iter_mut().enumerate() {
                v[..count].sort_unstable();
                image.get_pixel_mut(x, y)[c] = v[count / 2];
            }
        }
    }
}

pub fn convert(bytes: &[u8], options: &Options) -> Result<Conversion, String> {
    let start = Instant::now();
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err("Input must be between 1 byte and 64 MiB".into());
    }
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP)
    ) {
        return Err("Supported formats: PNG, JPEG, WebP".into());
    }
    let format = reader.format();
    let (w, h) = reader
        .into_dimensions()
        .map_err(|e| format!("Invalid image: {e}"))?;
    if w == 0 || h == 0 || w > 8192 || h > 8192 || w as u64 * h as u64 > MAX_PIXELS {
        return Err("Image limit: 8192 per side and 16 megapixels".into());
    }
    options.validate(w, h)?;
    let mut reader = ImageReader::new(Cursor::new(bytes));
    reader.set_format(format.unwrap());
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoded = reader
        .decode()
        .map_err(|e| format!("Decode failed: {e}"))?
        .to_rgba8();
    preprocess(&mut decoded, options.median);
    let q = color::quantize(&decoded, options.colors);
    drop(decoded);
    let (xp, yp) = grid::profiles(&q, w as usize, h as usize);
    let mut grids = if let Some(n) = options.grid_width {
        vec![grid::from_width(w, h, n)]
    } else {
        let mut all = grid::candidates(w, h);
        all.retain(|g| g.width as u64 * g.height as u64 <= MAX_CELLS);
        all.sort_by(|a, b| {
            grid::alignment(*b, &xp, &yp)
                .total_cmp(&grid::alignment(*a, &xp, &yp))
                .then_with(|| b.width.cmp(&a.width))
        });
        all.truncate(8);
        // Always keep a conservative fine-grid fallback if there is little evidence.
        if w as u64 * h as u64 <= MAX_CELLS
            && !all.contains(&Grid {
                width: w,
                height: h,
            })
        {
            all.push(Grid {
                width: w,
                height: h,
            });
        }
        all
    };
    if grids.is_empty() {
        return Err("No valid grid candidates".into());
    }
    let mut reports = Vec::new();
    let mut best: Option<(f64, Grid, Vec<u8>, [usize; 4])> = None;
    for g in grids.drain(..) {
        let cells = cell::analyze(&q, w as usize, h as usize, g);
        let out = cell::decide(&cells, &q, g, options);
        let metrics = cell::evaluate(
            &cells,
            &out,
            &q,
            g,
            grid::alignment(g, &xp, &yp),
            w as usize,
            h as usize,
        );
        let score = metrics.score;
        if best.as_ref().is_none_or(|b| {
            score > b.0 + 1e-9 || ((score - b.0).abs() <= 1e-9 && g.width > b.1.width)
        }) {
            let mut classes = [0; 4];
            for c in &cells {
                classes[match c.class {
                    Class::Flat => 0,
                    Class::Edge => 1,
                    Class::Detail => 2,
                    Class::Unknown => 3,
                }] += 1;
            }
            best = Some((score, g, out, classes));
        }
        reports.push(CandidateReport { grid: g, metrics });
    }
    reports.sort_by(|a, b| {
        b.metrics
            .score
            .total_cmp(&a.metrics.score)
            .then_with(|| b.grid.width.cmp(&a.grid.width))
    });
    let (_, grid, out, classes) = best.unwrap();
    let mut rgba = Vec::with_capacity(out.len() * 4);
    let mut used = [false; 32];
    for k in out {
        rgba.extend_from_slice(&q.colors[k as usize]);
        used[k as usize] = true;
    }
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png)
        .write_image(
            &rgba,
            grid.width,
            grid.height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    let palette = q
        .colors
        .into_iter()
        .enumerate()
        .filter_map(|(i, c)| used[i].then_some(c))
        .collect();
    Ok(Conversion {
        png,
        report: Report {
            source_width: w,
            source_height: h,
            grid,
            palette,
            classes,
            candidates: reports,
            processing_ms: start.elapsed().as_secs_f64() * 1000.0,
            options: options.clone(),
        },
    })
}
