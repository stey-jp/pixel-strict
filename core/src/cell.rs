use crate::{Options, color::Quantized, grid::Grid};
use serde::Serialize;

const AXES: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
const STRAIGHT_CONTRAST: f32 = 0.0007;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Class {
    Flat,
    Edge,
    Detail,
    Unknown,
    Line,
}

pub struct Cell {
    pub coverage: [f32; 32],
    line: [f32; 32],
    direction: [u8; 32],
    // Colors touching left/right/top/bottom source-cell boundaries.
    borders: [u32; 4],
    // Allowed source colors on the majority side of a stable straight boundary.
    boundary_colors: u32,
    source_luma: f32,
    pub dominant: u8,
    pub class: Class,
    // Palette bitsets keep protection bounded to colors actually present in a cell.
    silhouette: u32,
    lines: u32,
    details: u32,
}

// Building-oriented defaults. Future presets can replace this balance without
// changing classification, the pipeline, or output geometry.
struct ShapeBalance {
    silhouette: f32,
    line: f32,
    detail: f32,
    smoothing: [f32; 5],
    silhouette_smoothing: f32,
    retention_weights: [f64; 4],
    candidate_weight: f64,
    refinement_penalty: f64,
    tone_error_weight: f32,
    tone_structure_weight: f32,
}
const BUILDING: ShapeBalance = ShapeBalance {
    silhouette: 0.45,
    line: 0.85,
    detail: 0.65,
    smoothing: [1.0, 0.10, 0.035, 0.10, 0.025],
    silhouette_smoothing: 0.015,
    retention_weights: [0.35, 0.30, 0.20, 0.15],
    candidate_weight: 1.25,
    refinement_penalty: 0.08,
    tone_error_weight: 0.25,
    tone_structure_weight: 0.1,
};

fn smoothing_factor(c: &Cell) -> f32 {
    if c.silhouette != 0 {
        return BUILDING.silhouette_smoothing;
    }
    BUILDING.smoothing[c.class as usize]
}

fn protected(c: &Cell) -> bool {
    c.silhouette | c.lines | c.details | c.boundary_colors != 0
}

fn neighbor(i: usize, dx: i32, dy: i32, g: Grid) -> Option<usize> {
    let x = (i % g.width as usize) as i32 + dx;
    let y = (i / g.width as usize) as i32 + dy;
    if x >= 0 && y >= 0 && x < g.width as i32 && y < g.height as i32 {
        Some(y as usize * g.width as usize + x as usize)
    } else {
        None
    }
}

fn neighbors(i: usize, g: Grid) -> impl Iterator<Item = usize> + Clone {
    (-1..=1).flat_map(move |dy| {
        (-1..=1).filter_map(move |dx| {
            if dx == 0 && dy == 0 {
                None
            } else {
                neighbor(i, dx, dy, g)
            }
        })
    })
}

