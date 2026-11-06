//! A parametric heart made of two cubic Bézier segments per half: a lobe from the cleft to the widest point, and a side from there to the tip.
//!
//! The right half is designed in two reference frames, each 1 tall: the lobe frame above the widest point and the side frame below it, both linearly stretched into the square box.
//! Their widths give the proportions of a 1.1 aspect heart widest at 70%, where every tuning constant was calibrated.
//!
//! Every step is a smooth function of the proportions and every solve finds the unique root of a strictly monotonic function, so the outline moves without jumps or kinks.

use super::{Anchor, bezpath_from_anchors};
use glam::DVec2;
use kurbo::{BezPath, CubicBez, ParamCurveExtrema};
use std::f64::consts::{FRAC_PI_2, PI};

const REFERENCE_HALF_WIDTH: f64 = 0.55;
const REFERENCE_WIDEST: f64 = 0.7;
const LOBE_FRAME_WIDTH: f64 = REFERENCE_HALF_WIDTH / (1. - REFERENCE_WIDEST);
const SIDE_FRAME_WIDTH: f64 = REFERENCE_HALF_WIDTH / REFERENCE_WIDEST;

/// How far the tip handle may stray sideways from the side's chord, as a share of that chord.
const STRAY: f64 = 0.22;
/// Tip tangent deviation from the side's chord at which the narrow regime's side fullness range starts narrowing.
const SIDE_ANGLE_SCALE: f64 = 20_f64.to_radians();

/// The side's share of the curvature gap at the widest point when it is rounder or tighter than the lobe, in the narrow regime.
const ROUNDER_SIDE_SHARE: f64 = 0.5;
const TIGHTER_SIDE_SHARE: f64 = 0.15;
const SHARE_BLEND_WIDTH: f64 = 0.25;
/// Comfortable range of the automatic lobe lean, kept moderate so it doesn't override the user's lobe lean or cleft sharpness.
const AUTOMATIC_LEAN_LIMIT: f64 = 1.;
/// Bracket of the automatic lean solve, wide enough for the lobe to always tighten to the side's flattest.
const LEAN_BRACKET: f64 = 7.;
/// Longest shoulder handle as a share of the side frame's height, which bounds how flat the side can be at the widest point.
const SHOULDER_LIMIT: f64 = 0.95;
/// Softness (in natural log of curvature) of the bounds that keep the shared curvature reachable by both the lobe and the side.
const BOUND_SOFTNESS: f64 = 0.05;
/// Narrowest window between those bounds.
const MINIMUM_WINDOW: f64 = 0.1;
/// Extra tip tension above the default fullness, and the softness of its onset, so full sides don't pinch the point.
const TIP_EASE: f64 = 0.525;
const TIP_EASE_SOFTNESS: f64 = 0.08;
/// Farthest the tip handle reaches across the side frame's width.
const TIP_CROSSING_LIMIT: f64 = 0.9;

/// How far the tip and shoulder handles reach toward the corner, from straight to full sides.
const STRAIGHT_TIP_FRACTION: f64 = 0.3;
const FULL_TIP_FRACTION: f64 = 0.85;
const STRAIGHT_SHOULDER_FRACTION: f64 = 0.28;
const FULL_SHOULDER_FRACTION: f64 = 0.55;
/// How much the corner fractions lag side fullness in its lower half, evening out how the side fills as fullness rises.
const CORNER_FULLNESS_EASING: f64 = 0.65;
/// The side's share of the curvature gap in the corner regime when tighter, when rounder, and when rounder deep in that regime (where a long tip handle fills the side out itself).
const CORNER_TIGHTER_SHARE: f64 = 0.85;
const CORNER_ROUNDER_SHARE: f64 = 0.4;
const DEEP_CORNER_ROUNDER_SHARE: f64 = 0.15;
/// Extra automatic lean the lobe may take to tighten its end for straight sides in the corner regime.
const CORNER_EXTRA_LEAN: f64 = 1.5;
/// Smallest radius of curvature at the widest point (in heart heights), so extra tightening can't turn it into a corner.
const WIDEST_RADIUS_FLOOR: f64 = 0.04;
/// Corner leverage (the corner's drop times the widest point's height times the square root of the curvature) where the side gets its say, and where a longer tip handle starts filling the side out.
const SAY_LEVERAGE: f64 = 0.4;
const TIP_LEVERAGE: f64 = 0.9;
/// Smallest corner drops for those transitions, so a very tight lobe can't rush them, and the transitions' softness.
const MINIMUM_SAY_DROP: f64 = 0.15;
const MINIMUM_TIP_DROP: f64 = 0.25;
const DROP_SOFTNESS: f64 = 0.1;
const DROP_BOUND_SOFTNESS: f64 = 0.03;
/// Range of lean within which the natural lobe meets the side, safely inside the solve's bracket.
const FLATTEST_NATURAL_LEAN: f64 = -3.;
const TIGHTEST_NATURAL_LEAN: f64 = 4.;

