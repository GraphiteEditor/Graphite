// Exact convex hull of a closed Bézier spline.
//
// Architecture: support-function envelope. Every boundary feature (convex
// arc, corner, line segment) contributes a support function h(θ) = ⟨p(θ), n(θ)⟩
// over an interval of outward-normal angles θ. The hull is the upper envelope
// of these functions over [0, 2π); envelope pieces are hull arcs and envelope
// transitions are bitangent line segments.
//
// Each curved piece contributes *two* candidates (one per normal side), so no
// global orientation or convexity classification is needed — wrong-side
// candidates simply never win the envelope. This makes the algorithm robust
// to input orientation and self-intersecting curves.

use kurbo::{CubicBez, ParamCurve, ParamCurveDeriv, Point, Vec2};
use poly_cool::PolyDyn;
use std::f64::consts::PI;

const TOL: f64 = 1e-10;
/// Angular tolerance (radians) for interval membership and breakpoint dedup.
const ANG_EPS: f64 = 1e-9;
/// Angular tolerance for matching normals of a bitangent solution.
const ANG_MATCH: f64 = 1e-6;

const TAU: f64 = 2.0 * PI;

// ─── Public Types ───

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PieceKind {
    /// Monotone-curvature curved arc.
    Arc,
    /// Straight line segment.
    Line,
    /// Zero-length corner at a tangent-discontinuous junction.
    Corner,
}

/// A boundary piece produced by decomposition: monotone arc, line, or corner.
#[derive(Clone, Debug)]
pub struct MonotoneArc {
    pub bezier: CubicBez,
    pub kind: PieceKind,
    /// Tangent angle at start/end, unwrapped along the piece
    /// (theta_end - theta_start = signed tangent turn).
    pub theta_start: f64,
    pub theta_end: f64,
    /// Index of the original spline segment this piece came from.
    pub original_segment: usize,
    /// Parameter range within the original segment.
    pub original_t_end: f64,
}

#[derive(Clone, Debug)]
pub enum HullSegment {
    /// A portion of piece `arc_index` on the hull. `t_start` may exceed
    /// `t_end` when the hull traverses the piece against its parameterization.
    Arc {
        arc_index: usize,
        t_start: f64,
        t_end: f64,
    },
    /// A bitangent line segment bridging between two hull contacts.
    Line { start: Point, end: Point },
}

// ─── Basic Geometry Helpers ───

fn is_collinear(cb: &CubicBez) -> bool {
    let d1 = cb.p1 - cb.p0;
    let d2 = cb.p2 - cb.p0;
    let d3 = cb.p3 - cb.p0;
    let len = d1.hypot().max(d2.hypot()).max(d3.hypot()).max(1e-15);
    (d1.x * d2.y - d1.y * d2.x).abs() < 1e-8 * len
        && (d1.x * d3.y - d1.y * d3.x).abs() < 1e-8 * len
}

fn is_degenerate_point(cb: &CubicBez) -> bool {
    let tol = 1e-12;
    (cb.p0 - cb.p3).hypot() < tol
        && (cb.p0 - cb.p1).hypot() < tol
        && (cb.p0 - cb.p2).hypot() < tol
}

/// Tangent direction at the start of a cubic, robust to degenerate
/// parameterizations (p0 == p1 etc.): first nonzero control-point difference.
fn start_tangent_dir(cb: &CubicBez) -> Vec2 {
    for v in [cb.p1 - cb.p0, cb.p2 - cb.p0, cb.p3 - cb.p0] {
        if v.hypot() > 1e-12 {
            return v;
        }
    }
    Vec2::new(1.0, 0.0)
}

/// Tangent direction at the end of a cubic, robust to degenerate
/// parameterizations.
fn end_tangent_dir(cb: &CubicBez) -> Vec2 {
    for v in [cb.p3 - cb.p2, cb.p3 - cb.p1, cb.p3 - cb.p0] {
        if v.hypot() > 1e-12 {
            return v;
        }
    }
    Vec2::new(1.0, 0.0)
}

/// Tangent direction at parameter t, falling back to control-point geometry
/// near degenerate endpoints.
fn tangent_dir_at(cb: &CubicBez, t: f64) -> Vec2 {
    let d = cb.deriv().eval(t).to_vec2();
    if d.hypot() > 1e-9 * chord_scale(cb) {
        return d;
    }
    if t < 0.5 {
        start_tangent_dir(cb)
    } else {
        end_tangent_dir(cb)
    }
}

fn chord_scale(cb: &CubicBez) -> f64 {
    (cb.p1 - cb.p0)
        .hypot()
        .max((cb.p2 - cb.p0).hypot())
        .max((cb.p3 - cb.p0).hypot())
        .max(1e-15)
}

fn unwrap_angle(angle: f64, reference: f64) -> f64 {
    let mut a = angle;
    while a - reference > PI {
        a -= TAU;
    }
    while a - reference < -PI {
        a += TAU;
    }
    a
}

fn normalize_angle(a: f64) -> f64 {
    let mut x = a % TAU;
    if x < 0.0 {
        x += TAU;
    }
    x
}

/// Circular forward distance from `from` to `to` in [0, 2π).
fn ang_forward(from: f64, to: f64) -> f64 {
    normalize_angle(to - from)
}

// ─── Decomposition ───

