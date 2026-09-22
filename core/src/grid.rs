use crate::color::Quantized;
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct Grid {
    pub width: u32,
    pub height: u32,
}

pub fn from_width(width: u32, height: u32, columns: u32) -> Grid {
    Grid {
        width: columns,
        height: ((height as u64 * columns as u64 + width as u64 / 2) / width as u64).max(1) as u32,
    }
}

pub fn preserve_from_width(width: u32, height: u32, columns: u32) -> Result<Grid, String> {
    if columns == 0 || columns > width || !width.is_multiple_of(columns) {
        return Err(format!(
            "Preserve output requires equal integer square cells: grid width must divide source width ({width})"
        ));
    }
    let pitch = width / columns;
    if !height.is_multiple_of(pitch) {
        return Err(format!(
            "Preserve output requires equal integer square cells: source height ({height}) must be divisible by cell pitch ({pitch}px)"
        ));
    }
    Ok(Grid {
        width: columns,
        height: height / pitch,
    })
}

// Every divisor of gcd(width, height) is an exact square pixel pitch.
pub fn preserve_candidates(width: u32, height: u32) -> Vec<Grid> {
    let (mut a, mut b) = (width, height);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    (1..=a)
        .filter(|pitch| a.is_multiple_of(*pitch))
        .map(|pitch| Grid {
            width: width / pitch,
            height: height / pitch,
        })
        .collect()
}

// Includes exact divisors and nearby integer reconstructions for non-divisible images.
pub fn candidates(width: u32, height: u32) -> Vec<Grid> {
    let mut result = Vec::new();
    for size in 1..=128.min(width.min(height).max(1)) {
        let columns = ((width as f64 / size as f64).round() as u32).max(1);
        for n in columns.saturating_sub(1).max(1)..=(columns + 1).min(width) {
            if n != width && n > width / 2 {
                continue;
            }
            let grid = from_width(width, height, n);
            if grid.width as u64 * grid.height as u64 <= 1_048_576 && !result.contains(&grid) {
                result.push(grid);
            }
        }
    }
    result
}

pub fn profiles(q: &Quantized, w: usize, h: usize) -> (Vec<f64>, Vec<f64>) {
    let mut x = vec![0.0; w];
    let mut y = vec![0.0; h];
    for (row, row_energy) in y.iter_mut().enumerate() {
        for (col, col_energy) in x.iter_mut().enumerate() {
            let a = q.labels[row * w + col] as usize;
            if col > 0 {
                *col_energy += (q.distances[a][q.labels[row * w + col - 1] as usize].sqrt() - 0.04)
                    .max(0.0) as f64;
            }
            if row > 0 {
                *row_energy += (q.distances[a][q.labels[(row - 1) * w + col] as usize].sqrt()
                    - 0.04)
                    .max(0.0) as f64;
            }
        }
    }
    (x, y)
}

pub fn alignment(grid: Grid, x: &[f64], y: &[f64]) -> f64 {
    fn axis(p: &[f64], n: u32) -> (f64, f64) {
        let total: f64 = p.iter().sum();
        if total < 1e-9 || n <= 1 {
            return (0.0, total);
        }
        // Resampling and imperfect AI boundaries can shift transitions by a pixel.
        // Non-overlapping boundary bands tolerate that without shifting the output grid.
        let radius = usize::from(p.len() / n as usize >= 4);
        let energy: f64 = (1..n as usize)
            .map(|i| {
                let center = i * p.len() / n as usize;
                p[center - radius..=center + radius].iter().sum::<f64>()
            })
            .sum();
        let fraction = ((n - 1) as usize * (radius * 2 + 1)) as f64 / (p.len() - 1).max(1) as f64;
        // Correct for the extra boundaries afforded to finer grids.
        (
            (energy / total - fraction).max(0.0) / (1.0 - fraction).max(0.001) * total,
            total,
        )
    }
    let (a, at) = axis(x, grid.width);
    let (b, bt) = axis(y, grid.height);
    (a + b) / (at + bt).max(1e-9)
}