/// The proportions that shape a heart, each a fraction where 1 is 100%, except the tip angle in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeartProportions {
	/// How far the cleft dips between the lobes, from 0 at their tops to 1 level with the widest point.
	pub cleft_depth: f64,
	/// From -1 for a soft dip, through 0 for the lobes' natural round shape, to 1 for a sharp crease.
	pub cleft_sharpness: f64,
	/// From -1 with each lobe's fullness in toward the cleft, through 0 for the natural round lobe, to 1 out toward the sides.
	pub lobe_lean: f64,
	/// Height of the widest point above the tip, as a fraction of the heart's height.
	pub widest_point: f64,
	/// From 0 for straight sides tapering into the tip to 1 for full sides curving outward all the way down.
	pub side_fullness: f64,
	/// Opening angle at the tip, in radians.
	pub tip_angle: f64,
}

/// Width and height of the square box that [`heart_bezpath`] fills.
pub const HEART_SIZE: f64 = 100.;

/// Constructs a heart filling a [`HEART_SIZE`] square box centered on the origin, with its tip pointing down.
pub fn heart_bezpath(proportions: &HeartProportions) -> BezPath {
	let half = heart_half(proportions);

	// Center the box on the origin with y pointing down, and mirror the right half into the left
	let right = |point: DVec2| HEART_SIZE * DVec2::new(point.x, 0.5 - point.y);
	let left = |point: DVec2| HEART_SIZE * DVec2::new(-point.x, 0.5 - point.y);

	let anchors = [
		Anchor::new(right(half.cleft), Some(left(half.cleft_out)), Some(right(half.cleft_out))),
		Anchor::new(right(half.widest), Some(right(half.widest_in)), Some(right(half.widest_out))),
		Anchor::new(right(DVec2::ZERO), Some(right(half.tip_in)), Some(left(half.tip_in))),
		Anchor::new(left(half.widest), Some(left(half.widest_out)), Some(left(half.widest_in))),
	];

	bezpath_from_anchors(&anchors, true)
}

/// The control points of the heart's right half in the square box, with the tip at the origin and y pointing up.
struct HeartHalf {
	cleft: DVec2,
	cleft_out: DVec2,
	widest_in: DVec2,
	widest: DVec2,
	widest_out: DVec2,
	tip_in: DVec2,
}

/// The lobe's control points in the lobe frame.
struct Lobe {
	cleft: DVec2,
	cleft_out: DVec2,
	widest_in: DVec2,
}