/// Split multiple closed splines (loops) into pieces. Corner insertion wraps
/// within each loop; `original_segment` indexes into the flattened segment
/// list across all loops.
pub fn split_loops_into_arcs(loops: &[&[CubicBez]]) -> Vec<MonotoneArc> {
    let mut all = Vec::new();
    let mut seg_offset = 0;
    for segments in loops {
        decompose_loop(segments, seg_offset, &mut all);
        seg_offset += segments.len();
    }
    all
}

fn decompose_loop(segments: &[CubicBez], seg_offset: usize, out: &mut Vec<MonotoneArc>) {
    let mut pieces: Vec<MonotoneArc> = Vec::new();

    for (rel_idx, cb) in segments.iter().enumerate() {
        let seg_idx = seg_offset + rel_idx;
        if is_degenerate_point(cb) {
            continue;
        }
        if is_collinear(cb) {
            let dir = (cb.p3 - cb.p0).atan2();
            pieces.push(MonotoneArc {
                bezier: *cb,
                kind: PieceKind::Line,
                theta_start: dir,
                theta_end: dir,
                original_segment: seg_idx,
                original_t_end: 1.0,
            });
            continue;
        }

        let mut cuts: Vec<f64> = vec![0.0];
        let mut infl: Vec<f64> = cb
            .inflections()
            .iter()
            .copied()
            .filter(|&t| t > 1e-6 && t < 1.0 - 1e-6)
            .collect();
        infl.sort_by(|a, b| a.partial_cmp(b).unwrap());
        cuts.extend(infl);
        cuts.push(1.0);

        for w in cuts.windows(2) {
            let (t0, t1) = (w[0], w[1]);
            if t1 - t0 < TOL {
                continue;
            }
            let sub = cb.subsegment(t0..t1);
            if is_degenerate_point(&sub) {
                continue;
            }
            if is_collinear(&sub) {
                let dir = (sub.p3 - sub.p0).atan2();
                pieces.push(MonotoneArc {
                    bezier: sub,
                    kind: PieceKind::Line,
                    theta_start: dir,
                    theta_end: dir,
                    original_segment: seg_idx,
                    original_t_end: t1,
                });
                continue;
            }

            // Unwrap the tangent angle along the piece via interior samples so
            // theta_end - theta_start is the true signed turn.
            let mut theta = start_tangent_dir(&sub).atan2();
            let theta_s = theta;
            for k in 1..=8 {
                let t = k as f64 / 8.0;
                let d = tangent_dir_at(&sub, t);
                theta = unwrap_angle(d.atan2(), theta);
            }
            let theta_e = unwrap_angle(end_tangent_dir(&sub).atan2(), theta);

            pieces.push(MonotoneArc {
                bezier: sub,
                kind: PieceKind::Arc,
                theta_start: theta_s,
                theta_end: theta_e,
                original_segment: seg_idx,
                original_t_end: t1,
            });
        }
    }

    // Insert corner pieces at tangent-discontinuous junctions (wrapping
    // within this loop).
    let n = pieces.len();
    for i in 0..n {
        out.push(pieces[i].clone());
        let next = &pieces[(i + 1) % n];
        let theta_out = end_tangent_dir(&pieces[i].bezier).atan2();
        let theta_in = start_tangent_dir(&next.bezier).atan2();
        let mut gap = theta_in - theta_out;
        while gap > PI {
            gap -= TAU;
        }
        while gap <= -PI {
            gap += TAU;
        }
        if gap.abs() < 1e-8 {
            continue;
        }
        let vertex = pieces[i].bezier.p3;
        out.push(MonotoneArc {
            bezier: CubicBez::new(vertex, vertex, vertex, vertex),
            kind: PieceKind::Corner,
            theta_start: theta_out,
            theta_end: theta_out + gap,
            original_segment: pieces[i].original_segment,
            original_t_end: pieces[i].original_t_end,
        });
    }
}

// ─── Support Candidates ───

/// One support-function candidate: a piece viewed with one choice of outward
/// normal side.
#[derive(Clone, Debug)]
struct Candidate {
    piece: usize,
    kind: PieceKind,
    /// +1: normal = tangent rotated -90°; -1: normal = tangent rotated +90°.
    side: f64,
    /// Interval of normal angles covered, as (start ∈ [0,2π), length ≥ 0).
    /// Corners cover the full circle (len = 2π). Lines have len = 0.
    ang_start: f64,
    ang_len: f64,
    /// Curve parameter at interval start / end (arcs only).
    t_at_start: f64,
    t_at_end: f64,
}

impl Candidate {
    fn contains(&self, theta: f64, eps: f64) -> bool {
        if self.kind == PieceKind::Corner {
            return true;
        }
        let off = ang_forward(self.ang_start, theta);
        off <= self.ang_len + eps || off >= TAU - eps
    }
}

fn normal_angle(tangent: Vec2, side: f64) -> f64 {
    Vec2::new(side * tangent.y, -side * tangent.x).atan2()
}

