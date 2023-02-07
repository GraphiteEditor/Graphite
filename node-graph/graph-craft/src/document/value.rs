pub use dyn_any::StaticType;
use dyn_any::{DynAny, Upcast};
use dyn_clone::DynClone;
pub use glam::{DAffine2, DVec2};
use graphene_core::Node;
use std::hash::Hash;
pub use std::sync::Arc;

use crate::executor::Any;
pub use crate::imaginate_input::{ImaginateMaskStartingFill, ImaginateSamplingMethod, ImaginateStatus};

/// A type that is known, allowing serialization (serde::Deserialize is not object safe)
#[derive(Clone, Debug, PartialEq, specta::Type)]
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
	Color(graphene_core::raster::color::Color),
	Subpath(graphene_core::vector::subpath::Subpath),
	RcSubpath(Arc<graphene_core::vector::subpath::Subpath>),
	ImaginateSamplingMethod(ImaginateSamplingMethod),
	ImaginateMaskStartingFill(ImaginateMaskStartingFill),
	ImaginateStatus(ImaginateStatus),
	LayerPath(Option<Vec<u64>>),
}

#[allow(clippy::derive_hash_xor_eq)]
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
				9.hash(state);
				i.hash(state)
			}
			Self::RcImage(i) => {
				10.hash(state);
				i.hash(state)
			}
			Self::Color(c) => {
				11.hash(state);
				c.hash(state)
			}
			Self::Subpath(s) => {
				12.hash(state);
				s.hash(state)
			}
			Self::RcSubpath(s) => {
				13.hash(state);
				s.hash(state)
			}
			Self::ImaginateSamplingMethod(m) => {
				14.hash(state);
				m.hash(state)
			}
			Self::ImaginateMaskStartingFill(f) => {
				15.hash(state);
				f.hash(state)
			}
			Self::ImaginateStatus(s) => {
				16.hash(state);
				s.hash(state)
			}
			Self::LayerPath(p) => {
				17.hash(state);
				p.hash(state)
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
			TaggedValue::Color(x) => Box::new(x),
			TaggedValue::Subpath(x) => Box::new(x),
			TaggedValue::RcSubpath(x) => Box::new(x),
			TaggedValue::ImaginateSamplingMethod(x) => Box::new(x),
			TaggedValue::ImaginateMaskStartingFill(x) => Box::new(x),
			TaggedValue::ImaginateStatus(x) => Box::new(x),
			TaggedValue::LayerPath(x) => Box::new(x),
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
