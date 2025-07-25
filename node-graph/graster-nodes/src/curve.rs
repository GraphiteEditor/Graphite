use dyn_any::{DynAny, StaticType, StaticTypeSized};
use graphene_core::Node;
use graphene_core::color::{Channel, Linear, LuminanceMut};
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, PartialEq, DynAny, specta::Type, serde::Serialize, serde::Deserialize)]
pub struct Curve {
	#[serde(rename = "manipulatorGroups")]
	pub manipulator_groups: Vec<CurveManipulatorGroup>,
	#[serde(rename = "firstHandle")]
	pub first_handle: [f32; 2],
	#[serde(rename = "lastHandle")]
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

impl Hash for Curve {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.manipulator_groups.hash(state);
		[self.first_handle, self.last_handle].iter().flatten().for_each(|f| f.to_bits().hash(state));
	}
}

#[derive(Debug, Clone, Copy, PartialEq, DynAny, specta::Type, serde::Serialize, serde::Deserialize)]
pub struct CurveManipulatorGroup {
	pub anchor: [f32; 2],
	pub handles: [[f32; 2]; 2],
}

impl Hash for CurveManipulatorGroup {
	fn hash<H: Hasher>(&self, state: &mut H) {
		for c in self.handles.iter().chain([&self.anchor]).flatten() {
			c.to_bits().hash(state);
		}
	}
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