fn build_candidates(pieces: &[MonotoneArc]) -> Vec<Candidate> {
    let mut cands = Vec::new();
    for (i, p) in pieces.iter().enumerate() {
        match p.kind {
            PieceKind::Corner => {
                cands.push(Candidate {
                    piece: i,
                    kind: PieceKind::Corner,
                    side: 1.0,
                    ang_start: 0.0,
                    ang_len: TAU,
                    t_at_start: 0.0,
                    t_at_end: 0.0,
                });
            }
            PieceKind::Line => {
                for side in [1.0, -1.0] {
                    let dir = p.bezier.p3 - p.bezier.p0;
                    cands.push(Candidate {
                        piece: i,
                        kind: PieceKind::Line,
                        side,
                        ang_start: normalize_angle(normal_angle(dir, side)),
                        ang_len: 0.0,
                        t_at_start: 0.0,
                        t_at_end: 1.0,
                    });
                }
            }
            PieceKind::Arc => {
                let turn = p.theta_end - p.theta_start;
                for side in [1.0, -1.0] {
                    // Normal angle at t=0 / t=1 for this side.
                    let n0 = p.theta_start - side * PI / 2.0;
                    let n1 = p.theta_end - side * PI / 2.0;
                    let (start, len, t_s, t_e) = if turn >= 0.0 {
                        (n0, turn, 0.0, 1.0)
                    } else {
                        (n1, -turn, 1.0, 0.0)
                    };
                    cands.push(Candidate {
                        piece: i,
                        kind: PieceKind::Arc,
                        side,
                        ang_start: normalize_angle(start),
                        ang_len: len.min(TAU),
                        t_at_start: t_s,
                        t_at_end: t_e,
                    });
                }
            }
        }
    }
    cands
}

/// Do two candidates' angular intervals overlap on the circle (open overlap)?
fn intervals_overlap(a: &Candidate, b: &Candidate) -> bool {
    if a.kind == PieceKind::Corner || b.kind == PieceKind::Corner {
        return true;
    }
    if a.ang_len >= TAU - ANG_EPS || b.ang_len >= TAU - ANG_EPS {
        return true;
    }
    let off = ang_forward(a.ang_start, b.ang_start);
    off < a.ang_len + ANG_EPS || TAU - off < b.ang_len + ANG_EPS
}

// ─── Support Evaluation ───

/// Solve a quadratic a·t² + b·t + c = 0, returning real roots.
fn solve_quadratic(a: f64, b: f64, c: f64, scale: f64) -> Vec<f64> {
    if a.abs() < 1e-14 * scale {
        if b.abs() < 1e-14 * scale {
            return vec![];
        }
        return vec![-c / b];
    }
    let disc = b * b - 4.0 * a * c;
    if disc < 0.0 {
        return vec![];
    }
    let sq = disc.sqrt();
    // Numerically stable form
    let q = -0.5 * (b + b.signum() * sq);
    let mut roots = vec![q / a];
    if q.abs() > 1e-300 {
        roots.push(c / q);
    } else {
        roots.push(if a != 0.0 { -b / a - roots[0] } else { roots[0] });
    }
    roots
}

/// Power basis coefficients for γ'(t) = c0 + c1·t + c2·t².
fn deriv_power_basis(cb: &CubicBez) -> [Vec2; 3] {
    let p0 = cb.p0.to_vec2();
    let p1 = cb.p1.to_vec2();
    let p2 = cb.p2.to_vec2();
    let p3 = cb.p3.to_vec2();
    let q0 = 3.0 * (p1 - p0);
    let q1 = 3.0 * (p2 - p1);
    let q2 = 3.0 * (p3 - p2);
    [q0, 2.0 * (q1 - q0), q0 - 2.0 * q1 + q2]
}

/// Power basis coefficients for γ(t) = d0 + d1·t + d2·t² + d3·t³.
fn curve_power_basis(cb: &CubicBez) -> [Vec2; 4] {
    let p0 = cb.p0.to_vec2();
    let p1 = cb.p1.to_vec2();
    let p2 = cb.p2.to_vec2();
    let p3 = cb.p3.to_vec2();
    [
        p0,
        3.0 * (p1 - p0),
        3.0 * (p0 - 2.0 * p1 + p2),
        -p0 + 3.0 * p1 - 3.0 * p2 + p3,
    ]
}

/// Invert the Gauss map of an arc candidate: parameter t whose outward normal
/// (for this candidate's side) equals theta.
fn arc_param_at_normal(cand: &Candidate, piece: &MonotoneArc, theta: f64) -> f64 {
    let off = ang_forward(cand.ang_start, theta);
    let off = if off > TAU - ANG_EPS { 0.0 } else { off };
    if off < ANG_EPS {
        return cand.t_at_start;
    }
    if off > cand.ang_len - ANG_EPS {
        return cand.t_at_end;
    }

    let cb = &piece.bezier;
    // Tangent direction required at the solution.
    let u = Vec2::new(-theta.sin() * cand.side, theta.cos() * cand.side);
    let d = deriv_power_basis(cb);
    let scale = chord_scale(cb);
    let roots = solve_quadratic(d[2].cross(u), d[1].cross(u), d[0].cross(u), scale);

    let expected = cand.t_at_start + (off / cand.ang_len) * (cand.t_at_end - cand.t_at_start);
    let mut best: Option<f64> = None;
    for r in roots {
        if !(-1e-9..=1.0 + 1e-9).contains(&r) {
            continue;
        }
        let rc = r.clamp(0.0, 1.0);
        let dv = d[0] + rc * d[1] + rc * rc * d[2];
        if dv.dot(u) <= 0.0 {
            continue;
        }
        match best {
            Some(b) if (b - expected).abs() <= (rc - expected).abs() => {}
            _ => best = Some(rc),
        }
    }
    best.unwrap_or(expected.clamp(0.0, 1.0))
}

