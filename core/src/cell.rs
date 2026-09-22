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
}

pub struct Cell {
    pub coverage: [f32; 32],
    line: [f32; 32],
    direction: [u8; 32],
    pub dominant: u8,
    pub class: Class,
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
    cells
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
    let mut output = Vec::with_capacity(cells.len());
    for (i, c) in cells.iter().enumerate() {
        let ns = neighbors(i, g);
        let neighbor_count = ns.clone().count().max(1) as f32;
        let mut best = c.dominant;
        let mut best_score = f32::NEG_INFINITY;
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
            let edge = directional_support(i, k, cells, g);
            let (coverage_weight, surface_weight, edge_weight) = match c.class {
                Class::Flat => (0.65, 1.0, 0.15),
                Class::Edge => (1.0, 0.10, 1.0),
                Class::Detail => (1.2, 0.035, 0.65),
                Class::Unknown => (1.15, 0.10, 0.55),
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
                + continuity * (0.12 + smooth * surface_weight)
                + edge * protect * edge_weight
                - isolated * smooth * surface_weight * 0.20
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
        if visited[start] || cells[start].class != Class::Flat {
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
                    && q.distances[seed][before[n] as usize] <= tolerance
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
    pub score: f64,
}

pub fn evaluate(
    cells: &[Cell],
    out: &[u8],
    q: &Quantized,
    g: Grid,
    alignment: f64,
    w: usize,
    h: usize,
) -> Metrics {
    let mut isolated = 0;
    let mut near_edges = 0;
    let mut uniform = 0;
    let mut edge_total = 0.0;
    let mut edge_kept = 0.0;
    let mut error = 0.0;
    for (i, c) in cells.iter().enumerate() {
        let color = out[i] as usize;
        let ns = neighbors(i, g);
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
        if c.class == Class::Edge {
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
    let score = alignment * 2.0 + surface_uniformity * 0.20 + edge_continuity * 0.15
        - source_error.sqrt() * 3.0
        - isolation * 0.8
        - (g.width as f64 / w as f64) * 0.06;
    Metrics {
        surface_uniformity,
        edge_continuity,
        isolated_pixels: isolated,
        source_error,
        grid_alignment: alignment,
        score,
    }
}
