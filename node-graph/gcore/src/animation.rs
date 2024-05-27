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