/// Support value and contact point of a candidate at normal angle theta.
/// Assumes `cand.contains(theta)`.
fn support_at(cand: &Candidate, piece: &MonotoneArc, theta: f64) -> (f64, Point, f64) {
    let n = Vec2::new(theta.cos(), theta.sin());
    match cand.kind {
        PieceKind::Corner => {
            let p = piece.bezier.p0;
            (p.to_vec2().dot(n), p, 0.0)
        }
        PieceKind::Line => {
            let p = piece.bezier.p0;
            (p.to_vec2().dot(n), p, 0.0)
        }
        PieceKind::Arc => {
            let t = arc_param_at_normal(cand, piece, theta);
            let p = piece.bezier.eval(t);
            (p.to_vec2().dot(n), p, t)
        }
    }
}

// ─── Polynomial Utilities (for crossings) ───

fn poly_mul(a: &[f64], b: &[f64]) -> Vec<f64> {
    if a.is_empty() || b.is_empty() {
        return vec![];
    }
    let mut result = vec![0.0; a.len() + b.len() - 1];
    for (i, &ai) in a.iter().enumerate() {
        for (j, &bj) in b.iter().enumerate() {
            result[i + j] += ai * bj;
        }
    }
    result
}

fn poly_add(a: &[f64], b: &[f64]) -> Vec<f64> {
    let len = a.len().max(b.len());
    let mut result = vec![0.0; len];
    for (i, &v) in a.iter().enumerate() {
        result[i] += v;
    }
    for (i, &v) in b.iter().enumerate() {
        result[i] += v;
    }
    result
}

fn poly_sub(a: &[f64], b: &[f64]) -> Vec<f64> {
    let len = a.len().max(b.len());
    let mut result = vec![0.0; len];
    for (i, &v) in a.iter().enumerate() {
        result[i] += v;
    }
    for (i, &v) in b.iter().enumerate() {
        result[i] -= v;
    }
    result
}

fn poly_scale(a: &[f64], s: f64) -> Vec<f64> {
    a.iter().map(|&c| c * s).collect()
}

fn is_zero_poly(p: &[f64]) -> bool {
    p.is_empty() || p.iter().all(|&c| c.abs() < 1e-20)
}

fn trim_poly(p: &[f64]) -> &[f64] {
    let mut len = p.len();
    while len > 1 && p[len - 1].abs() < 1e-20 {
        len -= 1;
    }
    &p[..len]
}

fn poly_div_exact(num: &[f64], den: &[f64]) -> Vec<f64> {
    if is_zero_poly(num) {
        return vec![0.0];
    }
    let den_trimmed = trim_poly(den);
    let num_trimmed = trim_poly(num);
    if den_trimmed.len() == 1 {
        return poly_scale(num_trimmed, 1.0 / den_trimmed[0]);
    }
    if num_trimmed.len() < den_trimmed.len() {
        return vec![0.0];
    }
    let mut remainder = num_trimmed.to_vec();
    let mut quotient = vec![0.0; remainder.len() - den_trimmed.len() + 1];
    let lead_den = *den_trimmed.last().unwrap();
    for i in (0..quotient.len()).rev() {
        let idx = i + den_trimmed.len() - 1;
        let coeff = remainder[idx] / lead_den;
        quotient[i] = coeff;
        for (j, &d) in den_trimmed.iter().enumerate() {
            remainder[i + j] -= coeff * d;
        }
    }
    trim_poly(&quotient).to_vec()
}

fn eval_poly(coeffs: &[f64], t: f64) -> f64 {
    if coeffs.is_empty() {
        return 0.0;
    }
    let mut result = coeffs[coeffs.len() - 1];
    for i in (0..coeffs.len() - 1).rev() {
        result = result * t + coeffs[i];
    }
    result
}

fn eval_bivariate(eq: &[Vec<f64>], t1: f64, t2: f64) -> f64 {
    let mut result = 0.0;
    let mut t2_pow = 1.0;
    for coeffs_t1 in eq {
        result += eval_poly(coeffs_t1, t1) * t2_pow;
        t2_pow *= t2;
    }
    result
}

/// Bivariate system for the bitangent conditions between two cubics:
///   eq1: γ'_i(t1) × γ'_j(t2) = 0             (tangents parallel)
///   eq2: γ'_i(t1) × (γ_j(t2) - γ_i(t1)) = 0  (chord aligned with tangent)
/// Each equation is a vector of polynomials in t1, indexed by power of t2.
fn build_bitangent_system(arc_i: &CubicBez, arc_j: &CubicBez) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let di = deriv_power_basis(arc_i);
    let dj = deriv_power_basis(arc_j);
    let gi = curve_power_basis(arc_i);
    let gj = curve_power_basis(arc_j);

    let dix: Vec<f64> = di.iter().map(|v| v.x).collect();
    let diy: Vec<f64> = di.iter().map(|v| v.y).collect();
    let djx: Vec<f64> = dj.iter().map(|v| v.x).collect();
    let djy: Vec<f64> = dj.iter().map(|v| v.y).collect();
    let gix: Vec<f64> = gi.iter().map(|v| v.x).collect();
    let giy: Vec<f64> = gi.iter().map(|v| v.y).collect();
    let gjx: Vec<f64> = gj.iter().map(|v| v.x).collect();
    let gjy: Vec<f64> = gj.iter().map(|v| v.y).collect();

    let max_t2_eq1 = djy.len().max(djx.len());
    let mut eq1: Vec<Vec<f64>> = vec![vec![]; max_t2_eq1];
    for k in 0..djy.len() {
        eq1[k] = poly_scale(&dix, djy[k]);
    }
    for k in 0..djx.len() {
        let term = poly_scale(&diy, djx[k]);
        eq1[k] = if eq1[k].is_empty() {
            poly_scale(&term, -1.0)
        } else {
            poly_sub(&eq1[k], &term)
        };
    }

    let max_t2_eq2 = gjy.len().max(gjx.len());
    let mut eq2: Vec<Vec<f64>> = vec![vec![]; max_t2_eq2];
    for k in 0..gjy.len() {
        eq2[k] = poly_add(&eq2[k], &poly_scale(&dix, gjy[k]));
    }
    for k in 0..gjx.len() {
        eq2[k] = poly_sub(&eq2[k], &poly_scale(&diy, gjx[k]));
    }
    let pure_t1 = poly_sub(&poly_mul(&diy, &gix), &poly_mul(&dix, &giy));
    eq2[0] = poly_add(&eq2[0], &pure_t1);

    (eq1, eq2)
}

