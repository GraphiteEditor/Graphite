pub use dyn_any::StaticType;
use dyn_any::{DynAny, Upcast};
use dyn_clone::DynClone;
pub use glam::{DAffine2, DVec2};
use graphene_core::raster::{BlendMode, LuminanceCalculation};
use graphene_core::{Node, Type};
use std::hash::Hash;
pub use std::sync::Arc;

use crate::executor::Any;
pub use crate::imaginate_input::{ImaginateMaskStartingFill, ImaginateSamplingMethod, ImaginateStatus};

/// A type that is known, allowing serialization (serde::Deserialize is not object safe)
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TaggedValue {
	None,
	String(String),
	U32(u32),
	F32(f32),
	F64(f64),
	Bool(bool),
	DVec2(DVec2),
	OptionalDVec2(Option<DVec2>),
	DAffine2(DAffine2),
	Image(graphene_core::raster::Image),
	RcImage(Option<Arc<graphene_core::raster::Image>>),
	ImageFrame(graphene_core::raster::ImageFrame),
	Color(graphene_core::raster::color::Color),
	Subpath(bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>),
	RcSubpath(Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
	BlendMode(BlendMode),
	LuminanceCalculation(LuminanceCalculation),
	ImaginateSamplingMethod(ImaginateSamplingMethod),
	ImaginateMaskStartingFill(ImaginateMaskStartingFill),
	ImaginateStatus(ImaginateStatus),
	LayerPath(Option<Vec<u64>>),
	VectorData(graphene_core::vector::VectorData),
	Fill(graphene_core::vector::style::Fill),
	Stroke(graphene_core::vector::style::Stroke),
	VecF32(Vec<f32>),
	LineCap(graphene_core::vector::style::LineCap),
	LineJoin(graphene_core::vector::style::LineJoin),
	FillType(graphene_core::vector::style::FillType),
	GradientType(graphene_core::vector::style::GradientType),
	GradientPositions(Vec<(f64, Option<graphene_core::Color>)>),
}

#[allow(clippy::derived_hash_with_manual_eq)]
impl Hash for TaggedValue {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		match self {
			Self::None => 0.hash(state),
			Self::String(s) => {
				1.hash(state);
				s.hash(state)
			}
			Self::U32(u) => {
				2.hash(state);
				u.hash(state)
			}
			Self::F32(f) => {
				3.hash(state);
				f.to_bits().hash(state)
			}
			Self::F64(f) => {
				4.hash(state);
				f.to_bits().hash(state)
			}
			Self::Bool(b) => {
				5.hash(state);
				b.hash(state)
			}
			Self::DVec2(v) => {
				6.hash(state);
				v.to_array().iter().for_each(|x| x.to_bits().hash(state))
			}
			Self::OptionalDVec2(None) => 7.hash(state),
			Self::OptionalDVec2(Some(v)) => {
				8.hash(state);
				Self::DVec2(*v).hash(state)
			}
			Self::DAffine2(m) => {
				9.hash(state);
				m.to_cols_array().iter().for_each(|x| x.to_bits().hash(state))
			}
			Self::Image(i) => {
				10.hash(state);
				i.hash(state)
			}
			Self::RcImage(i) => {
				11.hash(state);
				i.hash(state)
			}
			Self::Color(c) => {
				12.hash(state);
				c.hash(state)
			}
			Self::Subpath(s) => {
				13.hash(state);
				s.hash(state)
			}
			Self::RcSubpath(s) => {
				14.hash(state);
				s.hash(state)
			}
			Self::BlendMode(b) => {
				15.hash(state);
				b.hash(state)
			}
			Self::LuminanceCalculation(l) => {
				16.hash(state);
				l.hash(state)
			}
			Self::ImaginateSamplingMethod(m) => {
				17.hash(state);
				m.hash(state)
			}
			Self::ImaginateMaskStartingFill(f) => {
				18.hash(state);
				f.hash(state)
			}
			Self::ImaginateStatus(s) => {
				19.hash(state);
				s.hash(state)
			}
			Self::LayerPath(p) => {
				20.hash(state);
				p.hash(state)
			}
			Self::ImageFrame(i) => {
				21.hash(state);
				i.image.hash(state);
				i.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state))
			}
			Self::VectorData(vector_data) => {
				22.hash(state);
				vector_data.subpaths.hash(state);
				vector_data.transform.to_cols_array().iter().for_each(|x| x.to_bits().hash(state));
				vector_data.style.hash(state);
			}
			Self::Fill(fill) => {
				23.hash(state);
				fill.hash(state);
			}
			Self::Stroke(stroke) => {
				24.hash(state);
				stroke.hash(state);
			}
			Self::VecF32(vec_f32) => {
				25.hash(state);
				vec_f32.iter().for_each(|val| val.to_bits().hash(state));
			}
			Self::LineCap(line_cap) => {
				26.hash(state);
				line_cap.hash(state);
			}
			Self::LineJoin(line_join) => {
				27.hash(state);
				line_join.hash(state);
			}
			Self::FillType(fill_type) => {
				28.hash(state);
				fill_type.hash(state);
			}
			Self::GradientType(gradient_type) => {
				29.hash(state);
				gradient_type.hash(state);
			}
			Self::GradientPositions(gradient_positions) => {
				30.hash(state);
				gradient_positions.len().hash(state);
				for (position, color) in gradient_positions {
					position.to_bits().hash(state);
					color.hash(state);
				}
			}
		}
	}
}