fn heart_half(proportions: &HeartProportions) -> HeartHalf {
	let depth = proportions.cleft_depth;
	let widest = proportions.widest_point;

	// The tip angle is literal in the square box, so convert it to the side frame's angle
	let frame_stretch = 2. * SIDE_FRAME_WIDTH * widest;
	let tip_half_angle = ((proportions.tip_angle / 2.).tan() * frame_stretch).atan();
	let (tip_sine, tip_cosine) = tip_half_angle.sin_cos();

	// Curvature at the widest point in the box, up to a factor that cancels in every comparison: the horizontal gap to the next control point over the squared vertical handle length
	let lobe_curvature = |lobe: &Lobe| (1. - lobe.cleft_out.x / LOBE_FRAME_WIDTH) / (lobe.widest_in.y * (1. - widest)).powi(2);
	let side_curvature = |gap: f64, shoulder_length: f64| gap / (shoulder_length * widest).powi(2);

	// The lobe for the user's lean plus an automatic lean, with an end curvature that rises strictly with lean
	let neutral = EllipseLobe::new(depth);
	let elevation = cleft_elevation(neutral.elevation, depth, proportions.cleft_sharpness);
	let user_lean = -(0.7 * proportions.lobe_lean + 0.15 * proportions.lobe_lean.powi(2));
	let lobe_for = |automatic_lean: f64| solve_lobe(depth, elevation, &neutral, user_lean + automatic_lean);
	let log_lobe_curvature = |automatic_lean: f64| lobe_curvature(&lobe_for(automatic_lean)).ln();
	let preferred_lobe = log_lobe_curvature(0.);
	let natural_lobe = log_lobe_curvature(-user_lean);

	// Narrow regime tip handle: a tension of its reach, softly limited in how far it strays from the side's chord and reaches across, and eased off so full sides don't pinch the point
	let fullness = effective_fullness(proportions.side_fullness, tip_half_angle);
	let side_chord = SIDE_FRAME_WIDTH.hypot(1.);
	let reach = smooth_min(SIDE_FRAME_WIDTH / tip_sine, 1. / tip_cosine);
	let fullness_ease = TIP_EASE_SOFTNESS * softplus((fullness - 0.5) / TIP_EASE_SOFTNESS);
	let tension = (0.96 - 0.88 * fullness + TIP_EASE * fullness_ease.powi(2)) * reach;
	let stray_ratio = tension * (tip_half_angle - SIDE_FRAME_WIDTH.atan()).sin() / (STRAY * side_chord);
	let stray_limited = tension / (1. + stray_ratio.powi(6)).powf(1. / 6.);
	let narrow_tip_length = soft_cap(stray_limited, TIP_CROSSING_LIMIT * SIDE_FRAME_WIDTH / tip_sine, 6);
	let narrow_gap = 1. - narrow_tip_length * tip_sine / SIDE_FRAME_WIDTH;
	let narrow_side = side_curvature(narrow_gap, 0.12 + 0.5 * fullness).ln();

	// Corner regime for blunt tips, where the tip's tangent meets the widest point's vertical tangent at a corner and both handles reach toward it by fractions of fullness.
	// The side prefers the natural lobe's curvature, scaled as continuity's (1 - tip) / shoulder^2 changes from the middle fractions.
	let corner_drop = 1. - SIDE_FRAME_WIDTH / tip_half_angle.tan();
	let corner_reach = SIDE_FRAME_WIDTH / tip_sine;
	let side_fullness = proportions.side_fullness - CORNER_FULLNESS_EASING * proportions.side_fullness * (1. - proportions.side_fullness);
	let corner_tip_fraction = STRAIGHT_TIP_FRACTION + (FULL_TIP_FRACTION - STRAIGHT_TIP_FRACTION) * side_fullness;
	let corner_shoulder_fraction = STRAIGHT_SHOULDER_FRACTION + (FULL_SHOULDER_FRACTION - STRAIGHT_SHOULDER_FRACTION) * side_fullness;
	let middle_tip_fraction = (STRAIGHT_TIP_FRACTION + FULL_TIP_FRACTION) / 2.;
	let middle_shoulder_fraction = (STRAIGHT_SHOULDER_FRACTION + FULL_SHOULDER_FRACTION) / 2.;
	let corner_side = natural_lobe + ((1. - corner_tip_fraction) / (1. - middle_tip_fraction)).ln() - 2. * (corner_shoulder_fraction / middle_shoulder_fraction).ln();

	// Regime weights from the corner's leverage, with drops softly kept above a minimum so a very tight lobe can't rush the transition
	let leverage_drop = |leverage: f64, log_curvature: f64, minimum: f64| soft_maximum(leverage / (widest * (log_curvature / 2.).exp()), minimum, DROP_BOUND_SOFTNESS);
	let say_weight = logistic((corner_drop - leverage_drop(SAY_LEVERAGE, preferred_lobe, MINIMUM_SAY_DROP)) / DROP_SOFTNESS);
	let provisional_corner_weight = logistic((corner_drop - leverage_drop(TIP_LEVERAGE, preferred_lobe, MINIMUM_TIP_DROP)) / DROP_SOFTNESS);

	// Shared curvature: a geometric blend of the lobe's and side's preferences, with the side's share small when tighter in the narrow regime and large in the corner regime
	let preferred_side = (1. - say_weight) * narrow_side + say_weight * corner_side;
	let rounderness = 0.5 + 0.5 * ((natural_lobe - preferred_side) / SHARE_BLEND_WIDTH).tanh();
	let corner_rounder_share = CORNER_ROUNDER_SHARE + (DEEP_CORNER_ROUNDER_SHARE - CORNER_ROUNDER_SHARE) * provisional_corner_weight;
	let tighter_share = (1. - say_weight) * TIGHTER_SIDE_SHARE + say_weight * CORNER_TIGHTER_SHARE;
	let rounder_share = (1. - say_weight) * ROUNDER_SIDE_SHARE + say_weight * corner_rounder_share;
	let share = tighter_share + (rounder_share - tighter_share) * rounderness;

	// The natural lobe meets the shared curvature by a lean, then the user's lean is added in full so it reshapes the lobe as much as it would on its own
	let natural_flattest = log_lobe_curvature(FLATTEST_NATURAL_LEAN - user_lean);
	let natural_tightest = log_lobe_curvature(TIGHTEST_NATURAL_LEAN - user_lean);
	let natural_agreed = soft_clamp(natural_lobe + share * (preferred_side - natural_lobe), natural_flattest, natural_tightest);
	let natural_lean = solve_increasing(log_lobe_curvature, natural_agreed, -LEAN_BRACKET, LEAN_BRACKET) + user_lean;
	let agreed = log_lobe_curvature(natural_lean);

	// Keep the target within what both the lobe (by a comfortable lean) and the side (by its longest shoulder) can reach, with the side's limit winning where they conflict
	let provisional_gap = (1. - provisional_corner_weight) * narrow_gap + provisional_corner_weight * (1. - corner_tip_fraction);
	let flattest_side = side_curvature(provisional_gap, SHOULDER_LIMIT).ln();
	let lower = soft_maximum(log_lobe_curvature(-AUTOMATIC_LEAN_LIMIT), flattest_side, BOUND_SOFTNESS);
	let tightest = soft_minimum(
		log_lobe_curvature(AUTOMATIC_LEAN_LIMIT + say_weight * CORNER_EXTRA_LEAN),
		(3. / WIDEST_RADIUS_FLOOR).ln(),
		BOUND_SOFTNESS,
	);
	let upper = soft_maximum(tightest, lower + MINIMUM_WINDOW, BOUND_SOFTNESS);
	let target = soft_clamp(agreed, lower, upper);

	let automatic_lean = solve_increasing(log_lobe_curvature, target, -LEAN_BRACKET, LEAN_BRACKET);
	let lobe = lobe_for(automatic_lean);

	// The tip handle blends the narrow mapping (longer for straighter sides) and the corner mapping (longer for fuller sides)
	let corner_weight = logistic((corner_drop - leverage_drop(TIP_LEVERAGE, target, MINIMUM_TIP_DROP)) / DROP_SOFTNESS);
	let tip_length = (1. - corner_weight) * narrow_tip_length + corner_weight * corner_tip_fraction * corner_reach;
	let tip_in = tip_length * DVec2::new(tip_sine, tip_cosine);

	// The side's shoulder handle matches the lobe's end curvature exactly
	let side_gap = 1. - tip_in.x / SIDE_FRAME_WIDTH;
	let shoulder_length = (side_gap / lobe_curvature(&lobe)).sqrt() / widest;

	// Map both frames into the box
	let from_lobe = |point: DVec2| DVec2::new(point.x / LOBE_FRAME_WIDTH / 2., widest + point.y * (1. - widest));
	let from_side = |point: DVec2| DVec2::new(point.x / SIDE_FRAME_WIDTH / 2., point.y * widest);
	HeartHalf {
		cleft: from_lobe(lobe.cleft),
		cleft_out: from_lobe(lobe.cleft_out),
		widest_in: from_lobe(lobe.widest_in),
		widest: DVec2::new(0.5, widest),
		widest_out: from_side(DVec2::new(SIDE_FRAME_WIDTH, 1. - shoulder_length)),
		tip_in: from_side(tip_in),
	}
}