pub fn analyze(q: &Quantized, w: usize, h: usize, g: Grid) -> Vec<Cell> {
    let shades = line_shades(q);
    let mut cells = Vec::with_capacity((g.width * g.height) as usize);
    for gy in 0..g.height as usize {
        for gx in 0..g.width as usize {
            let x0 = gx * w / g.width as usize;
            let x1 = (gx + 1) * w / g.width as usize;
            let y0 = gy * h / g.height as usize;
            let y1 = (gy + 1) * h / g.height as usize;
            let mut counts = [0u32; 32];
            let mut luma_sum = 0u64;
            let mut moments = [[0.0f32; 5]; 32];
            for y in y0..y1 {
                for x in x0..x1 {
                    let k = q.labels[y * w + x] as usize;
                    counts[k] += 1;
                    luma_sum += q.source_luma[y * w + x] as u64;
                    let rx = (x - x0) as f32;
                    let ry = (y - y0) as f32;
                    let m = &mut moments[k];
                    m[0] += rx;
                    m[1] += ry;
                    m[2] += rx * rx;
                    m[3] += ry * ry;
                    m[4] += rx * ry;
                }
            }
            let total = ((x1 - x0) * (y1 - y0)).max(1) as f32;
            let mut borders = [0; 4];
            if x1 - x0 > 1 || y1 - y0 > 1 {
                for y in y0..y1 {
                    borders[0] |= 1 << q.labels[y * w + x0];
                    borders[1] |= 1 << q.labels[y * w + x1 - 1];
                }
                for x in x0..x1 {
                    borders[2] |= 1 << q.labels[y0 * w + x];
                    borders[3] |= 1 << q.labels[(y1 - 1) * w + x];
                }
            }
            let coverage = counts.map(|n| n as f32 / total);
            let dominant = (0..q.colors.len())
                .max_by_key(|&k| (counts[k], std::cmp::Reverse(k)))
                .unwrap_or(0);
            let mut line = [0.0; 32];
            let mut direction = [0; 32];
            for k in 0..q.colors.len() {
                if counts[k] < 2 {
                    continue;
                }
                let m = moments[k].map(|v| v / counts[k] as f32);
                let xx = m[2] - m[0] * m[0];
                let yy = m[3] - m[1] * m[1];
                let xy = m[4] - m[0] * m[1];
                line[k] = ((xx - yy).powi(2) + 4.0 * xy * xy).sqrt() / (xx + yy).max(0.001);
                direction[k] = if xy.abs() > (xx - yy).abs() * 0.5 {
                    if xy >= 0.0 { 2 } else { 3 }
                } else if xx >= yy {
                    0
                } else {
                    1
                };
            }
            let significant = coverage.iter().filter(|&&c| c >= 0.12).count();
            let similar_coverage: f32 = coverage
                .iter()
                .enumerate()
                .filter(|(k, _)| q.distances[dominant][*k] <= 0.004)
                .map(|(_, v)| *v)
                .sum();
            let thin_contour = (0..q.colors.len()).any(|k| {
                coverage[k] >= 0.055 && line[k] > 0.75 && q.distances[dominant][k] > 0.008
            });
            let class = if thin_contour {
                Class::Edge
            } else if similar_coverage >= 0.88 {
                Class::Flat
            } else if significant <= 2 && line.iter().any(|&l| l > 0.45) {
                Class::Edge
            } else if significant >= 3 {
                Class::Detail
            } else {
                Class::Unknown
            };
            cells.push(Cell {
                coverage,
                line,
                direction,
                borders,
                boundary_colors: 0,
                source_luma: luma_sum as f32 / total,
                dominant: dominant as u8,
                class,
                silhouette: 0,
                lines: 0,
                details: 0,
            });
        }
    }
    // A pure-color cell can itself be a thin contour. Context must classify it too.
    let mut classes: Vec<_> = cells.iter().map(|c| c.class).collect();
    for (i, c) in cells.iter().enumerate() {
        if c.class != Class::Flat {
            continue;
        }
        let contrast = neighbors(i, g)
            .filter(|&n| q.distances[c.dominant as usize][cells[n].dominant as usize] > 0.008)
            .count();
        if contrast > 0 {
            classes[i] = Class::Edge;
        }
    }
    for (c, class) in cells.iter_mut().zip(classes) {
        c.class = class;
    }
    protect_silhouettes(&mut cells, q, g);
    protect_lines(&mut cells, q, g, &shades);
    protect_details(&mut cells, q, g);
    for (i, cell) in cells.iter_mut().enumerate() {
        if cell
            .coverage
            .iter()
            .enumerate()
            .any(|(k, &v)| v >= 0.055 && q.distances[k][cell.dominant as usize] > STRAIGHT_CONTRAST)
        {
            cell.boundary_colors = straight_boundary(i, cell, q, w, h, g, false);
            if cell.boundary_colors == 0 {
                cell.boundary_colors = straight_boundary(i, cell, q, w, h, g, true);
            }
        }
    }
    cells
}

