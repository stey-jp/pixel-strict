use crate::{Options, color::Quantized, grid::Grid};
use serde::Serialize;

const AXES: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

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
};

fn smoothing_factor(c: &Cell) -> f32 {
    if c.silhouette != 0 {
        return BUILDING.silhouette_smoothing;
    }
    BUILDING.smoothing[c.class as usize]
}

fn protected(c: &Cell) -> bool {
    c.silhouette | c.lines | c.details != 0
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
    let mut cells = Vec::with_capacity((g.width * g.height) as usize);
    for gy in 0..g.height as usize {
        for gx in 0..g.width as usize {
            let x0 = gx * w / g.width as usize;
            let x1 = (gx + 1) * w / g.width as usize;
            let y0 = gy * h / g.height as usize;
            let y1 = (gy + 1) * h / g.height as usize;
            let mut counts = [0u32; 32];
            let mut moments = [[0.0f32; 5]; 32];
            for y in y0..y1 {
                for x in x0..x1 {
                    let k = q.labels[y * w + x] as usize;
                    counts[k] += 1;
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
    protect_lines(&mut cells, q, g);
    protect_details(&mut cells, q, g);
    cells
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

fn protect_lines(cells: &mut [Cell], q: &Quantized, g: Grid) {
    for i in 0..cells.len() {
        let mut mask = 0;
        for k in 0..q.colors.len() {
            if cells[i].coverage[k] < 0.055 {
                continue;
            }
            let subcell = cells[i].line[k] > 0.75
                && cells[i].coverage[k] <= 0.5
                && q.distances[k][cells[i].dominant as usize] > 0.008
                && directional_support(i, k, cells, g) > 0.12;
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

fn directional_support(i: usize, k: usize, cells: &[Cell], g: Grid) -> f32 {
    let c = &cells[i];
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
                    (other.coverage[k] / 0.18).min(1.0) * orientation
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

pub fn decide(cells: &[Cell], q: &Quantized, g: Grid, o: &Options) -> Vec<u8> {
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
        for k in 0..q.colors.len() {
            let delta = q.distances[c.dominant as usize][k];
            let near = delta <= tolerance;
            if c.coverage[k] < 0.055
                && !(c.class == Class::Flat
                    && near
                    && ns.clone().any(|n| cells[n].dominant as usize == k))
            {
                continue;
            }
            let continuity = ns.clone().map(|n| cells[n].coverage[k]).sum::<f32>() / neighbor_count;
            let mut edge = directional_support(i, k, cells, g);
            if c.lines & (1 << k) != 0 {
                // One-sided support preserves endpoints of classified lines.
                for (axis, &(dx, dy)) in AXES.iter().enumerate() {
                    if c.line[k] > 0.45 && c.direction[k] != axis as u8 {
                        continue;
                    }
                    for sign in [-1, 1] {
                        if let Some(n) = neighbor(i, dx * sign, dy * sign, g) {
                            edge = edge.max((cells[n].coverage[k] / 0.18).min(1.0) * 0.6);
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
                + edge * protect * edge_weight
                + shape
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
                .map(|k| directional_support(i, k, cells, g))
                .fold(0.0, f32::max) as f64;
            edge_total += potential;
            edge_kept += directional_support(i, color, cells, g) as f64;
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
    ] = source_shape.retention(out, q, g, w, h);
    let line_continuity_retention = source_lines * ratio(line_kept, line_total);
    let small_detail_retention = source_details * ratio(preserved_detail_count, detail_total);
    let corner_edge_consistency = boundaries * ratio(corner_kept, corner_total);
    let shape_score = [
        silhouette_retention,
        line_continuity_retention,
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
        small_detail_retention,
        corner_edge_consistency,
        shape_score,
        score,
    }
}