/// Sylvester resultant of two bivariate polynomials w.r.t. t2 (result: poly in t1).
fn sylvester_resultant(f: &[Vec<f64>], g: &[Vec<f64>]) -> Vec<f64> {
    let m = f.len() - 1;
    let n = g.len() - 1;
    let size = m + n;
    let mut matrix: Vec<Vec<Vec<f64>>> = vec![vec![vec![]; size]; size];
    for i in 0..n {
        for k in 0..=m {
            let col = i + k;
            if col < size {
                matrix[i][col] = f[m - k].clone();
            }
        }
    }
    for i in 0..m {
        for k in 0..=n {
            let col = i + k;
            if col < size {
                matrix[n + i][col] = g[n - k].clone();
            }
        }
    }
    poly_matrix_determinant(&mut matrix, size)
}

/// Fraction-free (Bareiss) elimination for a matrix of polynomials.
fn poly_matrix_determinant(matrix: &mut Vec<Vec<Vec<f64>>>, n: usize) -> Vec<f64> {
    let mut prev_pivot = vec![1.0];
    for col in 0..n {
        let mut pivot_row = None;
        for row in col..n {
            if !is_zero_poly(&matrix[row][col]) {
                pivot_row = Some(row);
                break;
            }
        }
        let pivot_row = match pivot_row {
            Some(r) => r,
            None => return vec![0.0],
        };
        if pivot_row != col {
            matrix.swap(pivot_row, col);
        }
        let pivot = matrix[col][col].clone();
        for row in (col + 1)..n {
            for j in (col + 1)..n {
                let term1 = poly_mul(&pivot, &matrix[row][j]);
                let term2 = poly_mul(&matrix[row][col], &matrix[col][j]);
                matrix[row][j] = poly_div_exact(&poly_sub(&term1, &term2), &prev_pivot);
            }
            matrix[row][col] = vec![0.0];
        }
        prev_pivot = pivot;
    }
    matrix[n - 1][n - 1].clone()
}

fn back_substitute_t2(eq1: &[Vec<f64>], t1: f64) -> Vec<f64> {
    let coeffs: Vec<f64> = eq1.iter().map(|c| eval_poly(c, t1)).collect();
    let trimmed = trim_poly(&coeffs);
    if is_zero_poly(trimmed) {
        return vec![];
    }
    PolyDyn::new(trimmed.iter().copied()).roots_between(0.0, 1.0, TOL)
}

// ─── Crossings ───

/// A point where two candidates' support functions meet: a common supporting
/// direction with equal support value (a bitangency).
#[derive(Clone, Debug)]
struct Crossing {
    cand_a: usize,
    cand_b: usize,
    theta: f64,
    point_a: Point,
    point_b: Point,
    t_a: f64,
    t_b: f64,
}

/// All bitangent (t1, t2) solutions between two cubics.
fn solve_arc_arc(cb_i: &CubicBez, cb_j: &CubicBez) -> Vec<(f64, f64)> {
    let (eq1, eq2) = build_bitangent_system(cb_i, cb_j);
    let resultant = sylvester_resultant(&eq1, &eq2);
    let trimmed = trim_poly(&resultant);
    if is_zero_poly(trimmed) {
        return vec![];
    }
    let t1_roots = PolyDyn::new(trimmed.iter().copied()).roots_between(0.0, 1.0, TOL);
    let mut out = Vec::new();
    for &t1 in &t1_roots {
        for t2 in back_substitute_t2(&eq1, t1) {
            if !(-TOL..=1.0 + TOL).contains(&t2) {
                continue;
            }
            let t1c = t1.clamp(0.0, 1.0);
            let t2c = t2.clamp(0.0, 1.0);
            // Residual check with relative tolerance (eq2 scales as O(L²)).
            let residual = eval_bivariate(&eq2, t1c, t2c);
            let d = cb_i.deriv().eval(t1c).to_vec2();
            let disp = cb_j.eval(t2c) - cb_i.eval(t1c);
            let scale = d.hypot() * disp.hypot();
            if residual.abs() < 1e-6 * scale.max(1.0) {
                out.push((t1c, t2c));
            }
        }
    }
    out
}

/// All parameters t where the tangent line of `cb` at t passes through P.
fn solve_point_tangency(cb: &CubicBez, p: Point) -> Vec<f64> {
    let g = curve_power_basis(cb);
    let d = deriv_power_basis(cb);
    let dpx = [g[0].x - p.x, g[1].x, g[2].x, g[3].x];
    let dpy = [g[0].y - p.y, g[1].y, g[2].y, g[3].y];
    let dx: Vec<f64> = d.iter().map(|v| v.x).collect();
    let dy: Vec<f64> = d.iter().map(|v| v.y).collect();
    let eq = poly_sub(&poly_mul(&dx, &dpy), &poly_mul(&dy, &dpx));
    let trimmed = trim_poly(&eq);
    if is_zero_poly(trimmed) {
        return vec![];
    }
    PolyDyn::new(trimmed.iter().copied()).roots_between(0.0, 1.0, TOL)
}