/// The neutral lobe, from the axis-aligned ellipse through the cleft that touches the top and is widest at the widest point: its cleft tangent elevation and exact cubic handle lengths.
struct EllipseLobe {
	elevation: f64,
	cleft_length: f64,
	widest_length: f64,
}

impl EllipseLobe {
	fn new(depth: f64) -> Self {
		let cleft_y = 1. - depth;
		let root = (1. - cleft_y * cleft_y).sqrt();
		let semi_axis = LOBE_FRAME_WIDTH / (1. + root);
		let span = PI - cleft_y.asin();
		let handle_factor = 4. / 3. * (span / 4.).tan();

		Self {
			elevation: root.atan2(semi_axis * cleft_y),
			cleft_length: handle_factor * (semi_axis * cleft_y).hypot(root),
			widest_length: handle_factor,
		}
	}
}

/// Cleft tangent elevation from 0 (a flat dip) to 90° (a cusp), rotated from the ellipse's in logit space so it stays strictly between them, with less room for shallow clefts.
fn cleft_elevation(neutral: f64, depth: f64, sharpness: f64) -> f64 {
	let neutral_share = soft_minimum(neutral / FRAC_PI_2, 0.98, 0.03);
	let room = 0.15 + 0.85 * depth;

	FRAC_PI_2 * logistic(0.75 * logit(neutral_share) + room * (2. * sharpness + 0.6 * sharpness * sharpness))
}