fn straight_boundary(
    i: usize,
    cell: &Cell,
    q: &Quantized,
    w: usize,
    h: usize,
    g: Grid,
    source: bool,
) -> u32 {
    let (gx, gy) = (i % g.width as usize, i / g.width as usize);
    for vertical in [true, false] {
        let (column, row, width, height, columns, rows) = if vertical {
            (gx, gy, w, h, g.width as usize, g.height as usize)
        } else {
            (gy, gx, h, w, g.height as usize, g.width as usize)
        };
        let (x0, x1) = (column * width / columns, (column + 1) * width / columns);
        if x1 - x0 < 2 || column == 0 || column + 1 == columns {
            continue;
        }
        let index = |x: usize, y: usize| if vertical { y * w + x } else { x * w + y };
        let palette_at = |x, y| q.labels[index(x, y)] as usize;
        let at = |x, y| {
            if source {
                q.source_luma[index(x, y)] as usize
            } else {
                palette_at(x, y)
            }
        };
        let distance = |a: usize, b: usize| {
            if source {
                (a as f32 - b as f32).powi(2)
            } else {
                q.distances[a][b]
            }
        };
        let threshold = if source {
            24.0 * 24.0
        } else {
            STRAIGHT_CONTRAST
        };
        let mid = (row * height / rows + (row + 1) * height / rows - 1) / 2;
        // Anchor inside the neighboring faces, excluding narrow strokes with
        // the same background on both sides.
        let left = ((column - 1) * width / columns + x0 - 1) / 2;
        let right = (x1 + (column + 2) * width / columns - 1) / 2;
        let (a, b) = (at(left, mid), at(right, mid));
        if source
            && (q.colors[palette_at(left, mid)][3] == 0 || q.colors[palette_at(right, mid)][3] == 0)
        {
            continue;
        }
        let contrast = distance(a, b);
        let palette_contrast = q.distances[palette_at(left, mid)][palette_at(right, mid)];
        if contrast <= threshold || palette_contrast <= STRAIGHT_CONTRAST {
            continue;
        }
        let tolerance = contrast * 0.25;
        // Verify broad faces, rather than accidentally anchoring on another
        // nearby thin line. Palette shades are grouped against fixed anchors.
        let broad = |start: usize, end: usize, seed: usize| {
            (start..end)
                .filter(|&x| distance(seed, at(x, mid)) <= tolerance)
                .count()
                * 4
                >= (end - start) * 3
        };
        if !broad((column - 1) * width / columns, x0, a)
            || !broad(x1, (column + 2) * width / columns, b)
        {
            continue;
        }
        let agrees =
            |x, y| distance(a, at(x - 1, y)) <= tolerance && distance(b, at(x, y)) <= tolerance;
        let search = if source { x0..x1 + 1 } else { x0 + 1..x1 };
        let boundary = search
            .filter(|&x| agrees(x, mid))
            .filter(|&x| !source || distance(at(x - 1, mid), at(x, mid)) >= contrast * 0.0625)
            .max_by(|&x, &other| {
                distance(at(x - 1, mid), at(x, mid))
                    .total_cmp(&distance(at(other - 1, mid), at(other, mid)))
                    .then_with(|| other.cmp(&x))
            });
        let Some(boundary) = boundary else {
            continue;
        };
        let (mut total, mut aligned) = (0, 0);
        for r in row.saturating_sub(2)..=(row + 2).min(rows - 1) {
            let (y0, y1) = (r * height / rows, (r + 1) * height / rows);
            let mut previous = usize::MAX;
            for y in [y0, (y0 + y1 - 1) / 2, y1 - 1] {
                if y == previous {
                    continue;
                }
                previous = y;
                total += 1;
                let (row_a, row_b) = (at(left, y), at(right, y));
                let row_contrast = distance(row_a, row_b);
                let matches = |x| {
                    row_contrast > threshold
                        && distance(row_a, at(x - 1, y)) <= row_contrast * 0.25
                        && distance(row_b, at(x, y)) <= row_contrast * 0.25
                        && (!source || distance(at(x - 1, y), at(x, y)) >= row_contrast * 0.0625)
                };
                // A blended transition may move by one source pixel as its
                // brightness varies. It must still round to the same cell side.
                aligned += usize::from(if source {
                    (boundary.saturating_sub(1).max(x0)..=(boundary + 1).min(x1)).any(|x| {
                        (x - x0 >= x1 - x) == (boundary - x0 >= x1 - boundary) && matches(x)
                    })
                } else {
                    matches(boundary)
                });
            }
        }
        if total < 9 || aligned * 5 < total * 4 {
            continue;
        }
        let target = palette_at(
            if boundary - x0 >= x1 - boundary {
                left
            } else {
                right
            },
            mid,
        );
        let mask = (0..q.colors.len())
            .filter(|&k| {
                cell.coverage[k] >= 0.055
                    && q.distances[target][k] <= (palette_contrast * 0.25).min(0.004)
            })
            .fold(0, |mask, k| mask | (1 << k));
        // When the whole cell already belongs to this face, leave ordinary
        // surface merging enabled instead of locking its small color variations.
        let restricts = cell
            .coverage
            .iter()
            .enumerate()
            .any(|(k, &v)| v >= 0.055 && mask & (1 << k) == 0);
        if mask != 0 && (!source || restricts) {
            return mask;
        }
    }
    0
}

fn protect_silhouettes(cells: &mut [Cell], q: &Quantized, g: Grid) {
    // Flood border-connected surfaces against a fixed seed, never color-chain
    // through gradients. This is an exterior approximation, not segmentation.
    let mut exterior = vec![false; cells.len()];
    let mut region = Vec::new();
    for start in 0..cells.len() {
        let x = start % g.width as usize;
        let y = start / g.width as usize;
        if exterior[start]
            || (x > 0 && y > 0 && x + 1 < g.width as usize && y + 1 < g.height as usize)
        {
            continue;
        }
        exterior[start] = true;
        let seed = cells[start].dominant as usize;
        region.clear();
        region.push(start);
        let mut cursor = 0;
        while cursor < region.len() {
            let i = region[cursor];
            cursor += 1;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if let Some(n) = neighbor(i, dx, dy, g)
                    && !exterior[n]
                    && q.distances[seed][cells[n].dominant as usize] <= 0.004
                {
                    exterior[n] = true;
                    region.push(n);
                }
            }
        }
    }
    for i in 0..cells.len() {
        let dominant = cells[i].dominant as usize;
        let alpha = q.colors[dominant][3];
        let mut mask = 0;
        for k in 0..q.colors.len() {
            if cells[i].coverage[k] < 0.055 {
                continue;
            }
            let mixed_alpha = (0..q.colors.len()).any(|other| {
                cells[i].coverage[other] >= 0.055 && q.colors[other][3] != q.colors[k][3]
            });
            let boundary = neighbors(i, g).any(|n| {
                let other = cells[n].dominant as usize;
                q.colors[k][3] != q.colors[other][3]
                    || ((exterior[i] || exterior[n]) && q.distances[k][other] > 0.008)
            });
            if mixed_alpha || boundary || (q.colors[k][3] != alpha) {
                mask |= 1 << k;
            }
        }
        cells[i].silhouette = mask;
    }
}