fn find_crossings(cands: &[Candidate], pieces: &[MonotoneArc], scale: f64) -> Vec<Crossing> {
    let geom_eps = 1e-9 * scale;
    let mut crossings = Vec::new();

    for a in 0..cands.len() {
        for b in (a + 1)..cands.len() {
            let (ca, cb) = (&cands[a], &cands[b]);
            if ca.piece == cb.piece {
                continue;
            }
            if !intervals_overlap(ca, cb) {
                continue;
            }
            let (pa, pb) = (&pieces[ca.piece], &pieces[cb.piece]);

            match (ca.kind, cb.kind) {
                (PieceKind::Arc, PieceKind::Arc) => {
                    // Dedup: the same geometric pair is solved once per (a,b)
                    // candidate pair, but the algebraic solve doesn't depend on
                    // sides. Cache would help; correctness first.
                    for (t1, t2) in solve_arc_arc(&pa.bezier, &pb.bezier) {
                        let q1 = pa.bezier.eval(t1);
                        let q2 = pb.bezier.eval(t2);
                        if (q2 - q1).hypot() < geom_eps {
                            continue;
                        }
                        let d1 = tangent_dir_at(&pa.bezier, t1);
                        let d2 = tangent_dir_at(&pb.bezier, t2);
                        let th_a = normal_angle(d1, ca.side);
                        let th_b = normal_angle(d2, cb.side);
                        if ang_diff(th_a, th_b) > ANG_MATCH {
                            continue;
                        }
                        let theta = normalize_angle(th_a);
                        if !ca.contains(theta, ANG_EPS) || !cb.contains(theta, ANG_EPS) {
                            continue;
                        }
                        crossings.push(Crossing {
                            cand_a: a,
                            cand_b: b,
                            theta,
                            point_a: q1,
                            point_b: q2,
                            t_a: t1,
                            t_b: t2,
                        });
                    }
                }
                (PieceKind::Arc, PieceKind::Corner) | (PieceKind::Arc, PieceKind::Line)
                | (PieceKind::Corner, PieceKind::Arc) | (PieceKind::Line, PieceKind::Arc) => {
                    // Point-vs-arc tangency. For lines, both endpoints act as
                    // points; but line-endpoint junction corners already cover
                    // hull-relevant transitions, and a line interior can never
                    // be tangent from outside. Only corners need solving here.
                    let (arc_idx, pt_idx, arc_cand, pt_cand) = if ca.kind == PieceKind::Arc {
                        (ca.piece, cb.piece, a, b)
                    } else {
                        (cb.piece, ca.piece, b, a)
                    };
                    if pieces[pt_idx].kind == PieceKind::Line {
                        continue;
                    }
                    let arc_piece = &pieces[arc_idx];
                    let p = pieces[pt_idx].bezier.p0;
                    for t in solve_point_tangency(&arc_piece.bezier, p) {
                        let q = arc_piece.bezier.eval(t);
                        if (q - p).hypot() < geom_eps {
                            continue;
                        }
                        // Spurious roots where the derivative vanishes.
                        let draw = arc_piece.bezier.deriv().eval(t).to_vec2();
                        let dir = tangent_dir_at(&arc_piece.bezier, t);
                        let chord = q - p;
                        if draw.hypot() < 1e-9 * chord_scale(&arc_piece.bezier)
                            && dir.cross(chord).abs() > 1e-6 * dir.hypot() * chord.hypot()
                        {
                            continue;
                        }
                        let arc_side = cands[arc_cand].side;
                        let theta = normalize_angle(normal_angle(dir, arc_side));
                        if !cands[arc_cand].contains(theta, ANG_EPS) {
                            continue;
                        }
                        let (ta, tb, qa, qb) = if arc_cand == a {
                            (t, 0.0, q, p)
                        } else {
                            (0.0, t, p, q)
                        };
                        let _ = pt_cand;
                        crossings.push(Crossing {
                            cand_a: a,
                            cand_b: b,
                            theta,
                            point_a: qa,
                            point_b: qb,
                            t_a: ta,
                            t_b: tb,
                        });
                    }
                }
                (PieceKind::Corner, PieceKind::Corner) => {
                    let pi = pa.bezier.p0;
                    let pj = pb.bezier.p0;
                    let v = pj - pi;
                    if v.hypot() < geom_eps {
                        continue;
                    }
                    for theta in [
                        normalize_angle(v.atan2() + PI / 2.0),
                        normalize_angle(v.atan2() - PI / 2.0),
                    ] {
                        crossings.push(Crossing {
                            cand_a: a,
                            cand_b: b,
                            theta,
                            point_a: pi,
                            point_b: pj,
                            t_a: 0.0,
                            t_b: 0.0,
                        });
                    }
                }
                _ => {} // line-line, line-corner: transitions occur at
                         // breakpoints already contributed by the line angles.
            }
        }
    }
    crossings
}

fn ang_diff(a: f64, b: f64) -> f64 {
    let d = normalize_angle(a - b);
    d.min(TAU - d)
}

// ─── Envelope Sweep & Assembly ───

struct Envelope {
    /// Breakpoint angles, sorted, covering the circle.
    breaks: Vec<f64>,
    /// Winner candidate index for each interval (breaks[i], breaks[i+1]).
    winners: Vec<usize>,
}

