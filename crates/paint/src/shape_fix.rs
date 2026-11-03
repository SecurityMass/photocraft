//! Shape correction: a freehand stroke held still at its end becomes the line, ellipse, rectangle
//! or triangle it was meant to be. Pure geometry, without a document or a window; the shell decides
//! when to ask ([`HoldTimer`]) and redraws the [`Shape::outline`] with the same brush.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// A recognized shape, in the stroke's coordinates.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line { from: [f64; 2], to: [f64; 2] },
    Ellipse { center: [f64; 2], radii: [f64; 2], angle: f64 },
    Rectangle { corners: [[f64; 2]; 4] },
    Triangle { corners: [[f64; 2]; 3] },
}

/// Strokes smaller than this (the bounding box's diagonal, in px) stay as drawn.
const MIN_SIZE: f64 = 12.0;
/// Samples per closed outline, equally spaced along the stroke.
const SAMPLES: usize = 128;
/// A line may stray this much from its chord, as a fraction of the chord.
const LINE_STRAY: f64 = 0.04;
/// A closed shape's ends may be this far apart, as a fraction of the stroke's length.
const CLOSING_GAP: f64 = 0.18;
/// How far a stroke may be from a closed shape on average and at worst, as fractions of its size.
const MEAN_FIT: f64 = 0.045;
const WORST_FIT: f64 = 0.12;
/// Stroke length over shape perimeter: outside this it was drawn twice over, or is something else.
const LENGTH_RATIO: std::ops::RangeInclusive<f64> = 0.75..=1.3;
/// Radii this close make a circle.
const CIRCLE_RATIO: f64 = 0.88;
/// An axis this close to level is level (radians).
const LEVEL_SNAP: f64 = 8.0 * PI / 180.0;
/// Below this ratio of its radii an "ellipse" is a line drawn there and back.
const MIN_ELLIPSE_RATIO: f64 = 0.12;
/// Outline points this far apart (px), so the renderer draws straight sides and sharp corners.
const OUTLINE_STEP: f64 = 3.0;
const MAX_OUTLINE_POINTS: usize = 4096;

/// The shape `points` was meant to be; `None` keeps the stroke as drawn.
pub fn recognize(points: &[[f64; 2]]) -> Option<Shape> {
    let finite: Vec<[f64; 2]> = points.iter().copied().filter(|p| p[0].is_finite() && p[1].is_finite()).collect();
    let (min, max) = bounds(&finite)?;
    let size = dist(min, max);
    if size < MIN_SIZE {
        return None;
    }
    // The jitter of a held pointer, or of a shaky hand, adds length, not shape.
    let end = *finite.last()?;
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(finite.len());
    for p in finite {
        if pts.last().is_none_or(|l| dist(*l, p) >= size * 0.03) {
            pts.push(p);
        }
    }
    // The stroke ends where it ends, however close to the last point kept.
    if pts.last().is_some_and(|l| *l != end) {
        pts.push(end);
    }
    if pts.len() < 3 {
        return None;
    }
    let (first, last) = (*pts.first()?, *pts.last()?);
    let length = path_length(&pts);
    let chord = dist(first, last);
    if chord > 0.0 && length <= chord * LENGTH_RATIO.end() && pts.iter().all(|p| to_segment(*p, first, last) <= chord * LINE_STRAY) {
        return Some(Shape::Line { from: first, to: last });
    }
    if chord > length * CLOSING_GAP {
        return None;
    }
    let ring = resample(&pts, length);
    let drawn = path_length(&ring) + dist(*ring.last()?, *ring.first()?);
    let mut best: Option<(f64, Shape)> = None;
    for shape in [fit_ellipse(&ring), fit_rectangle(&ring), fit_triangle(&ring)].into_iter().flatten() {
        let (mean, worst) = fit_error(&shape, &ring, size);
        let ratio = drawn / shape.perimeter().max(f64::MIN_POSITIVE);
        if mean <= MEAN_FIT && worst <= WORST_FIT && LENGTH_RATIO.contains(&ratio) && best.as_ref().is_none_or(|b| mean < b.0) {
            best = Some((mean, shape));
        }
    }
    best.map(|b| b.1)
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

fn bounds(pts: &[[f64; 2]]) -> Option<([f64; 2], [f64; 2])> {
    let first = *pts.first()?;
    Some(pts.iter().fold((first, first), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])))
}

fn path_length(pts: &[[f64; 2]]) -> f64 {
    pts.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    dist(p, [a[0] + dx * t, a[1] + dy * t])
}

