//! Animation Curve implementation based off of Blender's fcurves.
//!

use dyn_any::DynAny;

use glam::DVec2;
use graphene_hash::CacheHash;
use kurbo::{CubicBez, ParamCurve, Point};

// Every keyframe defines a left handle point for any bezier easings to the left,
// and info defining the behavior to the right hand side of the keyframe
#[derive(Debug, Clone, Copy, PartialEq, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Keyframe {
	/// If None, defaults to knot in the case of a bezier keyframe to the left.
	pub left_handle: Option<DVec2>,
	pub knot: DVec2,
	pub interp_behavior: InterpolationBehavior,
}
impl Keyframe {
	pub fn new_linear(knot: DVec2, left_handle: Option<DVec2>) -> Self {
		Self {
			left_handle,
			knot,
			interp_behavior: InterpolationBehavior::Linear,
		}
	}
	pub fn new_constant(knot: DVec2, left_handle: Option<DVec2>) -> Self {
		Self {
			left_handle,
			knot,
			interp_behavior: InterpolationBehavior::Constant,
		}
	}
	pub fn new_bezier(knot: DVec2, left_handle: Option<DVec2>, right_handle: DVec2) -> Self {
		Self {
			left_handle,
			knot,
			interp_behavior: InterpolationBehavior::Bezier { right_handle },
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum InterpolationBehavior {
	Bezier { right_handle: DVec2 },
	Constant,
	Linear,
}

#[derive(Default, Debug, Clone, PartialEq, DynAny, CacheHash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AnimationCurve {
	keyframes: Vec<Keyframe>, // not public to maintain sorted order
}

impl AnimationCurve {
	pub fn new() -> Self {
		Self { keyframes: Vec::new() }
	}

	pub fn evaluate(&self, time: f64) -> f64 {
		if self.keyframes.is_empty() || !time.is_finite() {
			return 0.0;
		}

		// keyframes should (hopefully) have finite, real coordinates
		let index = self.keyframes.binary_search_by(|kf| kf.knot.x.partial_cmp(&time).unwrap_or(std::cmp::Ordering::Equal));

		// We are on a keyframe, use its knot
		if let Ok(idx) = index {
			return self.keyframes[idx].knot.y;
		}

		let index = index.unwrap_err();

		if index == 0 {
			return 0.0;
		} else if index == self.keyframes.len() {
			// unwrap is safe because of the non-empty guard at the top
			return self.keyframes.last().unwrap().knot.y;
		}

		let segment_start = &self.keyframes[index - 1];
		let segment_end = &self.keyframes[index];

		match segment_start.interp_behavior {
			InterpolationBehavior::Bezier { right_handle } => {
				let to_point = |vec: DVec2| Point::new(vec.x, vec.y);

				let curve = CubicBez::new(
					to_point(segment_start.knot),
					to_point(right_handle),
					segment_end.left_handle.map(|end| to_point(end)).unwrap_or_else(|| to_point(segment_end.knot)),
					to_point(segment_end.knot),
				);

				// Find the value of t where curve.x == time to find the value
				//TODO: find proper values for epsilon and k1. The docs suggest 0.2 for k1 but epsilon should be tested with several values
				let t = kurbo::common::solve_itp(|t| curve.eval(t).x - time, 0.0, 1.0, 0.00001, 1, 0.2, segment_start.knot.x - time, segment_end.knot.x - time);

				curve.eval(t).y
			}
			InterpolationBehavior::Constant => segment_start.knot.y,
			InterpolationBehavior::Linear => {
				let start = segment_start.knot.y;
				let end = segment_end.knot.y;
				let i = (time - segment_start.knot.x) / (segment_end.knot.x - segment_start.knot.x);

				start + (end - start) * i
			}
		}
	}

	pub fn keyframes(&self) -> &[Keyframe] {
		&self.keyframes
	}

	pub fn push_keyframe(&mut self, keyframe: Keyframe) {
		self.keyframes.push(keyframe);
		self.keyframes.sort_by(|lhs, rhs| lhs.knot.x.partial_cmp(&rhs.knot.x).unwrap_or(std::cmp::Ordering::Equal));
	}
	pub fn remove_keyframe(&mut self, idx: usize) -> Option<Keyframe> {
		if idx >= self.keyframes.len() {
			return None;
		}
		Some(self.keyframes.remove(idx))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	pub fn out_of_bounds() {
		let empty_curve = AnimationCurve::new();
		assert_eq!(empty_curve.evaluate(10.0), 0.0);

		let mut single_kf = AnimationCurve::new();
		single_kf.push_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(1.0, 10.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		assert_eq!(single_kf.evaluate(0.0), 0.0);
		assert_eq!(single_kf.evaluate(2.0), 10.0);
	}

	#[test]
	pub fn bezier_segment() {
		let mut anim_curve = AnimationCurve::new();
		anim_curve.push_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(0.0, 0.0),
			interp_behavior: InterpolationBehavior::Bezier { right_handle: DVec2::new(0.5, 0.0) },
		});
		anim_curve.push_keyframe(Keyframe {
			left_handle: Some(DVec2::new(0.5, 1.0)),
			knot: DVec2::new(1.0, 1.0),
			interp_behavior: InterpolationBehavior::Constant,
		});

		assert_eq!(anim_curve.evaluate(0.5), 0.5);
		assert!(anim_curve.evaluate(0.25) - 0.104 < 0.01);
		assert!(anim_curve.evaluate(0.75) - 0.896 < 0.01);
	}

	#[test]
	pub fn simple_segments() {
		let mut anim_curve = AnimationCurve::new();
		anim_curve.push_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(0.0, 0.0),
			interp_behavior: InterpolationBehavior::Linear,
		});
		anim_curve.push_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(1.0, 1.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		anim_curve.push_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(2.0, 0.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		anim_curve.push_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(3.0, 1.0),
			interp_behavior: InterpolationBehavior::Constant,
		});

		assert_eq!(anim_curve.evaluate(0.5), 0.5);
		assert_eq!(anim_curve.evaluate(0.25), 0.25);
		assert_eq!(anim_curve.evaluate(0.75), 0.75);

		assert_eq!(anim_curve.evaluate(2.5), 0.0);
	}

	#[test]
	pub fn constant_segment() {}
}
