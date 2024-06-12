use bezier_rs::{Bezier, TValue};
use dyn_any::{DynAny, StaticType};

use super::keyframe::KeyframeF64;

/// It consists of a normalised cubic bezier curve in 2-D, with the Y-axis denoting the value and the X-axis denoting the time.
///
/// The anchor points are at `(0, 0)` and `(1, 1)`. The x-values of the handles must in the range `[0, 1]`
/// to prevent the curve from doubling on itself (which would result in two values for a single time).
///
/// Since bezier curves are invariant under affine transformations, we can use the same normalised curve for interpolating between
/// any time-value endpoints.
#[derive(Debug, Copy, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BezierEasing {
	curve: Bezier,
}

impl BezierEasing {
	pub fn new(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
		assert!(x1 >= 0.0 && x1 <= 1.0 && x2 >= 0.0 && x2 <= 1.0);
		Self {
			curve: Bezier::from_cubic_coordinates(0., 0., x1, y1, x2, y2, 1., 1.),
		}
	}

	/// Finds the y-value for a given x-value on the (normalised) curve.
	fn interpolate(&self, x: f64) -> f64 {
		let t = self.curve.find_tvalues_for_x(x).next().unwrap();
		self.curve.evaluate(TValue::Parametric(t)).y
	}
}

#[derive(Debug, Copy, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Easing {
	Linear,
	Bezier(BezierEasing),
}

impl Easing {
	pub fn interpolate(k1: &KeyframeF64, k2: &KeyframeF64, time: f64) -> f64 {
		assert!(k1.time < time && time < k2.time);
		match k1.easing {
			Easing::Linear => {
				let t = (time - k1.time) / (k2.time - k1.time);
				k1.value + (k2.value - k1.value) * t
			}
			Easing::Bezier(bezier) => {
				let normalized_time = (time - k1.time) / (k2.time - k1.time);
				let normalized_value = bezier.interpolate(normalized_time);
				k1.value + (k2.value - k1.value) * normalized_value
			}
		}
	}
}
