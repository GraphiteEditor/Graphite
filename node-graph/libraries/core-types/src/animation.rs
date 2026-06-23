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
	pub const fn new() -> Self {
		Self { keyframes: Vec::new() }
	}

	pub fn evaluate(&self, time: f64) -> f64 {
		if self.keyframes.is_empty() || !time.is_finite() {
			return 0.0;
		}

		// keyframes have finite, real coordinates
		let index = self.keyframes.binary_search_by(|kf| kf.knot.x.partial_cmp(&time).unwrap_or(std::cmp::Ordering::Equal));

		// We are on a keyframe, use its knot
		if let Ok(idx) = index {
			return self.keyframes[idx].knot.y;
		}

		let index = index.unwrap_err();

		// Clamp to the first and last knot y-values when x is outside of all keyframes
		if index == 0 {
			return self.keyframes[0].knot.y;
		} else if index == self.keyframes.len() {
			// unwrap is safe because of the non-empty guard at the top
			return self.keyframes.last().unwrap().knot.y;
		}

		let segment_start = &self.keyframes[index - 1];
		let segment_end = &self.keyframes[index];

		match segment_start.interp_behavior {
			InterpolationBehavior::Bezier { right_handle } => {
				let start = segment_start.knot;
				let end = segment_end.knot;
				let left_handle = segment_end.left_handle.unwrap_or(end);

				// Clamp the handle x-coordinates of the handles to inside the segment.
				// This prevents the curve from folding over itself and having multiple values of t where x(t) == time.
				let right_x = right_handle.x.clamp(start.x, end.x);
				let left_x = left_handle.x.clamp(right_x, end.x);

				let curve = CubicBez::new(
					Point::new(start.x, start.y),
					Point::new(right_x, right_handle.y),
					Point::new(left_x, left_handle.y),
					Point::new(end.x, end.y),
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

	/// Pushes a new keyframe, overwriting one with the same x-value.
	/// Returns the index of the keyframe.
	///
	/// # Panics
	///
	/// This method panics if a keyframe with a non-finite x-coordinate is given.
	pub fn insert_keyframe(&mut self, keyframe: Keyframe) -> usize {
		assert!(keyframe.knot.x.is_finite(), "Keyframes must have a finite x-coordinate");

		match self.keyframes.binary_search_by(|kf| kf.knot.x.partial_cmp(&keyframe.knot.x).unwrap_or(std::cmp::Ordering::Equal)) {
			// Overwrite a keyframe with the same x-value
			Ok(idx) => {
				self.keyframes[idx] = keyframe;
				idx
			}
			Err(idx) => {
				self.keyframes.insert(idx, keyframe);
				idx
			}
		}
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
		single_kf.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(1.0, 10.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		assert_eq!(single_kf.evaluate(0.0), 10.0);
		assert_eq!(single_kf.evaluate(2.0), 10.0);
	}

	#[test]
	pub fn bezier_segment() {
		let mut anim_curve = AnimationCurve::new();
		anim_curve.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(0.0, 0.0),
			interp_behavior: InterpolationBehavior::Bezier { right_handle: DVec2::new(0.5, 0.0) },
		});
		anim_curve.insert_keyframe(Keyframe {
			left_handle: Some(DVec2::new(0.5, 1.0)),
			knot: DVec2::new(1.0, 1.0),
			interp_behavior: InterpolationBehavior::Constant,
		});

		assert_eq!(anim_curve.evaluate(0.5), 0.5);
		assert!((anim_curve.evaluate(0.25) - 0.104).abs() < 0.01);
		assert!((anim_curve.evaluate(0.75) - 0.896).abs() < 0.01);
	}

	#[test]
	pub fn simple_segments() {
		let mut anim_curve = AnimationCurve::new();
		anim_curve.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(0.0, 0.0),
			interp_behavior: InterpolationBehavior::Linear,
		});
		anim_curve.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(1.0, 1.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		anim_curve.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(2.0, 0.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		anim_curve.insert_keyframe(Keyframe {
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
	pub fn constant_segment() {
		let mut anim_curve = AnimationCurve::new();
		anim_curve.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(0.0, 0.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		anim_curve.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(1.0, 5.0),
			interp_behavior: InterpolationBehavior::Constant,
		});
		anim_curve.insert_keyframe(Keyframe {
			left_handle: None,
			knot: DVec2::new(2.0, -3.0),
			interp_behavior: InterpolationBehavior::Constant,
		});

		assert_eq!(anim_curve.evaluate(-1.0), 0.0);
		assert_eq!(anim_curve.evaluate(0.0), 0.0);
		assert_eq!(anim_curve.evaluate(0.5), 0.0);
		assert_eq!(anim_curve.evaluate(1.0), 5.0);
		assert_eq!(anim_curve.evaluate(2.0), -3.0);
	}
}
