use core::fmt::Debug;

use dyn_any::{DynAny, StaticType};

use super::easing::Easing;

#[derive(Debug, Copy, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyframeF64 {
	pub time: f64,
	pub value: f64,
	/// Easing from this keyframe to the next one
	pub easing: Easing,
}

impl KeyframeF64 {
	pub fn new(time: f64, value: f64, easing: Easing) -> Self {
		Self { time, value, easing }
	}
}

impl Default for KeyframeF64 {
	fn default() -> Self {
		Self {
			time: 0.,
			value: 0.,
			easing: Easing::Linear,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Default, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyframesF64 {
	pub keyframes: Vec<KeyframeF64>,
}

impl KeyframesF64 {
	pub fn new(keyframes: Vec<KeyframeF64>) -> Self {
		Self { keyframes }
	}

	pub fn get_value_at_time(&self, time: f64) -> f64 {
		if self.keyframes.is_empty() {
			return 0.;
		}
		if time <= self.keyframes[0].time {
			return self.keyframes[0].value;
		}
		if time >= self.keyframes[self.keyframes.len() - 1].time {
			return self.keyframes[self.keyframes.len() - 1].value;
		}
		// `partition_point` returns the first index for which the predicate is false
		// so, `ind` is the first index for which k.time >= time
		let ind = self.keyframes.partition_point(|k| k.time < time);
		if self.keyframes[ind].time == time {
			return self.keyframes[ind].value;
		}
		// ind > 0 because we already checked the first keyframe
		assert!(ind > 0);
		let k1 = &self.keyframes[ind - 1];
		let k2 = &self.keyframes[ind];

		Easing::interpolate(k1, k2, time)
	}
}
