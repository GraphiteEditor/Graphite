use core_types::Node;
use core_types::color::{Channel, Linear, LuminanceMut};
use dyn_any::{DynAny, StaticType, StaticTypeSized};
use std::ops::{Add, Mul, Sub};

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, PartialEq, core_types::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Curve {
	#[cfg_attr(feature = "serde", serde(rename = "manipulatorGroups"))]
	pub manipulator_groups: Vec<CurveManipulatorGroup>,
	#[cfg_attr(feature = "serde", serde(rename = "firstHandle"))]
	pub first_handle: [f32; 2],
	#[cfg_attr(feature = "serde", serde(rename = "lastHandle"))]
	pub last_handle: [f32; 2],
}

impl Default for Curve {
	fn default() -> Self {
		Self {
			manipulator_groups: vec![],
			first_handle: [0.2; 2],
			last_handle: [0.8; 2],
		}
	}
}

#[cfg_attr(feature = "wasm", derive(tsify::Tsify))]
#[derive(Debug, Clone, Copy, PartialEq, core_types::CacheHash, DynAny)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurveManipulatorGroup {
	pub anchor: [f32; 2],
	pub handles: [[f32; 2]; 2],
}

pub struct ValueMapperNode<C> {
	lut: Vec<C>,
}

unsafe impl<C: StaticTypeSized> StaticType for ValueMapperNode<C> {
	type Static = ValueMapperNode<C::Static>;
}

impl<C> ValueMapperNode<C> {
	pub const fn new(lut: Vec<C>) -> Self {
		Self { lut }
	}
}

impl<'i, L: LuminanceMut + 'i> Node<'i, L> for ValueMapperNode<L::LuminanceChannel>
where
	L::LuminanceChannel: Linear + Copy,
	L::LuminanceChannel: Add<Output = L::LuminanceChannel>,
	L::LuminanceChannel: Sub<Output = L::LuminanceChannel>,
	L::LuminanceChannel: Mul<Output = L::LuminanceChannel>,
{
	type Output = L;

	fn eval(&'i self, mut val: L) -> L {
		let luminance: f32 = val.luminance().to_linear();
		let floating_sample_index = luminance * (self.lut.len() - 1) as f32;
		let index_in_lut = floating_sample_index.floor() as usize;
		let a = self.lut[index_in_lut];
		let b = self.lut[(index_in_lut + 1).clamp(0, self.lut.len() - 1)];
		let result = a.lerp(b, L::LuminanceChannel::from_linear(floating_sample_index.fract()));
		val.set_luminance(result);
		val
	}
}
