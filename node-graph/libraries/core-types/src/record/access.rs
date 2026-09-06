//! Raw typed access to a record at a wiring-proven layout.

use super::layout::element_parked;
use crate::attribute;
use crate::gpoll::GPoll;

/// A view of one record: a pointer whose layout is proven at wiring, borrowing
/// the storage it points into for `'r`.
#[derive(Clone, Copy, Debug)]
pub struct Rec<'r>(pub(in crate::record) *const u8, pub(in crate::record) std::marker::PhantomData<&'r u8>);

impl<'r> Rec<'r> {
	/// # Safety
	/// `ptr` must point to a live record of the layout the consumer resolved
	/// at wiring, valid for `'r` and until the owning slot is next written.
	pub unsafe fn new(ptr: *const u8) -> Self {
		Rec(ptr, std::marker::PhantomData)
	}

	/// # Safety
	/// `offset` must be a field offset of the record's layout and `T` the
	/// field's type; both are proven at wiring. The record's base is aligned
	/// to its layout, so field reads are aligned.
	pub unsafe fn read<T: Copy>(self, offset: usize) -> T {
		unsafe { self.0.add(offset).cast::<T>().read() }
	}

	/// # Safety
	/// `T` must be the record's element type; the element sits at offset 0.
	pub unsafe fn element<T: Copy>(self) -> T {
		unsafe { self.read(0) }
	}

	pub fn ptr(self) -> *const u8 {
		self.0
	}
}

/// An opaque record value: every non-empty record spills to a claimed frame
/// and the value carries its pointer, while an empty record carries nothing.
#[derive(Clone, Copy)]
pub struct RecordValue<'e> {
	pub(in crate::record) ptr: *const u8,
	_lifetime: std::marker::PhantomData<&'e ()>,
}

impl std::fmt::Debug for RecordValue<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("RecordValue(..)")
	}
}

// SAFETY: `element_write` requires the element `Send + Sync` and attribute payloads
// are `Copy` or arena-backed, so the record bytes behind the pointer are thread-safe;
// `'e` ties the pointer's validity to the shared arena and record-stack discipline.
unsafe impl Send for RecordValue<'_> {}
// SAFETY: as `Send`.
unsafe impl Sync for RecordValue<'_> {}

impl<'e> RecordValue<'e> {
	#[doc(hidden)]
	pub fn zeroed() -> Self {
		RecordValue {
			ptr: std::ptr::null(),
			_lifetime: std::marker::PhantomData,
		}
	}

	/// The inline storage under construction; writes land in the value itself.
	#[doc(hidden)]
	pub fn as_mut_ptr(&mut self) -> *mut u8 {
		(&raw mut *self).cast()
	}

	#[doc(hidden)]
	pub fn spilled(rec: Rec<'_>) -> Self {
		RecordValue {
			ptr: rec.ptr(),
			_lifetime: std::marker::PhantomData,
		}
	}

	/// Rebinds the eval lifetime, lengthening a derived scope's record back
	/// onto the outer evaluation's. Sound only because the record's bytes live
	/// in the outer caller's slot region: a derived context shortens the
	/// caller's frame space rather than owning any, so the frame the record
	/// sits in outlives the derivation, exactly as it outlives the arena
	/// borrow the derived context carries.
	pub(in crate::record) fn rebind<'a>(self) -> RecordValue<'a> {
		RecordValue {
			ptr: self.ptr,
			_lifetime: std::marker::PhantomData,
		}
	}
}

/// Reads a declared attribute out of a record at a wiring-resolved offset,
/// falling back to the marker's census default where the layout does not carry
/// the name.
///
/// # Safety
/// `rec` must be a live record of the layout `offset` was resolved against,
/// and `offset`, where present, must be that layout's offset for `A` at the
/// read's level. The census admits one value type per attribute name and
/// panics on a conflicting declaration, so that field's bytes are a value of
/// `A::Value`, differing from the read type only in `'e`, which must not
/// outlive the evaluation a parked payload is arena-resident for.
pub unsafe fn read_at<'e, A: attribute::Attribute>(rec: Rec<'_>, offset: Option<usize>) -> attribute::Attr<'e, A> {
	attribute::Attr(match offset {
		// SAFETY: the caller's contract.
		Some(offset) => unsafe { rec.read::<A::Value<'e>>(offset) },
		None => A::default(),
	})
}

/// The read-less [`DerivedLazyInput`] glue: the token alone.
///
/// # Safety
/// `rec` must be a spilled record's frame.
pub unsafe fn token_only<'e>(rec: Rec<'_>, _reads: &[Option<usize>]) -> RecordValue<'e> {
	RecordValue::spilled(rec)
}

/// # Safety
/// `offset` must be a field offset of the layout of the record under
/// construction at `dst` and `T` the field's type; both are proven at wiring.
pub unsafe fn write_field<T>(dst: *mut u8, offset: usize, value: T) {
	unsafe { dst.add(offset).cast::<T>().write(value) }
}

/// # Safety
/// The record's element must be a `T` in the form [`element_parked`] picks,
/// and the borrow is only valid while the record is.
pub unsafe fn borrow_element<'e, T>(rec: Rec<'_>) -> &'e T {
	match element_parked::<T>() {
		true => unsafe { rec.element::<&T>() },
		false => unsafe { &*rec.ptr().cast::<T>() },
	}
}

/// # Safety
/// The record's element must be a `T` in the form [`element_parked`] picks.
pub unsafe fn read_element<T: Clone>(rec: Rec<'_>) -> T {
	unsafe { borrow_element::<T>(rec) }.clone()
}

