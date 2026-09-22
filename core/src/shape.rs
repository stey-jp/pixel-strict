//! Bounded source-space probes shared by every grid candidate. A coarse grid
//! cannot obtain a perfect retention score merely by losing its LINE classes.
use crate::{Grid, color::Quantized};

const MAX_PROBES: usize = 16_384;
const MAX_WIDTH_STEPS: usize = 64;

struct Probe {
    points: [usize; 3],
    labels: [u8; 3],
    line: bool,
    silhouette: bool,
    detail: bool,
    // Source interval along the scan axis, end exclusive (used for lines).
    span: [usize; 2],
}

struct Run {
    start: usize,
    end: usize,
    label: u8,
}

#[derive(Default)]
pub(crate) struct SourceShape {
    probes: Vec<Probe>,
    seen: u64,
}

impl SourceShape {
    pub fn new(q: &Quantized, w: usize, h: usize) -> Self {
        let mut result = Self::default();
        let mut runs: Vec<Run> = Vec::new();
        for vertical in [false, true] {
            let (length, rows) = if vertical { (h, w) } else { (w, h) };
            for row in 0..rows {
                let index = |p| if vertical { p * w + row } else { row * w + p };
                runs.clear();
                let mut start = 0;
                while start < length {
                    let label = q.labels[index(start)];
                    let mut end = start + 1;
                    while end < length
                        && q.distances[label as usize][q.labels[index(end)] as usize] <= 0.0007
                    {
                        end += 1;
                    }
                    runs.push(Run { start, end, label });
                    start = end;
                }
                let point = |r: &Run| index((r.start + r.end - 1) / 2);
                for pair in runs.windows(2) {
                    let (a, b) = (&pair[0], &pair[1]);
                    if q.distances[a.label as usize][b.label as usize] <= 0.008 {
                        continue;
                    }
                    result.push(Probe {
                        points: [point(a), point(b), 0],
                        labels: [a.label, b.label, 0],
                        line: false,
                        silhouette: a.start == 0
                            || b.end == length
                            || q.colors[a.label as usize][3] != q.colors[b.label as usize][3],
                        detail: false,
                        span: [0, 0],
                    });
                }
                for triple in runs.windows(3) {
                    let (a, b, c) = (&triple[0], &triple[1], &triple[2]);
                    let width = b.end - b.start;
                    if width <= (a.end - a.start).min(c.end - c.start)
                        && q.distances[a.label as usize][b.label as usize] > 0.0007
                        && q.distances[b.label as usize][c.label as usize] > 0.0007
                        && q.distances[a.label as usize][c.label as usize]
                            < q.distances[a.label as usize][b.label as usize]
                                .min(q.distances[b.label as usize][c.label as usize])
                                * 0.25
                    {
                        let center = (b.start + b.end - 1) / 2;
                        let across = |r: usize| {
                            if vertical {
                                center * w + r
                            } else {
                                r * w + center
                            }
                        };
                        let same = |offset: isize| {
                            let r = row as isize + offset;
                            r >= 0
                                && r < rows as isize
                                && q.distances[b.label as usize]
                                    [q.labels[across(r as usize)] as usize]
                                    <= 0.0003
                        };
                        // Weak color variations require spatial support in the
                        // other dimension; random quantization specks are not lines.
                        let coherent = (same(-1) && same(1))
                            || (same(-1) && same(-2))
                            || (same(1) && same(2))
                            || (width >= 2 && (same(-1) || same(1)));
                        if q.distances[a.label as usize][b.label as usize]
                            .min(q.distances[b.label as usize][c.label as usize])
                            <= 0.008
                            && !coherent
                        {
                            continue;
                        }
                        // Compact source probes prevent coarse grids hiding a
                        // vanished small component. Single-pixel specks are excluded.
                        let detail = width >= 2
                            && width <= length / 8
                            && row >= width * 2
                            && row + width * 2 < rows
                            && [row - width * 2, row + width * 2].into_iter().all(|r| {
                                q.distances[b.label as usize][q.labels[across(r)] as usize] > 0.0007
                            });
                        result.push(Probe {
                            points: [point(a), point(b), point(c)],
                            labels: [a.label, b.label, c.label],
                            line: true,
                            silhouette: false,
                            detail,
                            span: [b.start, b.end],
                        });
                    }
                }
            }
        }
        result
    }