/// The lobe in the lobe frame for a lean (the log of the cleft handle's growth relative to the widest point's handle), uniformly scaled so its top lands exactly on y = 1.
fn solve_lobe(depth: f64, elevation: f64, neutral: &EllipseLobe, lean: f64) -> Lobe {
	let cleft_y = 1. - depth;
	let chord_angle = (-cleft_y).atan2(LOBE_FRAME_WIDTH);
	let chord = LOBE_FRAME_WIDTH.hypot(cleft_y);

	// Rotating the cleft handle away from the ellipse keeps its tension relative to a circular arc on the lobe's chord
	let rotation = arc_handle(chord, elevation - chord_angle) / arc_handle(chord, neutral.elevation - chord_angle);
	let cleft_length = neutral.cleft_length * rotation * (lean / 2.).exp();
	let widest_length = neutral.widest_length * (-lean / 2.).exp();

	// The cleft handle stops short of the widest point, so the lobe stays a graph over x
	let cleft_cap = 0.97 * LOBE_FRAME_WIDTH / elevation.cos();
	let (sine, cosine) = elevation.sin_cos();
	let peak_at = |scale: f64| {
		let cleft_handle_height = cleft_y + soft_cap(scale * cleft_length, cleft_cap, 6) * sine;
		CubicBez::new((0., cleft_y), (0., cleft_handle_height), (0., scale * widest_length), (0., 0.)).bounding_box().max_y()
	};

	// Bracket the scale by doubling, then solve within it
	let mut high = 1.;
	while peak_at(high) < 1. && high < 1e9 {
		high *= 2.;
	}
	let scale = solve_increasing(peak_at, 1., 0., high);
	let handle = soft_cap(scale * cleft_length, cleft_cap, 6);

	Lobe {
		cleft: DVec2::new(0., cleft_y),
		cleft_out: DVec2::new(handle * cosine, cleft_y + handle * sine),
		widest_in: DVec2::new(LOBE_FRAME_WIDTH, scale * widest_length),
	}
}

/// Side fullness for the narrow regime, re-centered toward fuller sides for narrow tips, with a span that narrows as the tip's tangent swings away from the side's chord.
fn effective_fullness(fullness: f64, tip_half_angle: f64) -> f64 {
	let deviation = (tip_half_angle - SIDE_FRAME_WIDTH.atan()) / SIDE_ANGLE_SCALE;

	0.5 - 0.2 * deviation.tanh() + (fullness - 0.5) / (1. + 0.6 * deviation * deviation)
}