/// # Safety
/// `dst` must be fresh element storage of a record whose element is `T`.
/// `None` reports arena exhaustion for a parked element.
pub unsafe fn write_element<T: Send + Sync + dyn_any::StaticTypeSized>(dst: *mut u8, value: T, arena: &crate::arena::Arena) -> Option<()> {
	unsafe { write_element_sized(dst, value, arena, 0) }
}

/// [`write_element`] with the park glue's estimate of the heap `value` owns.
///
/// # Safety
/// As [`write_element`].
pub unsafe fn write_element_sized<T: Send + Sync + dyn_any::StaticTypeSized>(dst: *mut u8, value: T, arena: &crate::arena::Arena, retained: usize) -> Option<()> {
	// SAFETY: the caller's contract; `T::Static` is the element type's own key.
	unsafe { write_element_keyed(dst, value, arena, retained, std::any::TypeId::of::<T::Static>()) }
}

/// [`write_element_sized`] for replay glue already holding the element's
/// static form, whose key the element type it replays into supplies.
///
/// # Safety
/// As [`write_element`], and `type_of` must be that element type's key, since
/// a park carrying it is what [`Promotion::move_park`] moves at.
pub(in crate::record) unsafe fn write_element_keyed<T: Send + Sync>(dst: *mut u8, value: T, arena: &crate::arena::Arena, retained: usize, type_of: std::any::TypeId) -> Option<()> {
	match element_parked::<T>() {
		true => {
			let (parked, _) = arena.alloc_sized_as(value, retained, type_of)?;
			unsafe { dst.cast::<&T>().write(parked) };
			Some(())
		}
		false => {
			unsafe { dst.cast::<T>().write(value) };
			Some(())
		}
	}
}

/// Finishes a carried record frame: the element lands beside the fields
/// already carried into `dst`, and inline frames copy out of the scratch
/// bytes. Arena exhaustion of a parked element reports as an error poll.
///
/// # Safety
/// `dst` must be the claimed frame (or inline scratch when `frame_bytes` is
/// 0) of a record whose element is `T` and whose frame size is `frame_bytes`,
/// with every carried field already written.
pub(in crate::record) unsafe fn lift_poll_into<'e, T: Send + Sync + dyn_any::StaticTypeSized>(
	poll: GPoll<T>,
	dst: *mut u8,
	frame_bytes: usize,
	arena: &'e crate::arena::Arena,
) -> GPoll<RecordValue<'e>> {
	let build = |element: T| {
		let written = unsafe { write_element(dst, element, arena) };
		written.map(|()| match frame_bytes {
			0 => unsafe { dst.cast::<RecordValue>().read() },
			_ => RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) }),
		})
	};
	let exhausted = || {
		GPoll::Error(Box::new(crate::gpoll::GraphError {
			kind: crate::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		}))
	};
	match poll {
		GPoll::Final(element) => build(element).map_or_else(exhausted, GPoll::Final),
		GPoll::Partial(element) => build(element).map_or_else(exhausted, GPoll::Partial),
		GPoll::Fallback(boxed) => {
			let (element, error) = *boxed;
			build(element).map_or_else(exhausted, |value| GPoll::Fallback(Box::new((value, error))))
		}
		GPoll::Pending => GPoll::Pending,
		GPoll::Error(error) => GPoll::Error(error),
	}
}

/// # Safety
/// `src` must be a record of the plan's source layout and `dst` a buffer of
/// the plan's target layout; both are proven at wiring.
pub unsafe fn apply_plan(src: Rec<'_>, dst: *mut u8, plan: &[(usize, usize, usize)]) {
	for &(from, to, size) in plan {
		unsafe { std::ptr::copy_nonoverlapping(src.ptr().add(from), dst.add(to), size) };
	}
}

/// The value with its lifetimes substituted by `'static`, for erased storage
/// whose reads re-bind a live lifetime.
///
/// # Safety
/// The erased value's borrows must not be used past their real lifetimes: the
/// stored form may only be read through a surface that re-binds a lifetime no
/// longer than the borrows' own, or after deep glue replaced every borrow with
/// owned content.
pub unsafe fn erase_static<T: dyn_any::StaticTypeSized>(value: T) -> T::Static {
	let value = std::mem::ManuallyDrop::new(value);
	// SAFETY: `Static` is `Self` with lifetimes substituted, layout-identical
	// by `StaticTypeSized`'s contract.
	unsafe { std::ptr::read((&raw const value).cast::<T::Static>()) }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn elements_write_and_read_in_their_picked_form() {
		let arena = crate::arena::Arena::new(256).unwrap();

		let mut inline = [0u64; 2];
		unsafe { write_element(inline.as_mut_ptr().cast(), 4.5f64, &arena) }.unwrap();
		let rec = unsafe { Rec::new(inline.as_ptr().cast()) };
		assert_eq!(unsafe { *borrow_element::<f64>(rec) }, 4.5);
		assert_eq!(unsafe { read_element::<f64>(rec) }, 4.5);

		let mut parked = [0u64; 2];
		unsafe { write_element(parked.as_mut_ptr().cast(), String::from("moved once"), &arena) }.unwrap();
		let rec = unsafe { Rec::new(parked.as_ptr().cast()) };
		assert_eq!(unsafe { borrow_element::<String>(rec) }.as_str(), "moved once");
		assert_eq!(unsafe { read_element::<String>(rec) }, "moved once");
	}

	#[test]
	fn record_values_are_one_word() {
		assert_eq!(size_of::<RecordValue>(), 8);
		assert_eq!(align_of::<RecordValue>(), 8);
	}
}