/// [`SAMPLES`] points equally spaced along the stroke, so slow parts don't weigh more.
fn resample(pts: &[[f64; 2]], length: f64) -> Vec<[f64; 2]> {
    let step = length / SAMPLES as f64;
    let mut out = Vec::with_capacity(SAMPLES);
    let mut walked = 0.0;
    for w in pts.windows(2) {
        let seg = dist(w[0], w[1]);
        while seg > 0.0 && out.len() < SAMPLES && (out.len() as f64) * step <= walked + seg {
            let t = ((out.len() as f64) * step - walked) / seg;
            out.push([w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t]);
        }
        walked += seg;
    }
    out
}

/// `p` in a frame turned by `angle`.
fn turned(p: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.sin_cos();
    [p[0] * c + p[1] * s, -p[0] * s + p[1] * c]
}

/// `p` from a frame turned by `angle` back to the stroke's.
fn unturned(p: [f64; 2], angle: f64) -> [f64; 2] {
    let (s, c) = angle.sin_cos();
    [p[0] * c - p[1] * s, p[0] * s + p[1] * c]
}

fn fit_ellipse(ring: &[[f64; 2]]) -> Option<Shape> {
    let n = ring.len() as f64;
    let mean = ring.iter().fold([0.0, 0.0], |m, p| [m[0] + p[0] / n, m[1] + p[1] / n]);
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for p in ring {
        let (x, y) = (p[0] - mean[0], p[1] - mean[1]);
        xx += x * x;
        yy += y * y;
        xy += x * y;
    }
    let mut angle = 0.5 * (2.0 * xy).atan2(xx - yy);
    let frame: Vec<[f64; 2]> = ring.iter().map(|p| turned(*p, angle)).collect();
    let (lo, hi) = bounds(&frame)?;
    let center = unturned([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0], angle);
    let mut radii = [(hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0];
    if radii[1] > radii[0] {
        radii.swap(0, 1);
        angle += FRAC_PI_2;
    }
    if radii[0] <= 0.0 || radii[1] / radii[0] < MIN_ELLIPSE_RATIO {
        return None;
    }
    if radii[1] / radii[0] >= CIRCLE_RATIO {
        let r = (radii[0] + radii[1]) / 2.0;
        return Some(Shape::Ellipse { center, radii: [r, r], angle: 0.0 });
    }
    let half_turn = angle.rem_euclid(PI);
    if half_turn.min(PI - half_turn) <= LEVEL_SNAP {
        angle = 0.0;
    } else if (half_turn - FRAC_PI_2).abs() <= LEVEL_SNAP {
        angle = FRAC_PI_2;
    }
    Some(Shape::Ellipse { center, radii, angle })
}

/// The `at` fraction (0..1) of `values`, which it sorts.
fn percentile(values: &mut [f64], at: f64) -> f64 {
    values.sort_by(f64::total_cmp);
    let i = (values.len().saturating_sub(1) as f64 * at).round() as usize;
    values.get(i).copied().unwrap_or(0.0)
}

fn rectangle_at(ring: &[[f64; 2]], angle: f64) -> Shape {
    let frame: Vec<[f64; 2]> = ring.iter().map(|p| turned(*p, angle)).collect();
    // A stray overshoot at a corner doesn't move a whole side.
    let mut xs: Vec<f64> = frame.iter().map(|p| p[0]).collect();
    let mut ys: Vec<f64> = frame.iter().map(|p| p[1]).collect();
    let (x0, x1) = (percentile(&mut xs, 0.02), percentile(&mut xs, 0.98));
    let (y0, y1) = (percentile(&mut ys, 0.02), percentile(&mut ys, 0.98));
    Shape::Rectangle { corners: [[x0, y0], [x1, y0], [x1, y1], [x0, y1]].map(|p| unturned(p, angle)) }
}

fn fit_rectangle(ring: &[[f64; 2]]) -> Option<Shape> {
    let (lo, hi) = bounds(ring)?;
    let size = dist(lo, hi);
    let mut best: Option<(f64, f64)> = None;
    for degrees in 0..90 {
        let angle = f64::from(degrees).to_radians();
        let (mean, _) = fit_error(&rectangle_at(ring, angle), ring, size);
        if best.is_none_or(|b| mean < b.0) {
            best = Some((mean, angle));
        }
    }
    let (_, angle) = best?;
    let snapped = if angle <= LEVEL_SNAP || FRAC_PI_2 - angle <= LEVEL_SNAP { 0.0 } else { angle };
    Some(rectangle_at(ring, snapped))
}

fn fit_triangle(ring: &[[f64; 2]]) -> Option<Shape> {
    // The two points furthest apart are corners, and the third is the furthest from their line.
    let mut pair: Option<(f64, [f64; 2], [f64; 2])> = None;
    for (i, a) in ring.iter().enumerate() {
        for b in ring.iter().skip(i + 1) {
            let d = dist(*a, *b);
            if pair.is_none_or(|p| d > p.0) {
                pair = Some((d, *a, *b));
            }
        }
    }
    let (_, a, b) = pair?;
    let line = |p: &[f64; 2]| ((b[0] - a[0]) * (a[1] - p[1]) - (a[0] - p[0]) * (b[1] - a[1])).abs();
    let c = ring.iter().copied().max_by(|p, q| line(p).total_cmp(&line(q)))?;
    Some(Shape::Triangle { corners: [a, b, c] })
}

/// How far `ring` is from `shape`: the mean and the worst, as fractions of `size`.
fn fit_error(shape: &Shape, ring: &[[f64; 2]], size: f64) -> (f64, f64) {
    let (mut sum, mut worst) = (0.0f64, 0.0f64);
    for p in ring {
        let d = shape.distance(*p);
        sum += d;
        worst = worst.max(d);
    }
    (sum / (ring.len().max(1) as f64 * size), worst / size)
}

fn polygon_distance(corners: &[[f64; 2]], p: [f64; 2]) -> f64 {
    let wrap = corners.iter().skip(1).chain(corners.first());
    corners.iter().zip(wrap).map(|(a, b)| to_segment(p, *a, *b)).fold(f64::INFINITY, f64::min)
}

fn closed(corners: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let path: Vec<[f64; 2]> = corners.iter().chain(corners.first()).copied().collect();
    dense(&path)
}

fn dense(path: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for w in path.windows(2) {
        let n = ((dist(w[0], w[1]) / OUTLINE_STEP).ceil() as usize).clamp(1, MAX_OUTLINE_POINTS);
        out.extend((0..n).map(|i| {
            let t = i as f64 / n as f64;
            [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t]
        }));
    }
    out.extend(path.last());
    out
}

impl Shape {
    /// The distance from `p` to the shape's line (for an ellipse, a close estimate).
    fn distance(&self, p: [f64; 2]) -> f64 {
        match self {
            Shape::Line { from, to } => to_segment(p, *from, *to),
            Shape::Ellipse { center, radii, angle } => {
                let q = turned([p[0] - center[0], p[1] - center[1]], *angle);
                let rho = (q[0] / radii[0]).hypot(q[1] / radii[1]);
                if rho > 0.0 { (rho - 1.0).abs() / rho * q[0].hypot(q[1]) } else { radii[1] }
            }
            Shape::Rectangle { corners } => polygon_distance(corners, p),
            Shape::Triangle { corners } => polygon_distance(corners, p),
        }
    }

    fn perimeter(&self) -> f64 {
        path_length(&self.outline())
    }

    /// The shape as a dense polyline to stroke (closed shapes end where they start).
    pub fn outline(&self) -> Vec<[f64; 2]> {
        match self {
            Shape::Line { from, to } => dense(&[*from, *to]),
            Shape::Ellipse { center, radii, angle } => {
                let n = ((TAU * radii[0].max(radii[1]) / OUTLINE_STEP).ceil() as usize).clamp(64, MAX_OUTLINE_POINTS);
                (0..=n)
                    .map(|i| {
                        let (s, c) = (i as f64 / n as f64 * TAU).sin_cos();
                        let q = unturned([radii[0] * c, radii[1] * s], *angle);
                        [center[0] + q[0], center[1] + q[1]]
                    })
                    .collect()
            }
            Shape::Rectangle { corners } => closed(corners),
            Shape::Triangle { corners } => closed(corners),
        }
    }
}

/// How long the pointer must stay still before the stroke is corrected.
pub const HOLD_MS: f64 = 500.0;

/// Tells when a pressed pointer has stayed within a small radius for [`HOLD_MS`].
#[derive(Clone, Copy, Debug, Default)]
pub struct HoldTimer {
    anchor: Option<([f64; 2], f64)>,
    fired: bool,
}

impl HoldTimer {
    /// Feed the pointer at `now_ms`; true once per hold, when it has lasted [`HOLD_MS`]. Moving
    /// further than `tolerance` from where the hold began starts a new one.
    pub fn update(&mut self, at: [f64; 2], now_ms: f64, tolerance: f64) -> bool {
        match self.anchor {
            Some((anchor, since)) if dist(anchor, at) <= tolerance => {
                let due = !self.fired && now_ms - since >= HOLD_MS;
                self.fired |= due;
                due
            }
            _ => {
                *self = Self { anchor: Some((at, now_ms)), fired: false };
                false
            }
        }
    }
}

#[cfg(test)]
mod tests;