/// Solves `increasing(x) = target` within [`low`, `high`] by the Illinois variant of regula falsi, returning the nearer end if the target lies outside.
fn solve_increasing(increasing: impl Fn(f64) -> f64, target: f64, low: f64, high: f64) -> f64 {
	let (mut low, mut high) = (low, high);
	let mut low_error = increasing(low) - target;
	let mut high_error = increasing(high) - target;
	if low_error >= 0. {
		return low;
	}
	if high_error <= 0. {
		return high;
	}

	let tolerance = 1e-13 * target.abs().max(1.);
	let mut last_moved_low = None;
	for _ in 0..200 {
		let x = (low * high_error - high * low_error) / (high_error - low_error);
		let error = increasing(x) - target;
		if error.abs() <= tolerance || high - low <= 1e-15 {
			return x;
		}

		// Halving the stale end's error when the same end moves twice keeps the convergence superlinear
		if error < 0. {
			low = x;
			low_error = error;
			if last_moved_low == Some(true) {
				high_error /= 2.;
			}
			last_moved_low = Some(true);
		} else {
			high = x;
			high_error = error;
			if last_moved_low == Some(false) {
				low_error /= 2.;
			}
			last_moved_low = Some(false);
		}
	}

	(low + high) / 2.
}

/// Handle length of a circular arc leaving at angle `alpha` to its chord.
fn arc_handle(chord: f64, alpha: f64) -> f64 {
	2. / 3. * chord / (1. + alpha.cos())
}

fn logistic(x: f64) -> f64 {
	1. / (1. + (-x).exp())
}

fn logit(p: f64) -> f64 {
	(p / (1. - p)).ln()
}

/// Numerically stable ln(1 + e^x).
fn softplus(x: f64) -> f64 {
	if x > 0. { x + (-x).exp().ln_1p() } else { x.exp().ln_1p() }
}

/// Smooth monotonic saturation of a positive value: about `x` when well below the cap, approaching the cap asymptotically.
fn soft_cap(x: f64, cap: f64, power: i32) -> f64 {
	x / (1. + (x / cap).powi(power)).powf(1. / power as f64)
}

/// Smooth minimum of two positive values, at or below both.
fn smooth_min(a: f64, b: f64) -> f64 {
	(a.powi(-8) + b.powi(-8)).powf(-1. / 8.)
}

/// Smooth minimum of `x` and a limit, at or below both, within about `softness` of the hard minimum.
fn soft_minimum(x: f64, limit: f64, softness: f64) -> f64 {
	limit - softness * softplus((limit - x) / softness)
}

/// Smooth maximum of two values, at or above both, within about `softness` of the hard maximum.
fn soft_maximum(a: f64, b: f64, softness: f64) -> f64 {
	a.max(b) + softness * (-(a - b).abs() / softness).exp().ln_1p()
}

/// Maps any value smoothly and monotonically into the open interval (`low`, `high`), nearly unchanged well inside it.
fn soft_clamp(x: f64, low: f64, high: f64) -> f64 {
	let t = (x - low) / (high - low);
	let above_zero = 0.04 * softplus(t / 0.04);

	low + (high - low) * soft_cap(above_zero, 1., 8)
}

#[cfg(test)]
mod tests {
	use super::*;
	use kurbo::{ParamCurveCurvature, PathSeg, Shape};

	const DEFAULT: HeartProportions = HeartProportions {
		cleft_depth: 0.7,
		cleft_sharpness: 0.,
		lobe_lean: 0.,
		widest_point: 0.7,
		side_fullness: 0.5,
		tip_angle: 75_f64.to_radians(),
	};
	const MINIMUM: HeartProportions = HeartProportions {
		cleft_depth: 0.2,
		cleft_sharpness: -1.,
		lobe_lean: -1.,
		widest_point: 0.58,
		side_fullness: 0.,
		tip_angle: 15_f64.to_radians(),
	};
	const MAXIMUM: HeartProportions = HeartProportions {
		cleft_depth: 1.1,
		cleft_sharpness: 1.,
		lobe_lean: 1.,
		widest_point: 0.8,
		side_fullness: 1.,
		tip_angle: 150_f64.to_radians(),
	};

	/// Writes and reads one of the proportions.
	type Proportion = (fn(&mut HeartProportions, f64), fn(&HeartProportions) -> f64);

