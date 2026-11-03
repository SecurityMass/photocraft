use super::*;
use std::f64::consts::TAU;

/// A small deterministic generator, so a failure repeats.
struct Noise(u64);

impl Noise {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0
    }
    /// A hand's wobble: smooth drift plus a little jitter, `amount` px at most.
    fn shake(&mut self, pts: Vec<[f64; 2]>, amount: f64) -> Vec<[f64; 2]> {
        let (mut dx, mut dy) = (0.0, 0.0);
        pts.into_iter()
            .map(|p| {
                dx = (dx + self.next() * amount * 0.25).clamp(-amount * 0.7, amount * 0.7);
                dy = (dy + self.next() * amount * 0.25).clamp(-amount * 0.7, amount * 0.7);
                [p[0] + dx + self.next() * amount * 0.3, p[1] + dy + self.next() * amount * 0.3]
            })
            .collect()
    }
}

fn polyline(corners: &[[f64; 2]], per_side: usize) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for w in corners.windows(2) {
        for i in 0..per_side {
            let t = i as f64 / per_side as f64;
            out.push([w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t]);
        }
    }
    out.extend(corners.last());
    out
}

fn ellipse(c: [f64; 2], r: [f64; 2], angle: f64, n: usize) -> Vec<[f64; 2]> {
    (0..=n)
        .map(|i| {
            let a = i as f64 / n as f64 * TAU;
            let (x, y) = (r[0] * a.cos(), r[1] * a.sin());
            [c[0] + x * angle.cos() - y * angle.sin(), c[1] + x * angle.sin() + y * angle.cos()]
        })
        .collect()
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

const SEEDS: [u64; 8] = [1, 2, 3, 5, 8, 13, 21, 34];

#[test]
fn noisy_lines_are_lines_with_their_ends_kept() {
    for seed in SEEDS {
        let pts = Noise(seed).shake(polyline(&[[20.0, 30.0], [320.0, 180.0]], 80), 3.0);
        match recognize(&pts) {
            Some(Shape::Line { from, to }) => {
                assert!(close(from[0], 20.0, 6.0) && close(from[1], 30.0, 6.0), "{from:?}");
                assert!(close(to[0], 320.0, 6.0) && close(to[1], 180.0, 6.0), "{to:?}");
            }
            other => panic!("seed {seed}: {other:?}"),
        }
    }
}

#[test]
fn noisy_circles_are_perfect_circles() {
    for seed in SEEDS {
        let pts = Noise(seed).shake(ellipse([200.0, 150.0], [100.0, 100.0], 0.0, 120), 3.0);
        match recognize(&pts) {
            Some(Shape::Ellipse { center, radii, .. }) => {
                assert!(close(center[0], 200.0, 6.0) && close(center[1], 150.0, 6.0), "{center:?}");
                assert_eq!(radii[0], radii[1], "a near-circle snaps to a circle");
                assert!(close(radii[0], 100.0, 8.0), "{radii:?}");
            }
            other => panic!("seed {seed}: {other:?}"),
        }
    }
}

#[test]
fn noisy_ellipses_keep_their_proportions_and_tilt() {
    for seed in SEEDS {
        let pts = Noise(seed).shake(ellipse([300.0, 200.0], [160.0, 70.0], 0.5, 140), 3.0);
        match recognize(&pts) {
            Some(Shape::Ellipse { radii, angle, .. }) => {
                assert!(close(radii[0], 160.0, 12.0) && close(radii[1], 70.0, 10.0), "{radii:?}");
                assert!(close(angle.rem_euclid(std::f64::consts::PI), 0.5, 0.12), "{angle}");
            }
            other => panic!("seed {seed}: {other:?}"),
        }
    }
}

#[test]
fn noisy_rectangles_are_rectangles() {
    for seed in SEEDS {
        let c = [[50.0, 40.0], [350.0, 40.0], [350.0, 240.0], [50.0, 240.0], [50.0, 40.0]];
        let pts = Noise(seed).shake(polyline(&c, 40), 3.0);
        match recognize(&pts) {
            Some(Shape::Rectangle { corners }) => {
                let xs: Vec<f64> = corners.iter().map(|p| p[0]).collect();
                let ys: Vec<f64> = corners.iter().map(|p| p[1]).collect();
                let (x0, x1) = (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max));
                let (y0, y1) = (ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max));
                assert!(close(x0, 50.0, 8.0) && close(x1, 350.0, 8.0) && close(y0, 40.0, 8.0) && close(y1, 240.0, 8.0), "{corners:?}");
                // Nearly level: it snaps level, so only two different x values.
                assert!(corners.iter().all(|p| p[0] == x0 || p[0] == x1), "{corners:?}");
            }
            other => panic!("seed {seed}: {other:?}"),
        }
    }
}

