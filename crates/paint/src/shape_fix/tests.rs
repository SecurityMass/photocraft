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

/// Triangles drawn by hand with a mouse, as the app recorded them.
const MOUSE_TRIANGLES: [&[[f64; 2]]; 2] = [
    &[[216.00, 14.00], [216.00, 15.00], [216.00, 16.00], [216.00, 17.00], [215.00, 19.00], [215.00, 21.00], [214.00, 24.00], [213.00, 27.00], [212.00, 29.00], [211.00, 32.00], [209.00, 34.00], [208.00, 37.00], [207.00, 37.00], [206.00, 40.00], [204.00, 42.00], [204.00, 43.00], [203.00, 44.00], [202.00, 45.00], [201.00, 46.00], [199.00, 48.00], [197.00, 49.00], [197.00, 50.00], [196.00, 51.00], [194.00, 52.00], [193.00, 53.00], [193.00, 54.00], [192.00, 54.00], [192.00, 55.00], [191.00, 56.00], [191.00, 57.00], [190.00, 57.00], [190.00, 58.00], [190.00, 59.00], [191.00, 59.00], [191.00, 60.00], [192.00, 60.00], [193.00, 62.00], [195.00, 63.00], [198.00, 64.00], [201.00, 64.00], [205.00, 66.00], [209.00, 66.00], [214.00, 67.00], [219.00, 67.00], [220.00, 68.00], [224.00, 69.00], [230.00, 69.00], [231.00, 69.00], [237.00, 70.00], [243.00, 71.00], [244.00, 72.00], [250.00, 72.00], [251.00, 72.00], [255.00, 73.00], [257.00, 73.00], [262.00, 74.00], [263.00, 74.00], [267.00, 75.00], [271.00, 75.00], [272.00, 75.00], [273.00, 75.00], [275.00, 75.00], [276.00, 75.00], [279.00, 75.00], [282.00, 75.00], [283.00, 75.00], [284.00, 75.00], [284.00, 74.00], [284.00, 73.00], [284.00, 72.00], [284.00, 71.00], [283.00, 70.00], [283.00, 69.00], [283.00, 68.00], [282.00, 66.00], [281.00, 66.00], [281.00, 65.00], [280.00, 64.00], [278.00, 61.00], [275.00, 59.00], [272.00, 56.00], [271.00, 56.00], [268.00, 53.00], [267.00, 53.00], [264.00, 51.00], [260.00, 48.00], [255.00, 45.00], [254.00, 45.00], [250.00, 43.00], [250.00, 42.00], [246.00, 41.00], [245.00, 41.00], [242.00, 38.00], [241.00, 38.00], [238.00, 36.00], [237.00, 36.00], [234.00, 34.00], [233.00, 34.00], [233.00, 33.00], [230.00, 32.00], [227.00, 30.00], [224.00, 29.00], [224.00, 28.00], [221.00, 27.00], [219.00, 25.00], [217.00, 24.00], [215.00, 23.00], [215.00, 22.00], [214.00, 22.00]],
    &[[293.00, 149.00], [293.00, 150.00], [293.00, 151.00], [293.00, 153.00], [293.00, 154.00], [292.00, 156.00], [292.00, 157.00], [292.00, 159.00], [291.00, 160.00], [291.00, 161.00], [290.00, 162.00], [290.00, 163.00], [290.00, 164.00], [290.00, 165.00], [289.00, 165.00], [289.00, 166.00], [289.00, 167.00], [289.00, 168.00], [288.00, 168.00], [288.00, 169.00], [288.00, 170.00], [289.00, 170.00], [290.00, 170.00], [292.00, 170.00], [293.00, 170.00], [294.00, 170.00], [296.00, 170.00], [298.00, 170.00], [300.00, 170.00], [303.00, 170.00], [305.00, 170.00], [306.00, 170.00], [307.00, 170.00], [308.00, 170.00], [309.00, 170.00], [310.00, 170.00], [311.00, 170.00], [312.00, 170.00], [312.00, 169.00], [313.00, 169.00], [314.00, 169.00], [315.00, 169.00], [315.00, 168.00], [315.00, 167.00], [315.00, 166.00], [315.00, 165.00], [314.00, 164.00], [313.00, 163.00], [312.00, 162.00], [311.00, 162.00], [311.00, 161.00], [310.00, 160.00], [310.00, 159.00], [309.00, 158.00], [308.00, 157.00], [308.00, 156.00], [308.00, 155.00], [307.00, 154.00], [306.00, 153.00], [306.00, 152.00], [305.00, 152.00], [305.00, 151.00], [305.00, 150.00], [304.00, 149.00], [303.00, 148.00], [302.00, 148.00], [302.00, 147.00], [301.00, 147.00], [300.00, 147.00], [299.00, 147.00], [299.00, 146.00], [298.00, 146.00], [297.00, 146.00], [297.00, 145.00], [296.00, 145.00]],
];

#[test]
fn triangles_drawn_with_a_mouse_are_triangles() {
    for pts in MOUSE_TRIANGLES {
        assert!(matches!(recognize(pts), Some(Shape::Triangle { .. })), "{:?}", recognize(pts));
    }
}