fn sweep_envelope(cands: &[Candidate], pieces: &[MonotoneArc], crossings: &[Crossing], scale: f64) -> Envelope {
    let mut breaks: Vec<f64> = Vec::new();
    for c in cands {
        if c.kind == PieceKind::Corner {
            continue;
        }
        breaks.push(normalize_angle(c.ang_start));
        breaks.push(normalize_angle(c.ang_start + c.ang_len));
    }
    for x in crossings {
        breaks.push(x.theta);
    }
    breaks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    breaks.dedup_by(|a, b| (*a - *b).abs() < ANG_EPS);
    if breaks.is_empty() {
        breaks.push(0.0);
    }
    // Circular dedup of first/last.
    if breaks.len() > 1 && (TAU - breaks[breaks.len() - 1] + breaks[0]).abs() < ANG_EPS {
        breaks.pop();
    }

    let eps_h = 1e-9 * scale;
    let m = breaks.len();
    let mut winners = Vec::with_capacity(m);
    for i in 0..m {
        let a = breaks[i];
        let b = if i + 1 < m { breaks[i + 1] } else { breaks[0] + TAU };
        let mid = normalize_angle(a + ang_forward(a, normalize_angle(b)) / 2.0);
        let mut best: Option<(f64, usize)> = None;
        for (ci, c) in cands.iter().enumerate() {
            if c.kind == PieceKind::Line {
                continue;
            }
            if !c.contains(mid, ANG_EPS) {
                continue;
            }
            let (h, _, _) = support_at(c, &pieces[c.piece], mid);
            match best {
                Some((bh, _)) if h <= bh + eps_h => {
                    // Tie: prefer arcs over corners (a corner coincident with
                    // an arc endpoint should yield to the arc).
                    if h >= bh - eps_h
                        && c.kind == PieceKind::Arc
                        && cands[best.unwrap().1].kind == PieceKind::Corner
                    {
                        best = Some((h, ci));
                    }
                }
                _ => best = Some((h, ci)),
            }
        }
        let w = best.expect("no active candidate — coverage gap").1;
        winners.push(w);
    }
    Envelope { breaks, winners }
}

/// One entry of a transition tie-set: a contact point on the common support line.
struct TieEntry {
    cand: usize,
    proj: f64,
    point: Point,
    t: f64,
}

