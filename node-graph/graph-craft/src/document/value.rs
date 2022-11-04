use dyn_any::StaticType;

use dyn_clone::DynClone;

use dyn_any::{DynAny, Upcast};

pub type Value = Box<dyn ValueTrait>;

pub trait ValueTrait: DynAny<'static> + Upcast<dyn DynAny<'static>> + std::fmt::Debug + DynClone {}

pub trait IntoValue: Sized + ValueTrait + 'static {
	fn into_any(self) -> Value {
		Box::new(self)
	}
}

impl<T: 'static + StaticType + Upcast<dyn DynAny<'static>> + std::fmt::Debug + PartialEq + Clone> ValueTrait for T {}

impl<T: 'static + ValueTrait> IntoValue for T {}

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

impl PartialEq for Box<dyn ValueTrait> {
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

impl Clone for Value {
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
	fn test_any_src() {
		assert!(2_u32.into_any() == 2_u32.into_any());
		assert!(2_u32.into_any() != 3_u32.into_any());
		assert!(2_u32.into_any() != 3_i32.into_any());
	}
}