#[test]
fn a_tilted_rectangle_stays_tilted() {
    let (a, b) = (0.6f64.cos(), 0.6f64.sin());
    let turn = |x: f64, y: f64| [300.0 + x * a - y * b, 300.0 + x * b + y * a];
    let c = [turn(-150.0, -80.0), turn(150.0, -80.0), turn(150.0, 80.0), turn(-150.0, 80.0), turn(-150.0, -80.0)];
    let pts = Noise(7).shake(polyline(&c, 40), 2.5);
    assert!(matches!(recognize(&pts), Some(Shape::Rectangle { .. })), "{:?}", recognize(&pts));
}

#[test]
fn noisy_triangles_are_triangles() {
    for seed in SEEDS {
        let c = [[200.0, 40.0], [360.0, 300.0], [40.0, 300.0], [200.0, 40.0]];
        let pts = Noise(seed).shake(polyline(&c, 50), 3.0);
        match recognize(&pts) {
            Some(Shape::Triangle { corners }) => {
                for want in &c[..3] {
                    assert!(corners.iter().any(|p| close(p[0], want[0], 16.0) && close(p[1], want[1], 16.0)), "seed {seed}: {corners:?}");
                }
            }
            other => panic!("seed {seed}: {other:?}"),
        }
    }
}

#[test]
fn a_shape_closed_with_a_small_gap_or_overlap_still_counts() {
    let mut gap = ellipse([200.0, 200.0], [100.0, 100.0], 0.0, 120);
    gap.truncate(112);
    assert!(matches!(recognize(&gap), Some(Shape::Ellipse { .. })));
    let mut over = ellipse([200.0, 200.0], [100.0, 100.0], 0.0, 120);
    over.extend(ellipse([200.0, 200.0], [100.0, 100.0], 0.0, 120).into_iter().take(8));
    assert!(matches!(recognize(&over), Some(Shape::Ellipse { .. })));
}

#[test]
fn scribbles_are_not_recognized() {
    for seed in SEEDS {
        let mut n = Noise(seed);
        let pts: Vec<[f64; 2]> = (0..200).map(|_| [200.0 + n.next() * 150.0, 200.0 + n.next() * 150.0]).collect();
        assert_eq!(recognize(&pts), None, "seed {seed}");
    }
}

#[test]
fn other_freehand_drawings_are_not_recognized() {
    // A zig-zag, an open arc, an S, a five-pointed star, a circle drawn twice over.
    let zig = polyline(&[[0.0, 0.0], [60.0, 80.0], [120.0, 0.0], [180.0, 80.0], [240.0, 0.0]], 15);
    assert_eq!(recognize(&zig), None);
    let arc: Vec<[f64; 2]> = (0..=60)
        .map(|i| {
            let a = i as f64 / 60.0 * 3.2;
            [200.0 + 100.0 * a.cos(), 200.0 + 100.0 * a.sin()]
        })
        .collect();
    assert_eq!(recognize(&arc), None);
    let s: Vec<[f64; 2]> = (0..=100)
        .map(|i| {
            let t = i as f64 / 100.0;
            [t * 300.0, 100.0 + 60.0 * (t * TAU).sin()]
        })
        .collect();
    assert_eq!(recognize(&s), None);
    let star: Vec<[f64; 2]> = (0..=5)
        .map(|i| {
            let a = i as f64 * 2.0 * TAU / 5.0;
            [200.0 + 150.0 * a.cos(), 200.0 + 150.0 * a.sin()]
        })
        .collect();
    assert_eq!(recognize(&polyline(&star, 25)), None);
    let twice: Vec<[f64; 2]> = ellipse([200.0, 200.0], [100.0, 100.0], 0.0, 240).into_iter().chain(ellipse([200.0, 200.0], [100.0, 100.0], 0.0, 240)).collect();
    assert_eq!(recognize(&twice), None);
}