fn protect_lines(cells: &mut [Cell], q: &Quantized, g: Grid, shades: &[u32; 32]) {
    for i in 0..cells.len() {
        let mut mask = 0;
        for k in 0..q.colors.len() {
            if cells[i].coverage[k] < 0.055 {
                continue;
            }
            let subcell = thin_direction(&cells[i], k, q)
                && cells[i].coverage[k] <= 0.5
                && q.distances[k][cells[i].dominant as usize] > 0.008
                && directional_support(i, k, cells, g, q, shades)
                    > if cells[i].line[k] > 0.75 {
                        0.12
                    } else {
                        cells[i].line[k] * 0.12
                    };
            let thin = cells[i].coverage[k] >= 0.5
                && AXES.iter().any(|&(dx, dy)| {
                    let along = [-1, 1].into_iter().any(|s| {
                        neighbor(i, dx * s, dy * s, g).is_some_and(|n| cells[n].coverage[k] >= 0.5)
                    });
                    // Contrast on both sides within two cells excludes broad faces.
                    along
                        && [-1, 1].into_iter().all(|s| {
                            (1..=2).any(|d| {
                                neighbor(i, -dy * s * d, dx * s * d, g).is_some_and(|n| {
                                    cells[n].coverage[k] < 0.18
                                        && q.distances[k][cells[n].dominant as usize] > 0.0007
                                })
                            })
                        })
                });
            if subcell || thin {
                mask |= 1 << k;
            }
        }
        cells[i].lines = mask;
        if mask != 0 {
            cells[i].class = Class::Line;
        }
    }
    // Join turns and T-junctions from the original line map. A corner lacks
    // two-sided perpendicular contrast, but must not be treated as flat noise.
    let lines: Vec<_> = cells.iter().map(|c| c.lines).collect();
    for (i, cell) in cells.iter_mut().enumerate() {
        for k in 0..q.colors.len() {
            if cell.coverage[k] < 0.18 {
                continue;
            }
            let axes = AXES
                .iter()
                .filter(|&&(dx, dy)| {
                    [-1, 1].into_iter().any(|s| {
                        neighbor(i, dx * s, dy * s, g).is_some_and(|n| lines[n] & (1 << k) != 0)
                    })
                })
                .count();
            if axes >= 2 {
                cell.lines |= 1 << k;
                cell.class = Class::Line;
            }
        }
    }
}

fn protect_details(cells: &mut [Cell], q: &Quantized, g: Grid) {
    // Group near-identical dominant shades before measuring component size;
    // palette fragments of a noisy face are not separate small objects.
    let mut seen = vec![false; cells.len()];
    let mut region = Vec::new();
    for start in 0..cells.len() {
        if seen[start] {
            continue;
        }
        let seed = cells[start].dominant as usize;
        seen[start] = true;
        region.clear();
        region.push(start);
        let mut cursor = 0;
        while cursor < region.len() {
            let i = region[cursor];
            cursor += 1;
            for n in neighbors(i, g) {
                if !seen[n] && q.distances[seed][cells[n].dominant as usize] <= 0.0007 {
                    seen[n] = true;
                    region.push(n);
                }
            }
        }
        if (2..=6).contains(&region.len()) {
            for &i in &region {
                cells[i].details |= 1 << cells[i].dominant;
                if cells[i].class != Class::Line {
                    cells[i].class = Class::Detail;
                }
            }
        }
    }
    // One 8-connected traversal per present palette label. No per-color image
    // buffers, and singletons deliberately remain eligible for surface cleanup.
    let mut visited = vec![0u32; cells.len()];
    for k in 0..q.colors.len() {
        let bit = 1 << k;
        let present = |c: &Cell| c.coverage[k] >= 0.18;
        for start in 0..cells.len() {
            if visited[start] & bit != 0 || !present(&cells[start]) {
                continue;
            }
            region.clear();
            region.push(start);
            visited[start] |= bit;
            let mut cursor = 0;
            while cursor < region.len() {
                let i = region[cursor];
                cursor += 1;
                for n in neighbors(i, g) {
                    if visited[n] & bit == 0 && present(&cells[n]) {
                        visited[n] |= bit;
                        region.push(n);
                    }
                }
            }
            if !(2..=6).contains(&region.len()) {
                continue;
            }
            let contrast = region.iter().any(|&i| {
                neighbors(i, g).any(|n| {
                    cells[n].coverage[k] < 0.18
                        && q.distances[k][cells[n].dominant as usize] > 0.0003
                })
            });
            if contrast {
                for &i in &region {
                    if q.distances[k][cells[i].dominant as usize] <= 0.008 {
                        continue;
                    }
                    cells[i].details |= bit;
                    if cells[i].class != Class::Line {
                        cells[i].class = Class::Detail;
                    }
                }
            }
        }
    }
}

fn labels(mut mask: u32) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        if mask == 0 {
            return None;
        }
        let k = mask.trailing_zeros() as usize;
        mask &= mask - 1;
        Some(k)
    })
}

fn line_shades(q: &Quantized) -> [u32; 32] {
    std::array::from_fn(|k| {
        (0..q.colors.len())
            .filter(|&other| q.distances[k][other] <= 0.02)
            .fold(0, |mask, other| mask | (1 << other))
    })
}

