//! Bounded source-space probes shared by every grid candidate. A coarse grid
//! cannot obtain a perfect retention score merely by losing its LINE classes.
use crate::{Grid, color::Quantized};

const MAX_PROBES: usize = 16_384;

struct Probe {
    points: [usize; 3],
    labels: [u8; 3],
    line: bool,
    silhouette: bool,
    detail: bool,
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

    // (silhouette retention, line retention, boundary consistency, compact detail retention).
    pub fn retention(&self, out: &[u8], q: &Quantized, g: Grid, w: usize, h: usize) -> [f64; 4] {
        let mut totals = [0usize; 4];
        let mut kept = [0usize; 4];
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
        std::array::from_fn(|i| {
            if totals[i] == 0 {
                1.0
            } else {
                kept[i] as f64 / totals[i] as f64
            }
        })
    }
}