impl<'a> TaggedValue {
	/// Converts to a Box<dyn DynAny> - this isn't very neat but I'm not sure of a better approach
	pub fn to_any(self) -> Any<'a> {
		match self {
			TaggedValue::None => Box::new(()),
			TaggedValue::String(x) => Box::new(x),
			TaggedValue::U32(x) => Box::new(x),
			TaggedValue::F32(x) => Box::new(x),
			TaggedValue::F64(x) => Box::new(x),
			TaggedValue::Bool(x) => Box::new(x),
			TaggedValue::DVec2(x) => Box::new(x),
			TaggedValue::OptionalDVec2(x) => Box::new(x),
			TaggedValue::DAffine2(x) => Box::new(x),
			TaggedValue::Image(x) => Box::new(x),
			TaggedValue::RcImage(x) => Box::new(x),
			TaggedValue::ImageFrame(x) => Box::new(x),
			TaggedValue::Color(x) => Box::new(x),
			TaggedValue::Subpath(x) => Box::new(x),
			TaggedValue::RcSubpath(x) => Box::new(x),
			TaggedValue::BlendMode(x) => Box::new(x),
			TaggedValue::LuminanceCalculation(x) => Box::new(x),
			TaggedValue::ImaginateSamplingMethod(x) => Box::new(x),
			TaggedValue::ImaginateMaskStartingFill(x) => Box::new(x),
			TaggedValue::ImaginateStatus(x) => Box::new(x),
			TaggedValue::LayerPath(x) => Box::new(x),
			TaggedValue::VectorData(x) => Box::new(x),
			TaggedValue::Fill(x) => Box::new(x),
			TaggedValue::Stroke(x) => Box::new(x),
			TaggedValue::VecF32(x) => Box::new(x),
			TaggedValue::LineCap(x) => Box::new(x),
			TaggedValue::LineJoin(x) => Box::new(x),
			TaggedValue::FillType(x) => Box::new(x),
			TaggedValue::GradientType(x) => Box::new(x),
			TaggedValue::GradientPositions(x) => Box::new(x),
		}
	}

	pub fn ty(&self) -> Type {
		use graphene_core::TypeDescriptor;
		use std::borrow::Cow;
		match self {
			TaggedValue::None => concrete!(()),
			TaggedValue::String(_) => concrete!(String),
			TaggedValue::U32(_) => concrete!(u32),
			TaggedValue::F32(_) => concrete!(f32),
			TaggedValue::F64(_) => concrete!(f64),
			TaggedValue::Bool(_) => concrete!(bool),
			TaggedValue::DVec2(_) => concrete!(DVec2),
			TaggedValue::OptionalDVec2(_) => concrete!(Option<DVec2>),
			TaggedValue::Image(_) => concrete!(graphene_core::raster::Image),
			TaggedValue::RcImage(_) => concrete!(Option<Arc<graphene_core::raster::Image>>),
			TaggedValue::ImageFrame(_) => concrete!(graphene_core::raster::ImageFrame),
			TaggedValue::Color(_) => concrete!(graphene_core::raster::Color),
			TaggedValue::Subpath(_) => concrete!(bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>),
			TaggedValue::RcSubpath(_) => concrete!(Arc<bezier_rs::Subpath<graphene_core::uuid::ManipulatorGroupId>>),
			TaggedValue::BlendMode(_) => concrete!(BlendMode),
			TaggedValue::ImaginateSamplingMethod(_) => concrete!(ImaginateSamplingMethod),
			TaggedValue::ImaginateMaskStartingFill(_) => concrete!(ImaginateMaskStartingFill),
			TaggedValue::ImaginateStatus(_) => concrete!(ImaginateStatus),
			TaggedValue::LayerPath(_) => concrete!(Option<Vec<u64>>),
			TaggedValue::DAffine2(_) => concrete!(DAffine2),
			TaggedValue::LuminanceCalculation(_) => concrete!(LuminanceCalculation),
			TaggedValue::VectorData(_) => concrete!(graphene_core::vector::VectorData),
			TaggedValue::Fill(_) => concrete!(graphene_core::vector::style::Fill),
			TaggedValue::Stroke(_) => concrete!(graphene_core::vector::style::Stroke),
			TaggedValue::VecF32(_) => concrete!(Vec<f32>),
			TaggedValue::LineCap(_) => concrete!(graphene_core::vector::style::LineCap),
			TaggedValue::LineJoin(_) => concrete!(graphene_core::vector::style::LineJoin),
			TaggedValue::FillType(_) => concrete!(graphene_core::vector::style::FillType),
			TaggedValue::GradientType(_) => concrete!(graphene_core::vector::style::GradientType),
			TaggedValue::GradientPositions(_) => concrete!(Vec<(f64, Option<graphene_core::Color>)>),
		}
	}
}