fn assemble(cands: &[Candidate], pieces: &[MonotoneArc], crossings: &[Crossing], env: &Envelope, scale: f64) -> Vec<HullSegment> {
    let m = env.breaks.len();
    let geom_eps = 1e-7 * scale;
    let eps_h = 1e-7 * scale;

    // Merge consecutive intervals with the same winner into runs.
    // runs: (winner, theta_from, theta_to) with theta_to lifted ≥ theta_from.
    let mut run_bounds: Vec<usize> = Vec::new(); // indices into breaks where winner changes
    for i in 0..m {
        let prev = env.winners[(i + m - 1) % m];
        if env.winners[i] != prev {
            run_bounds.push(i);
        }
    }

    if run_bounds.is_empty() {
        // Single winner covers everything: hull is that single closed piece
        // (or one arc traversed fully) — emit all its pieces.
        let w = env.winners[0];
        let c = &cands[w];
        if c.kind == PieceKind::Arc {
            // A closed convex curve decomposed into one arc — unusual but emit fully.
            return vec![HullSegment::Arc {
                arc_index: c.piece,
                t_start: c.t_at_start,
                t_end: c.t_at_end,
            }];
        }
        return vec![];
    }

    // For crossing lookup at transitions.
    let find_crossing = |x: usize, y: usize, theta: f64| -> Option<&Crossing> {
        crossings
            .iter()
            .filter(|c| {
                ((c.cand_a == x && c.cand_b == y) || (c.cand_a == y && c.cand_b == x))
                    && ang_diff(c.theta, theta) < 1e-7
            })
            .min_by(|p, q| {
                ang_diff(p.theta, theta)
                    .partial_cmp(&ang_diff(q.theta, theta))
                    .unwrap()
            })
    };

    let mut segments: Vec<HullSegment> = Vec::new();
    let nb = run_bounds.len();

    for ri in 0..nb {
        // Run: winner w from break run_bounds[ri] to run_bounds[(ri+1) % nb].
        let start_bi = run_bounds[ri];
        let end_bi = run_bounds[(ri + 1) % nb];
        let w = env.winners[start_bi];
        let c = &cands[w];
        let theta_in = env.breaks[start_bi];
        let theta_out = env.breaks[end_bi];

        // Entry/exit contact parameters for arc winners.
        if c.kind == PieceKind::Arc {
            let prev_w = env.winners[(start_bi + m - 1) % m];
            let next_w = env.winners[end_bi];
            let t_in = find_crossing(prev_w, w, theta_in)
                .map(|x| if x.cand_a == w { x.t_a } else { x.t_b })
                .unwrap_or_else(|| arc_param_at_normal(c, &pieces[c.piece], theta_in));
            let t_out = find_crossing(w, next_w, theta_out)
                .map(|x| if x.cand_a == w { x.t_a } else { x.t_b })
                .unwrap_or_else(|| arc_param_at_normal(c, &pieces[c.piece], theta_out));
            if (t_out - t_in).abs() > 1e-12 {
                segments.push(HullSegment::Arc {
                    arc_index: c.piece,
                    t_start: t_in,
                    t_end: t_out,
                });
            }
        }

        // Transition at theta_out between w and the next run's winner.
        let next_w = env.winners[end_bi];
        let theta_c = theta_out;
        let n = Vec2::new(theta_c.cos(), theta_c.sin());
        let tau = Vec2::new(-theta_c.sin(), theta_c.cos());

        // Gather tie set: every candidate achieving max support at theta_c.
        // Line pieces at this angle are tracked separately so consecutive
        // nodes connected by an input line emit that line, not a bitangent.
        let mut max_h = f64::NEG_INFINITY;
        let mut entries: Vec<TieEntry> = Vec::new();
        let mut line_ties: Vec<usize> = Vec::new(); // candidate indices
        for (ci, cc) in cands.iter().enumerate() {
            if cc.kind == PieceKind::Line {
                let d = ang_diff(normalize_angle(cc.ang_start), theta_c);
                if d > 1e-7 {
                    continue;
                }
                // Both endpoints of the line are contacts.
                let (q0, q3) = (pieces[cc.piece].bezier.p0, pieces[cc.piece].bezier.p3);
                let h = q0.to_vec2().dot(n);
                max_h = max_h.max(h);
                line_ties.push(ci);
                entries.push(TieEntry {
                    cand: ci,
                    proj: q0.to_vec2().dot(tau),
                    point: q0,
                    t: 0.0,
                });
                entries.push(TieEntry {
                    cand: ci,
                    proj: q3.to_vec2().dot(tau),
                    point: q3,
                    t: 1.0,
                });
                continue;
            }
            if !cc.contains(theta_c, 1e-7) {
                continue;
            }
            let (h, p, t) = support_at(cc, &pieces[cc.piece], theta_c);
            max_h = max_h.max(h);
            entries.push(TieEntry {
                cand: ci,
                proj: p.to_vec2().dot(tau),
                point: p,
                t,
            });
        }
        // The exiting/entering winners define this transition: use their exact
        // algebraic crossing contacts when available, and always keep them
        // (by continuity of the envelope they are at the max).
        if let Some(x) = find_crossing(w, next_w, theta_c) {
            for e in entries.iter_mut() {
                let (p, t) = if x.cand_a == e.cand {
                    (x.point_a, x.t_a)
                } else if x.cand_b == e.cand {
                    (x.point_b, x.t_b)
                } else {
                    continue;
                };
                e.point = p;
                e.t = t;
                e.proj = p.to_vec2().dot(tau);
            }
        }
        // Keep only entries at the max support level (winners always kept).
        let mut tie: Vec<TieEntry> = entries
            .into_iter()
            .filter(|e| {
                e.cand == w || e.cand == next_w || e.point.to_vec2().dot(n) >= max_h - eps_h
            })
            .collect();
        tie.sort_by(|a, b| a.proj.partial_cmp(&b.proj).unwrap());

        // Deduplicate coincident contact points, preferring the exiting winner
        // first and the entering winner last.
        let mut nodes: Vec<TieEntry> = Vec::new();
        for e in tie {
            if let Some(last) = nodes.last() {
                if (e.point - last.point).hypot() < geom_eps {
                    // Same geometric node: keep the more relevant candidate.
                    let keep_new = e.cand == w || e.cand == next_w;
                    let keep_old = last.cand == w || last.cand == next_w;
                    if keep_new && !keep_old {
                        nodes.pop();
                        nodes.push(e);
                    }
                    continue;
                }
            }
            nodes.push(e);
        }

        // Thread through the nodes in traversal order. A gap between
        // consecutive nodes is an input line piece if one spans it, otherwise
        // a bitangent line.
        for k in 0..nodes.len().saturating_sub(1) {
            let (e1, e2) = (&nodes[k], &nodes[k + 1]);
            let spanning_line = line_ties.iter().copied().find(|&ci| {
                let lb = &pieces[cands[ci].piece].bezier;
                ((lb.p0 - e1.point).hypot() < geom_eps && (lb.p3 - e2.point).hypot() < geom_eps)
                    || ((lb.p3 - e1.point).hypot() < geom_eps
                        && (lb.p0 - e2.point).hypot() < geom_eps)
            });
            if let Some(ci) = spanning_line {
                let lb = &pieces[cands[ci].piece].bezier;
                let forward = (lb.p0 - e1.point).hypot() < geom_eps;
                segments.push(HullSegment::Arc {
                    arc_index: cands[ci].piece,
                    t_start: if forward { 0.0 } else { 1.0 },
                    t_end: if forward { 1.0 } else { 0.0 },
                });
            } else {
                segments.push(HullSegment::Line { start: e1.point, end: e2.point });
            }
        }
    }

    segments
}

// ─── Public API ───

/// Convex hull of multiple closed splines. Hull `arc_index` values reference
/// the pieces returned by `split_loops_into_arcs` for the same loops.
pub fn convex_hull_loops(loops: &[&[CubicBez]]) -> Vec<HullSegment> {
    let pieces = split_loops_into_arcs(loops);
    if pieces.is_empty() {
        return vec![];
    }

    let mut scale: f64 = 0.0;
    for p in &pieces {
        for cp in [p.bezier.p0, p.bezier.p1, p.bezier.p2, p.bezier.p3] {
            scale = scale.max(cp.x.abs()).max(cp.y.abs());
        }
    }
    let scale = scale.max(1e-9);

    let cands = build_candidates(&pieces);
    let crossings = find_crossings(&cands, &pieces, scale);
    let env = sweep_envelope(&cands, &pieces, &crossings, scale);
    assemble(&cands, &pieces, &crossings, &env, scale)
}