	const PROPORTIONS: [Proportion; 6] = [
		(|proportions, value| proportions.cleft_depth = value, |proportions| proportions.cleft_depth),
		(|proportions, value| proportions.cleft_sharpness = value, |proportions| proportions.cleft_sharpness),
		(|proportions, value| proportions.lobe_lean = value, |proportions| proportions.lobe_lean),
		(|proportions, value| proportions.widest_point = value, |proportions| proportions.widest_point),
		(|proportions, value| proportions.side_fullness = value, |proportions| proportions.side_fullness),
		(|proportions, value| proportions.tip_angle = value, |proportions| proportions.tip_angle),
	];

	/// The default plus every corner of the domain, where the solves are pushed the hardest.
	fn sample_proportions() -> Vec<HeartProportions> {
		let corners = (0..1 << PROPORTIONS.len()).map(|corner: usize| {
			let mut proportions = DEFAULT;
			for (i, (set, get)) in PROPORTIONS.iter().enumerate() {
				set(&mut proportions, get(if corner & (1 << i) == 0 { &MINIMUM } else { &MAXIMUM }));
			}
			proportions
		});

		std::iter::once(DEFAULT).chain(corners).collect()
	}

	fn cubics(bezpath: &BezPath) -> Vec<CubicBez> {
		bezpath
			.segments()
			.map(|segment| match segment {
				PathSeg::Cubic(cubic) => cubic,
				_ => panic!("every heart segment should be a cubic"),
			})
			.collect()
	}

	#[test]
	fn heart_fills_its_box() {
		for proportions in sample_proportions() {
			let (bounds, half_size) = (heart_bezpath(&proportions).bounding_box(), HEART_SIZE / 2.);

			assert!(
				(bounds.x0 + half_size).abs() < 1e-9 && (bounds.x1 - half_size).abs() < 1e-9,
				"width bounds {bounds:?} for {proportions:?}"
			);
			assert!(
				(bounds.y0 + half_size).abs() < 1e-9 && (bounds.y1 - half_size).abs() < 1e-9,
				"height bounds {bounds:?} for {proportions:?}"
			);
		}
	}

	#[test]
	fn heart_curvature_is_continuous_at_the_widest_points() {
		for proportions in sample_proportions() {
			let cubics = cubics(&heart_bezpath(&proportions));
			assert_eq!(cubics.len(), 4);

			// Each widest point joins a lobe and a side, arriving and leaving in either order
			for (arriving, leaving) in [(cubics[0], cubics[1]), (cubics[2], cubics[3])] {
				let (arriving_curvature, leaving_curvature) = (arriving.curvature(1.), leaving.curvature(0.));
				let relative_difference = (arriving_curvature - leaving_curvature).abs() / arriving_curvature.abs().max(leaving_curvature.abs());

				assert!(relative_difference < 1e-9, "curvatures {arriving_curvature} and {leaving_curvature} for {proportions:?}");
			}
		}
	}

	#[test]
	fn heart_moves_continuously_with_every_proportion() {
		let control_points = |proportions: &HeartProportions| -> Vec<kurbo::Point> { cubics(&heart_bezpath(proportions)).iter().flat_map(|cubic| [cubic.p0, cubic.p1, cubic.p2]).collect() };

		// The largest movement of any control point between neighboring samples as one proportion sweeps its domain
		let largest_step = |baseline: &HeartProportions, (set, get): &Proportion, steps: usize| -> f64 {
			let (low, high) = (get(&MINIMUM), get(&MAXIMUM));
			let samples: Vec<_> = (0..=steps)
				.map(|step| {
					let mut proportions = *baseline;
					set(&mut proportions, low + (high - low) * step as f64 / steps as f64);
					control_points(&proportions)
				})
				.collect();

			samples
				.windows(2)
				.flat_map(|pair| pair[0].iter().zip(&pair[1]).map(|(before, after)| before.distance(*after)))
				.fold(0., f64::max)
		};

		// Doubling the sampling halves the largest step of a continuous motion, while a jump keeps its full size
		for baseline in [DEFAULT, MINIMUM, MAXIMUM] {
			for (index, proportion) in PROPORTIONS.iter().enumerate() {
				let (coarse, fine) = (largest_step(&baseline, proportion, 200), largest_step(&baseline, proportion, 400));

				assert!(fine < 0.6 * coarse, "largest steps {coarse} then {fine} sweeping proportion {index} from {baseline:?}");
			}
		}
	}
}