pub struct UpcastNode {
	value: TaggedValue,
}
impl<'input> Node<'input, Box<dyn DynAny<'input> + 'input>> for UpcastNode {
	type Output = Box<dyn DynAny<'input> + 'input>;

	fn eval<'s: 'input>(&'s self, _: Box<dyn DynAny<'input> + 'input>) -> Self::Output {
		self.value.clone().to_any()
	}
}
impl UpcastNode {
	pub fn new(value: TaggedValue) -> Self {
		Self { value }
	}
}

pub type Value<'a> = Box<dyn for<'i> ValueTrait<'i> + 'a>;

pub trait ValueTrait<'a>: DynAny<'a> + Upcast<dyn DynAny<'a> + 'a> + std::fmt::Debug + DynClone + Sync + Send + 'a {}

pub trait IntoValue<'a>: Sized + for<'i> ValueTrait<'i> + 'a {
	fn into_any(self) -> Value<'a> {
		Box::new(self)
	}
}

impl<'a, T: 'a + StaticType + Upcast<dyn DynAny<'a> + 'a> + std::fmt::Debug + PartialEq + Clone + Sync + Send + 'a> ValueTrait<'a> for T {}

impl<'a, T: for<'i> ValueTrait<'i> + 'a> IntoValue<'a> for T {}

#[repr(C)]
pub(crate) struct Vtable {
	pub(crate) destructor: unsafe fn(*mut ()),
	pub(crate) size: usize,
	pub(crate) align: usize,
}

#[repr(C)]
pub(crate) struct TraitObject {
	pub(crate) self_ptr: *mut u8,
	pub(crate) vtable: &'static Vtable,
}

impl<'a> PartialEq for Box<dyn for<'i> ValueTrait<'i> + 'a> {
	#[cfg_attr(miri, ignore)]
	fn eq(&self, other: &Self) -> bool {
		if self.type_id() != other.type_id() {
			return false;
		}
		let self_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(self.as_ref()) };
		let other_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(other.as_ref()) };
		let size = self_trait_object.vtable.size;
		let self_mem = unsafe { std::slice::from_raw_parts(self_trait_object.self_ptr, size) };
		let other_mem = unsafe { std::slice::from_raw_parts(other_trait_object.self_ptr, size) };
		self_mem == other_mem
	}
}

impl<'a> Hash for Value<'a> {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		let self_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(self.as_ref()) };
		let size = self_trait_object.vtable.size;
		let self_mem = unsafe { std::slice::from_raw_parts(self_trait_object.self_ptr, size) };
		self_mem.hash(state);
	}
}

impl<'a> Clone for Value<'a> {
	fn clone(&self) -> Self {
		let self_trait_object = unsafe { std::mem::transmute::<&dyn ValueTrait, TraitObject>(self.as_ref()) };
		let size = self_trait_object.vtable.size;
		let self_mem = unsafe { std::slice::from_raw_parts(self_trait_object.self_ptr, size) }.to_owned();
		let ptr = Vec::leak(self_mem);
		unsafe {
			std::mem::transmute(TraitObject {
				self_ptr: ptr as *mut [u8] as *mut u8,
				vtable: self_trait_object.vtable,
			})
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	#[cfg_attr(miri, ignore)]
	fn test_any_src() {
		assert!(2_u32.into_any() == 2_u32.into_any());
		assert!(2_u32.into_any() != 3_u32.into_any());
		assert!(2_u32.into_any() != 3_i32.into_any());
	}
}