    fn push(&mut self, probe: Probe) {
        self.seen += 1;
        if self.probes.len() < MAX_PROBES {
            self.probes.push(probe);
        } else {
            // Fixed hash reservoir: bounded memory, no RNG or scan-order bias
            // toward the top-left. The same probes evaluate every candidate.
            let mut hash = self.seen.wrapping_mul(0x9e3779b97f4a7c15);
            hash = (hash ^ (hash >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
            hash = (hash ^ (hash >> 27)).wrapping_mul(0x94d049bb133111eb);
            let slot = ((hash ^ (hash >> 31)) % self.seen) as usize;
            if slot < MAX_PROBES {
                self.probes[slot] = probe;
            }
        }
    }

    // (silhouette, line continuity, boundary consistency, compact detail, line width).
    pub fn retention(&self, out: &[u8], q: &Quantized, g: Grid, w: usize, h: usize) -> [f64; 5] {
        let mut totals = [0usize; 4];
        let mut kept = [0usize; 4];
        let mut widths = 0.0;
        for probe in &self.probes {
            let count = if probe.line { 3 } else { 2 };
            let labels = probe.points.map(|p| {
                // Inverse of the integer intervals used by analyze, including
                // non-divisible Logical dimensions.
                let x = ((p % w + 1) * g.width as usize - 1) / w;
                let y = ((p / w + 1) * g.height as usize - 1) / h;
                out[y * g.width as usize + x] as usize
            });
            let retained = (0..count - 1).all(|i| {
                let a = probe.labels[i] as usize;
                let b = probe.labels[i + 1] as usize;
                let contrast = q.distances[a][b];
                q.distances[a][labels[i]] <= contrast * 0.25
                    && q.distances[b][labels[i + 1]] <= contrast * 0.25
                    && q.distances[labels[i]][labels[i + 1]] >= contrast * 0.25
            });
            if probe.line {
                widths += probe.width_retention(out, q, g, w, h);
            }
            for (i, enabled) in [probe.silhouette, probe.line, !probe.line, probe.detail]
                .into_iter()
                .enumerate()
            {
                if enabled {
                    totals[i] += 1;
                    kept[i] += usize::from(retained);
                }
            }
        }
        let retention: [f64; 4] = std::array::from_fn(|i| {
            if totals[i] == 0 {
                1.0
            } else {
                kept[i] as f64 / totals[i] as f64
            }
        });
        [
            retention[0],
            retention[1],
            retention[2],
            retention[3],
            if totals[1] == 0 {
                1.0
            } else {
                widths / totals[1] as f64
            },
        ]
    }
}

impl Probe {
    fn width_retention(&self, out: &[u8], q: &Quantized, g: Grid, w: usize, h: usize) -> f64 {
        let vertical = self.points[0] / w != self.points[1] / w;
        let p = self.points[1];
        let x = ((p % w + 1) * g.width as usize - 1) / w;
        let y = ((p / w + 1) * g.height as usize - 1) / h;
        let (center, length, count) = if vertical {
            (y, h, g.height as usize)
        } else {
            (x, w, g.width as usize)
        };
        let target = self.labels[1] as usize;
        let tolerance = q.distances[target][self.labels[0] as usize]
            .min(q.distances[target][self.labels[2] as usize])
            * 0.25;
        let matches = |position| {
            let i = if vertical {
                position * g.width as usize + x
            } else {
                y * g.width as usize + position
            };
            q.distances[target][out[i] as usize] <= tolerance
        };
        if !matches(center) {
            return 0.0;
        }
        let (mut start, mut end) = (center, center + 1);
        for _ in 0..MAX_WIDTH_STEPS {
            if start == 0 || !matches(start - 1) {
                break;
            }
            start -= 1;
        }
        for _ in 0..MAX_WIDTH_STEPS {
            if end == count || !matches(end) {
                break;
            }
            end += 1;
        }
        // Compare in source coordinates for both modes, including the unequal
        // integer source intervals in Logical. No output resampling is needed.
        let source_width = self.span[1] - self.span[0];
        let output_width = end * length / count - start * length / count;
        let open = (start > 0 && matches(start - 1)) || (end < count && matches(end));
        if open && output_width <= source_width {
            // At the scan cap only a lower bound is known. Do not invent a
            // thinning penalty for a wide run whose ends were not reached.
            return 1.0;
        }
        // Allow rounding/noisy boundaries on wider lines; a one-pixel line
        // must not get a free extra pixel of thickness.
        let allowance = source_width / 4;
        ((source_width.min(output_width) + allowance) as f64
            / source_width.max(output_width) as f64)
            .min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn width_detects_expansion_and_thinning_with_identical_probe_colors() {
        for vertical in [false, true] {
            let (w, h) = if vertical { (12, 24) } else { (24, 12) };
            let source = RgbaImage::from_fn(w, h, |x, y| {
                Rgba(if (9..11).contains(&if vertical { y } else { x }) {
                    [24, 28, 32, 255]
                } else {
                    [200, 184, 160, 255]
                })
            });
            let q = crate::color::quantize(&source, Some(16));
            let shape = SourceShape::new(&q, w as usize, h as usize);
            let target = q.labels[if vertical { 9 * w as usize } else { 9 }];
            let background = q.labels[0];
            for (span, expected) in [(9..11, 1.0), (8..12, 0.5), (9..10, 0.5)] {
                let out: Vec<_> = (0..q.labels.len())
                    .map(|i| {
                        let coordinate = if vertical {
                            i / w as usize
                        } else {
                            i % w as usize
                        };
                        if span.contains(&coordinate) {
                            target
                        } else {
                            background
                        }
                    })
                    .collect();
                let metrics = shape.retention(
                    &out,
                    &q,
                    Grid {
                        width: w,
                        height: h,
                    },
                    w as usize,
                    h as usize,
                );
                // Center/background checks alone accept every one of these.
                assert_eq!(metrics[1], 1.0);
                assert_eq!(metrics[4], expected);
            }
            let missing = vec![background; q.labels.len()];
            assert_eq!(
                shape.retention(
                    &missing,
                    &q,
                    Grid {
                        width: w,
                        height: h
                    },
                    w as usize,
                    h as usize
                )[4],
                0.0
            );
        }
    }

    #[test]
    fn width_uses_non_divisible_logical_intervals_on_both_axes() {
        for vertical in [false, true] {
            let (w, h, g) = if vertical {
                (
                    7,
                    11,
                    Grid {
                        width: 2,
                        height: 4,
                    },
                )
            } else {
                (
                    11,
                    7,
                    Grid {
                        width: 4,
                        height: 2,
                    },
                )
            };
            let source = RgbaImage::from_fn(w, h, |x, y| {
                Rgba(if (5..8).contains(&if vertical { y } else { x }) {
                    [24, 28, 32, 255]
                } else {
                    [0, 0, 0, 0]
                })
            });
            let q = crate::color::quantize(&source, Some(16));
            let shape = SourceShape::new(&q, w as usize, h as usize);
            let target = q.labels[if vertical { 5 * w as usize } else { 5 }];
            let out: Vec<_> = (0..g.width * g.height)
                .map(|i| {
                    if (if vertical { i / g.width } else { i % g.width }) == 2 {
                        target
                    } else {
                        q.labels[0]
                    }
                })
                .collect();
            // The third cell covers [5, 8), not a rounded uniform 11/4 pitch.
            assert_eq!(shape.retention(&out, &q, g, w as usize, h as usize)[4], 1.0);
        }
    }
}
