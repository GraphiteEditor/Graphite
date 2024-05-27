use core::fmt::Debug;
use std::hash::Hash;

use dyn_any::{DynAny, StaticType};

use crate::Node;

#[derive(Debug, Copy, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyframeF64 {
	pub time: f64,
	pub value: f64,
	// TODO: support different types of easing
	// pub easing: Easing,
}

impl KeyframeF64 {
	pub fn new(time: f64, value: f64) -> Self {
		Self { time, value }
	}
}

#[derive(Debug, Clone, PartialEq, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct KeyframesF64 {
	pub keyframes: Vec<KeyframeF64>,
}

impl KeyframesF64 {
	pub fn new(keyframes: Vec<KeyframeF64>) -> Self {
		Self { keyframes }
	}

	pub fn get_value(&self, time: f64) -> f64 {
		if self.keyframes.len() == 0 {
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
		return Self::interpolate(k1, k2, time);
	}

	fn interpolate(k1: &KeyframeF64, k2: &KeyframeF64, time: f64) -> f64 {
		assert!(k1.time < time && time < k2.time);
		let t = (time - k1.time) / (k2.time - k1.time);
		return k1.value + (k2.value - k1.value) * t;
	}
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct AnimationIdentityNode;
impl<'i, O: 'i> Node<'i, O> for AnimationIdentityNode {
	type Output = O;
	fn eval(&'i self, input: O) -> Self::Output {
		input
	}
}
impl AnimationIdentityNode {
	pub fn new() -> Self {
		Self
	}
}