fn coverage(c: &Cell, mask: u32) -> f32 {
    labels(mask).map(|k| c.coverage[k]).sum()
}

fn supporting_coverage(c: &Cell, k: usize, mask: u32, axis: u8) -> f32 {
    // Fall back across a shade change, rather than boosting colors already
    // represented in the neighboring cell (e.g. mixed boundary pixels).
    if c.coverage[k] > 0.0 {
        return c.coverage[k];
    }
    let amount = labels(mask)
        .filter(|&label| c.line[label] >= 0.60 && c.direction[label] == axis)
        .map(|label| c.coverage[label])
        .sum::<f32>();
    // A different broad face is not evidence for a minority line color.
    if amount <= 0.5 { amount } else { 0.0 }
}

fn thin_direction(c: &Cell, k: usize, q: &Quantized) -> bool {
    // Half-cell-width straight strokes have lower anisotropy. Accept those
    // only at strong contrast so blended face edges are not promoted to lines.
    c.line[k] > 0.75
        || (c.direction[k] < 2 && c.line[k] >= 0.60 && q.distances[k][c.dominant as usize] > 0.05)
}

fn contrasting_shades(k: usize, dominant: usize, q: &Quantized, shades: &[u32; 32]) -> u32 {
    let tolerance = q.distances[k][dominant] * 0.125;
    labels(shades[k])
        .filter(|&other| q.distances[k][other] <= tolerance)
        .fold(0, |mask, other| mask | (1 << other))
}

fn support_mask(c: &Cell, k: usize, q: &Quantized, shades: &[u32; 32]) -> u32 {
    // Similar shades may support a high-contrast subcell contour, never a
    // low-contrast face or a different opacity. Selection still uses real colors.
    if c.coverage[k] <= 0.5
        && thin_direction(c, k, q)
        && q.distances[k][c.dominant as usize] > 0.008
    {
        contrasting_shades(k, c.dominant as usize, q, shades)
    } else {
        1 << k
    }
}

fn directional_support(
    i: usize,
    k: usize,
    cells: &[Cell],
    g: Grid,
    q: &Quantized,
    shades: &[u32; 32],
) -> f32 {
    let c = &cells[i];
    let mask = support_mask(c, k, q, shades);
    AXES.iter()
        .enumerate()
        .map(|(axis, &(dx, dy))| {
            let support = |sign| {
                neighbor(i, dx * sign, dy * sign, g).map_or(0.0, |n| {
                    let other = &cells[n];
                    // Subcell lines must agree on orientation; solid cells also support a contour.
                    let orientation = if other.line[k] > 0.5 && other.direction[k] != axis as u8 {
                        0.25
                    } else {
                        1.0
                    };
                    (supporting_coverage(other, k, mask, axis as u8) / 0.18).min(1.0) * orientation
                })
            };
            let a = support(-1);
            let b = support(1);
            let own = if c.line[k] > 0.45 {
                if c.direction[k] == axis as u8 {
                    c.line[k]
                } else {
                    0.0
                }
            } else {
                0.35
            };
            own * (a.min(b) * 0.85 + a.max(b) * 0.15)
        })
        .fold(0.0, f32::max)
}

fn straight_spill(
    i: usize,
    k: usize,
    cells: &[Cell],
    g: Grid,
    q: &Quantized,
    shades: &[u32; 32],
) -> bool {
    let c = &cells[i];
    let axis = c.direction[k] as usize;
    if axis > 1 || c.line[k] < 0.90 || q.distances[k][c.dominant as usize] <= 0.008 {
        return false;
    }
    let mask = contrasting_shades(k, c.dominant as usize, q, shades);
    if coverage(c, mask) > 0.5 {
        return false;
    }
    let (dx, dy) = AXES[axis];
    let (px, py) = (dy, dx);
    for sign in [-1, 1] {
        let Some(j) = neighbor(i, px * sign, py * sign, g) else {
            continue;
        };
        let side = if axis == 1 { 0 } else { 2 } + usize::from(sign > 0);
        // Only consolidate fragments actually touching across the same border.
        if c.borders[side] & mask == 0 || cells[j].borders[side ^ 1] & mask == 0 {
            continue;
        }
        let mut sums = [0.0; 2];
        let mut rows = 0;
        let mut joined = 0;
        let mut straight = true;
        for offset in -2..=2 {
            let (Some(a), Some(b)) = (
                neighbor(i, dx * offset, dy * offset, g),
                neighbor(j, dx * offset, dy * offset, g),
            ) else {
                continue;
            };
            let pair = [&cells[a], &cells[b]];
            let amounts = pair.map(|cell| coverage(cell, mask));
            if amounts[0] + amounts[1] < 0.055 {
                continue;
            }
            // Broad lines, turns and crossings keep their original decisions.
            if amounts[0] + amounts[1] > 0.75
                || pair.iter().zip(amounts).any(|(cell, amount)| {
                    amount >= 0.055
                        && !labels(mask).any(|label| {
                            cell.coverage[label] >= 0.055
                                && cell.line[label] >= 0.60
                                && cell.direction[label] as usize == axis
                        })
                })
            {
                straight = false;
                break;
            }
            rows += 1;
            joined += usize::from(
                pair[0].borders[side] & mask != 0 && pair[1].borders[side ^ 1] & mask != 0,
            );
            for (sum, amount) in sums.iter_mut().zip(amounts) {
                *sum += amount;
            }
        }
        if straight
            && rows >= 3
            && joined >= 2
            && (sums[1] > sums[0] || (sums[1] == sums[0] && j < i))
        {
            return true;
        }
    }
    false
}