#[test]
fn tiny_marks_and_bad_input_are_not_recognized() {
    assert_eq!(recognize(&[]), None);
    assert_eq!(recognize(&[[5.0, 5.0]]), None);
    assert_eq!(recognize(&[[5.0, 5.0]; 50]), None);
    assert_eq!(recognize(&[[5.0, 5.0], [6.0, 6.0], [7.0, 7.0]]), None);
    assert_eq!(recognize(&[[f64::NAN, 5.0], [f64::INFINITY, 6.0], [100.0, 700.0], [300.0, 900.0]]), None);
}

#[test]
fn a_tail_of_jitter_from_the_hold_does_not_change_the_answer() {
    let mut pts = polyline(&[[20.0, 30.0], [320.0, 180.0]], 80);
    let mut n = Noise(3);
    pts.extend((0..80).map(|_| [320.0 + n.next(), 180.0 + n.next()]));
    assert!(matches!(recognize(&pts), Some(Shape::Line { .. })), "{:?}", recognize(&pts));
}

#[test]
fn a_tremor_at_every_sample_is_still_a_line() {
    let pts: Vec<[f64; 2]> = (0..=50).map(|i| [20.0 + f64::from(i) * 3.2, 50.0 + if i % 2 == 0 { 1.5 } else { -1.5 }]).collect();
    assert!(matches!(recognize(&pts), Some(Shape::Line { .. })), "{:?}", recognize(&pts));
}

#[test]
fn outlines_follow_the_shape() {
    let line = Shape::Line { from: [0.0, 0.0], to: [100.0, 0.0] }.outline();
    assert_eq!((line.first().copied(), line.last().copied()), (Some([0.0, 0.0]), Some([100.0, 0.0])));
    let circle = Shape::Ellipse { center: [50.0, 50.0], radii: [40.0, 40.0], angle: 0.0 }.outline();
    assert!(circle.len() > 16);
    assert!(circle.iter().all(|p| ((p[0] - 50.0).hypot(p[1] - 50.0) - 40.0).abs() < 1e-6));
    assert!((circle[0][0] - circle[circle.len() - 1][0]).hypot(circle[0][1] - circle[circle.len() - 1][1]) < 1e-6, "closed");
    let square = Shape::Rectangle { corners: [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]] }.outline();
    assert_eq!((square.first(), square.last()), (Some(&[0.0, 0.0]), Some(&[0.0, 0.0])));
    for corner in [[10.0, 0.0], [10.0, 10.0], [0.0, 10.0]] {
        assert!(square.contains(&corner));
    }
    let tri = Shape::Triangle { corners: [[0.0, 0.0], [10.0, 0.0], [5.0, 8.0]] }.outline();
    assert_eq!(tri.first(), tri.last());
    assert!(tri.contains(&[5.0, 8.0]));
}

#[test]
fn the_hold_fires_once_after_half_a_second_still() {
    let mut h = HoldTimer::default();
    assert!(!h.update([10.0, 10.0], 0.0, 3.0));
    assert!(!h.update([11.0, 9.0], 300.0, 3.0), "jitter inside the tolerance, too early");
    assert!(h.update([10.0, 11.0], 500.0, 3.0));
    assert!(!h.update([10.0, 11.0], 900.0, 3.0), "once per hold");
}

#[test]
fn moving_restarts_the_hold_and_allows_another() {
    let mut h = HoldTimer::default();
    h.update([0.0, 0.0], 0.0, 3.0);
    assert!(!h.update([50.0, 0.0], 400.0, 3.0));
    assert!(!h.update([50.0, 0.0], 600.0, 3.0), "the clock restarted at 400 ms");
    assert!(h.update([50.0, 0.0], 900.0, 3.0));
    assert!(!h.update([90.0, 0.0], 950.0, 3.0));
    assert!(h.update([90.0, 0.0], 1450.0, 3.0), "a new hold after moving again");
}