pub fn decide(cells: &[Cell], q: &Quantized, g: Grid, o: &Options) -> Vec<u8> {
    let shades = line_shades(q);
    let luma: Vec<_> = q
        .colors
        .iter()
        .map(|c| (77.0 * c[0] as f32 + 150.0 * c[1] as f32 + 29.0 * c[2] as f32) / 256.0)
        .collect();
    let smooth = [0.0, 0.45, 0.85, 1.2][o.smoothing as usize];
    let protect = [0.0, 0.55, 0.95, 1.35][o.edge_protection as usize];
    let tolerance = [0.0, 0.0007, 0.0018, 0.004][o.smoothing as usize];
    let shape = [0.0, 0.55, 1.0, 1.45][o.shape_protection as usize];
    let mut output = Vec::with_capacity(cells.len());
    for (i, c) in cells.iter().enumerate() {
        let ns = neighbors(i, g);
        let neighbor_count = ns.clone().count().max(1) as f32;
        let mut best = c.dominant;
        let mut best_score = f32::NEG_INFINITY;
        let effective_smoothing = smooth * smoothing_factor(c);
        let tone = coherent_tone(i, cells, q, g);
        let structure_weight = if tone.is_some() {
            BUILDING.tone_structure_weight
        } else {
            1.0
        };
        for (k, &candidate_luma) in luma.iter().enumerate() {
            if c.boundary_colors != 0 && c.boundary_colors & (1 << k) == 0 {
                continue;
            }
            let delta = q.distances[c.dominant as usize][k];
            let near = delta <= tolerance;
            if c.coverage[k] < 0.055
                && !(c.class == Class::Flat
                    && near
                    && ns.clone().any(|n| cells[n].dominant as usize == k))
            {
                continue;
            }
            if c.boundary_colors == 0 && straight_spill(i, k, cells, g, q, &shades) {
                continue;
            }
            let continuity = ns.clone().map(|n| cells[n].coverage[k]).sum::<f32>() / neighbor_count;
            let mut edge = directional_support(i, k, cells, g, q, &shades);
            if c.lines & (1 << k) != 0 {
                // One-sided support preserves endpoints of classified lines.
                for (axis, &(dx, dy)) in AXES.iter().enumerate() {
                    if c.line[k] > 0.45 && c.direction[k] != axis as u8 {
                        continue;
                    }
                    for sign in [-1, 1] {
                        if let Some(n) = neighbor(i, dx * sign, dy * sign, g) {
                            edge = edge.max(
                                (supporting_coverage(
                                    &cells[n],
                                    k,
                                    support_mask(c, k, q, &shades),
                                    axis as u8,
                                ) / 0.18)
                                    .min(1.0)
                                    * 0.6,
                            );
                        }
                    }
                }
            }
            let (coverage_weight, edge_weight) = match c.class {
                Class::Flat => (0.65, 0.15),
                Class::Edge => (1.0, 1.0),
                Class::Line => (1.0, 1.0),
                Class::Detail => (1.2, 0.65),
                Class::Unknown => (1.15, 0.55),
            };
            let isolated = if continuity < 0.01 && edge < 0.1 {
                1.0
            } else {
                0.0
            };
            let variation = if near {
                delta.sqrt() * 2.0
            } else {
                delta.sqrt() * 0.35
            };
            let score = c.coverage[k] * coverage_weight
                + continuity * (0.12 + effective_smoothing)
                + edge * protect * edge_weight * structure_weight
                + shape
                    * structure_weight
                    * (if c.lines & (1 << k) != 0 {
                        BUILDING.line * edge
                    } else {
                        0.0
                    } + if c.silhouette & (1 << k) != 0 {
                        BUILDING.silhouette * (c.coverage[k] + edge).min(1.0)
                    } else {
                        0.0
                    } + if c.details & (1 << k) != 0 {
                        BUILDING.detail * if c.silhouette != 0 { 1.2 } else { 1.0 }
                    } else {
                        0.0
                    })
                - isolated * effective_smoothing * 0.20
                - tone.map_or(0.0, |value| {
                    (candidate_luma - value).abs() * BUILDING.tone_error_weight
                })
                - variation;
            if score > best_score {
                best_score = score;
                best = k as u8;
            }
        }
        output.push(best);
    }
    // Merge only connected FLAT regions, bounded against a fixed seed color.
    // No in-place neighborhood updates: traversal order cannot propagate new colors.
    let before = output.clone();
    let mut visited = vec![false; cells.len()];
    for start in 0..cells.len() {
        if visited[start] || cells[start].class != Class::Flat || protected(&cells[start]) {
            continue;
        }
        visited[start] = true;
        let seed = before[start] as usize;
        let mut region = vec![start];
        let mut cursor = 0;
        let mut votes = [0u32; 32];
        while cursor < region.len() {
            let i = region[cursor];
            cursor += 1;
            votes[before[i] as usize] += 1;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                if let Some(n) = neighbor(i, dx, dy, g)
                    && !visited[n]
                    && cells[n].class != Class::Detail
                    && cells[n].lines | cells[n].details == 0
                    && cells[n].boundary_colors == 0
                    // Exterior boundaries allow only much closer shades of the
                    // same face; opacity/contrast boundaries cannot be merged.
                    && q.distances[seed][before[n] as usize]
                        <= tolerance * if cells[n].silhouette != 0 { 0.2 } else { 1.0 }
                    && cells[n]
                        .coverage
                        .iter()
                        .enumerate()
                        .filter(|(k, _)| q.distances[seed][*k] <= tolerance)
                        .map(|(_, v)| *v)
                        .sum::<f32>()
                        >= 0.88
                {
                    visited[n] = true;
                    region.push(n);
                }
            }
        }
        let winner = (0..q.colors.len())
            .max_by_key(|&k| (votes[k], std::cmp::Reverse(k)))
            .unwrap_or(seed) as u8;
        for i in region {
            output[i] = winner;
        }
    }
    output
}

// Three competing shades of a continuous stroke must not receive more weight
// than the source brightness merely because one forms a thin stripe in a cell.
fn coherent_tone(i: usize, cells: &[Cell], q: &Quantized, g: Grid) -> Option<f32> {
    let c = &cells[i];
    if c.boundary_colors != 0 || c.details != 0 || c.coverage[c.dominant as usize] > 0.75 {
        return None;
    }
    let mask: u32 = (0..q.colors.len())
        .filter(|&k| c.coverage[k] >= 0.1)
        .fold(0, |mask, k| mask | (1 << k));
    if mask.count_ones() < 3
        || coverage(c, mask) < 0.88
        || (0..q.colors.len()).any(|k| c.coverage[k] > 0.0 && q.colors[k][3] == 0)
        || labels(mask)
            .any(|a| q.colors[a][3] == 0 || labels(mask).any(|b| q.distances[a][b] > 0.018))
    {
        return None;
    }
    let family = (0..q.colors.len())
        .filter(|&k| q.distances[c.dominant as usize][k] <= 0.018)
        .fold(0, |mask, k| mask | (1 << k));
    for (axis, &(dx, dy)) in AXES[..2].iter().enumerate() {
        if !labels(mask).any(|k| c.line[k] >= 0.6 && c.direction[k] == axis as u8) {
            continue;
        }
        let mut total = c.source_luma;
        let mut ambiguous = 1;
        let mut count = 1;
        let mut stopped = [false; 2];
        // Keep a centered five-cell window when possible. At an endpoint or a
        // lighting step, seek the missing support on the same uninterrupted side.
        // Stop on a brightness/direction change; never skip cells to collect votes.
        'support: for distance in 1..=4 {
            for (side, sign) in [-1, 1].into_iter().enumerate() {
                if stopped[side] {
                    continue;
                }
                let Some(n) = neighbor(i, dx * sign * distance, dy * sign * distance, g) else {
                    stopped[side] = true;
                    continue;
                };
                let other = &cells[n];
                if (other.source_luma - c.source_luma).abs() > 6.0
                    || coverage(other, family) < 0.88
                    || !labels(family).any(|k| {
                        other.coverage[k] >= 0.1
                            && other.line[k] >= 0.6
                            && other.direction[k] == axis as u8
                    })
                {
                    stopped[side] = true;
                    continue;
                }
                total += other.source_luma;
                ambiguous += usize::from(
                    other.boundary_colors == 0
                        && other.details == 0
                        && labels(family).filter(|&k| other.coverage[k] >= 0.1).count() >= 3,
                );
                count += 1;
                if count == 5 {
                    break 'support;
                }
            }
        }
        if count == 5 && ambiguous >= 3 {
            return Some(total / 5.0);
        }
    }
    None
}

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub surface_uniformity: f64,
    pub edge_continuity: f64,
    pub isolated_pixels: usize,
    pub source_error: f64,
    pub grid_alignment: f64,
    pub silhouette_count: usize,
    pub preserved_detail_count: usize,
    pub silhouette_retention: f64,
    pub line_continuity_retention: f64,
    pub line_width_retention: f64,
    pub small_detail_retention: f64,
    pub corner_edge_consistency: f64,
    pub shape_score: f64,
    pub score: f64,
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate(
    cells: &[Cell],
    out: &[u8],
    q: &Quantized,
    g: Grid,
    alignment: f64,
    w: usize,
    h: usize,
    source_shape: &crate::shape::SourceShape,
) -> Metrics {
    let shades = line_shades(q);
    let mut isolated = 0;
    let mut near_edges = 0;
    let mut uniform = 0;
    let mut edge_total = 0.0;
    let mut edge_kept = 0.0;
    let mut error = 0.0;
    let mut silhouette_count = 0;
    let mut detail_total = 0;
    let mut preserved_detail_count = 0;
    let mut line_total = 0;
    let mut line_kept = 0;
    let mut corner_total = 0;
    let mut corner_kept = 0;
    for (i, c) in cells.iter().enumerate() {
        let color = out[i] as usize;
        let ns = neighbors(i, g);
        silhouette_count += usize::from(c.silhouette != 0);
        if c.details != 0 {
            detail_total += 1;
            preserved_detail_count += usize::from(c.details & (1 << color) != 0);
        }
        if c.lines != 0 {
            // Check decided neighbors, not just support in the source array.
            for n in ns
                .clone()
                .filter(|&n| n > i && cells[n].lines & c.lines != 0)
            {
                line_total += 1;
                line_kept += usize::from(out[n] == out[i] && c.lines & (1 << color) != 0);
            }
        }
        for &(dx, dy) in &[(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            if let (Some(a), Some(b)) = (neighbor(i, dx, 0, g), neighbor(i, 0, dy, g)) {
                let contrast_a = q.distances[c.dominant as usize][cells[a].dominant as usize];
                let contrast_b = q.distances[c.dominant as usize][cells[b].dominant as usize];
                if contrast_a > 0.008 && contrast_b > 0.008 {
                    corner_total += 1;
                    corner_kept += usize::from(
                        q.distances[color][out[a] as usize] > contrast_a * 0.25
                            && q.distances[color][out[b] as usize] > contrast_b * 0.25
                            && q.distances[c.dominant as usize][color]
                                < contrast_a.min(contrast_b) * 0.25,
                    );
                }
            }
        }
        if ns.clone().next().is_some() && ns.clone().all(|n| out[n] != out[i]) {
            isolated += 1;
        }
        for n in ns {
            if q.distances[color][out[n] as usize] < 0.004 {
                near_edges += 1;
                if out[i] == out[n] {
                    uniform += 1;
                }
            }
        }
        if matches!(c.class, Class::Edge | Class::Line) {
            let potential = (0..q.colors.len())
                .filter(|&k| c.coverage[k] >= 0.055)
                .map(|k| directional_support(i, k, cells, g, q, &shades))
                .fold(0.0, f32::max) as f64;
            edge_total += potential;
            edge_kept += directional_support(i, color, cells, g, q, &shades) as f64;
        }
        let x = i % g.width as usize;
        let y = i / g.width as usize;
        let area = ((x + 1) * w / g.width as usize - x * w / g.width as usize)
            * ((y + 1) * h / g.height as usize - y * h / g.height as usize);
        error += c
            .coverage
            .iter()
            .enumerate()
            .map(|(k, &v)| v as f64 * q.distances[k][color] as f64)
            .sum::<f64>()
            * area as f64;
    }
    let source_error = error / (w * h).max(1) as f64;
    let surface_uniformity = if near_edges == 0 {
        1.0
    } else {
        uniform as f64 / near_edges as f64
    };
    let edge_continuity = if edge_total < 1e-9 {
        1.0
    } else {
        (edge_kept / edge_total).min(1.0)
    };
    let isolation = isolated as f64 / out.len().max(1) as f64;
    let ratio = |kept: usize, total: usize| {
        if total == 0 {
            1.0
        } else {
            kept as f64 / total as f64
        }
    };
    let [
        silhouette_retention,
        source_lines,
        boundaries,
        source_details,
        line_width_retention,
    ] = source_shape.retention(out, q, g, w, h);
    let line_continuity_retention = source_lines * ratio(line_kept, line_total);
    let small_detail_retention = source_details * ratio(preserved_detail_count, detail_total);
    let corner_edge_consistency = boundaries * ratio(corner_kept, corner_total);
    let shape_score = [
        silhouette_retention,
        line_continuity_retention * line_width_retention,
        small_detail_retention,
        corner_edge_consistency,
    ]
    .into_iter()
    .zip(BUILDING.retention_weights)
    .map(|(value, weight)| value * weight)
    .sum::<f64>();
    // Periodic alignment is weak evidence if a shape family is mostly lost.
    let alignment_credit = (2.0
        * silhouette_retention
            .min(line_continuity_retention)
            .min(line_width_retention)
            .min(small_detail_retention)
            .min(corner_edge_consistency))
    .clamp(0.1, 1.0);
    // Constant cost per doubling, independent of source upscaling.
    let refinement = (g.width as f64).log2() * BUILDING.refinement_penalty;
    let score = alignment * alignment_credit * 2.0
        + surface_uniformity * 0.20
        + edge_continuity * 0.15
        + shape_score * BUILDING.candidate_weight
        - source_error.sqrt() * 3.0
        - isolation * 0.8
        - refinement;
    Metrics {
        surface_uniformity,
        edge_continuity,
        isolated_pixels: isolated,
        source_error,
        grid_alignment: alignment,
        silhouette_count,
        preserved_detail_count,
        silhouette_retention,
        line_continuity_retention,
        line_width_retention,
        small_detail_retention,
        corner_edge_consistency,
        shape_score,
        score,
    }
}
