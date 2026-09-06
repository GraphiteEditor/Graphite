//! The owned crossing: deep copies that outlive the evaluation their content borrowed.

use super::access::Rec;
use super::layout::Layout;
use super::serve::FrameClaim;

/// Deep-copy overrides for element types whose plain clone borrows the
/// evaluation's arena (a `Graphic` holding a group interior). The generic
/// element glue consults this registry, so every layout carrying such an
/// element deep-copies at memo and capture seams regardless of which
/// constructor built the glue. The clone-out must produce a value of the
/// element's own type that owns all of its content; the re-park restores that
/// value's arena-resident form before parking it.
#[derive(Clone, Copy)]
pub(in crate::record) struct DeepElementGlue {
	pub(in crate::record) clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	pub(in crate::record) repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
}

static DEEP_ELEMENT_CLONES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, DeepElementGlue>>> = std::sync::LazyLock::new(Default::default);

/// Registers the deep copy-out and re-park pair for elements of `T`. Called
/// at startup from the crate that owns the type.
pub fn register_deep_element_clone<T: dyn_any::StaticTypeSized>(
	clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
) {
	DEEP_ELEMENT_CLONES.lock().unwrap().insert(std::any::TypeId::of::<T::Static>(), DeepElementGlue { clone_out, repark });
}

pub(in crate::record) fn deep_element_glue(type_id: std::any::TypeId) -> Option<DeepElementGlue> {
	DEEP_ELEMENT_CLONES.lock().unwrap().get(&type_id).copied()
}

/// Deep-copy overrides for field values whose content borrows the
/// evaluation's arena (a graphic list holding native groups), keyed by the
/// field's owned value form. Consulted at the persistence seams only:
/// `read_erased` itself stays shallow, since introspection reads captures in
/// generation. Both halves decline when the value already owns all of its
/// content, so group-free values pay no extra clone: `copy_out` returns
/// `None` for unchanged, `replay` returns `Some(None)` for unchanged and
/// `None` for arena exhaustion.
#[derive(Clone, Copy)]
pub(in crate::record) struct DeepFieldGlue {
	pub(in crate::record) copy_out: fn(&dyn crate::list::AnyAttributeValue) -> Option<Box<dyn crate::list::AnyAttributeValue>>,
	pub(in crate::record) replay: crate::list::FieldReplayFn,
}

static DEEP_FIELD_VALUES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, DeepFieldGlue>>> = std::sync::LazyLock::new(Default::default);

/// Registers the deep copy-out and replay pair for field values of `T`.
/// Called at startup from the crate that owns the type.
pub fn register_deep_field_value<T: 'static>(
	copy_out: fn(&dyn crate::list::AnyAttributeValue) -> Option<Box<dyn crate::list::AnyAttributeValue>>,
	replay: crate::list::FieldReplayFn,
) {
	DEEP_FIELD_VALUES.lock().unwrap().insert(std::any::TypeId::of::<T>(), DeepFieldGlue { copy_out, replay });
}

pub(in crate::record) fn deep_field_glue(type_id: std::any::TypeId) -> Option<DeepFieldGlue> {
	DEEP_FIELD_VALUES.lock().unwrap().get(&type_id).copied()
}

/// The copy-out half over an erased field value: the owned form a value takes
/// when it crosses out of the evaluation whose arena its content borrows. A
/// value with no registered glue already owns everything and passes through.
pub fn deepen_field_value(value: Box<dyn crate::list::AnyAttributeValue>) -> Box<dyn crate::list::AnyAttributeValue> {
	match deep_field_glue(value.as_any().type_id()) {
		Some(glue) => (glue.copy_out)(&*value).unwrap_or(value),
		None => value,
	}
}

/// The replay half over an erased field value: `Some(None)` where the value
/// already owns its content, `None` on arena exhaustion.
pub fn replay_field_value(value: &dyn crate::list::AnyAttributeValue, arena: &crate::arena::Arena) -> Option<Option<Box<dyn crate::list::AnyAttributeValue>>> {
	match deep_field_glue(value.as_any().type_id()) {
		Some(glue) => (glue.replay)(value, arena),
		None => Some(None),
	}
}

/// A record deep-copied out of its evaluation: the packed bytes plus owned
/// clones of every parked payload, replayable into a later evaluation's
/// storage through the layout's erased glue. The layout stays with the
/// holder, which proved it at wiring.
pub struct OwnedRecord {
	pub(in crate::record) bytes: Box<[u8]>,
	pub(in crate::record) element: Option<Box<dyn std::any::Any + Send + Sync>>,
	fields: Vec<(usize, Box<dyn crate::list::AnyAttributeValue>)>,
}

impl std::fmt::Debug for OwnedRecord {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("OwnedRecord(..)")
	}
}

impl OwnedRecord {
	/// # Safety
	/// `rec` must be a live record of `layout`.
	pub unsafe fn copy_out(layout: &Layout, rec: Rec<'_>) -> OwnedRecord {
		let bytes: Box<[u8]> = unsafe { std::slice::from_raw_parts(rec.ptr(), layout.size) }.into();
		let element = layout.element.parked.then(|| unsafe { (layout.element.clone_out)(rec.ptr()) });
		let fields = layout
			.fields
			.iter()
			.enumerate()
			.filter(|(_, field)| field.repark.is_some())
			.map(|(index, field)| (index, deepen_field_value(unsafe { (field.read_erased)(rec.ptr().add(field.offset)) })))
			.collect();
		OwnedRecord { bytes, element, fields }
	}

	/// Replays the copy into a caller's claim, re-parking droppable payloads
	/// against `arena`; the claim's layout is the one the copy was taken at,
	/// and `None` reports arena exhaustion.
	pub fn replay_into(&self, slot: &mut FrameClaim<'_, '_>, arena: &crate::arena::Arena) -> Option<()> {
		let layout = slot.layout;
		self.write_into(layout, slot.dst(), arena)
	}

	fn write_into(&self, layout: &Layout, dst: *mut u8, arena: &crate::arena::Arena) -> Option<()> {
		unsafe { std::ptr::copy_nonoverlapping(self.bytes.as_ptr(), dst, self.bytes.len()) };
		if let Some(element) = &self.element {
			unsafe { (layout.element.repark)(&**element, dst, arena) }?;
		}
		for (index, value) in &self.fields {
			let field = &layout.fields[*index];
			let repark = field.repark.expect("copied fields carry re-park glue");
			let resident = replay_field_value(&**value, arena)?;
			unsafe { repark(resident.as_deref().unwrap_or(&**value), dst.add(field.offset), arena) }?;
		}
		Some(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::record::access::{read_element, write_element, write_field};
	use crate::record::frames::FrameArena;
	use crate::record::layout::{FieldWrite, element_write};

	#[test]
	fn owned_records_replay_re_parked_payloads_after_the_source_dies() {
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[FieldWrite::of::<crate::attribute::Name>(0)]);
		let mut buffer = vec![0u64; layout.size.div_ceil(8)];
		let base: *mut u8 = buffer.as_mut_ptr().cast();

		let copy = {
			let arena = crate::arena::Arena::new(1024).unwrap();
			unsafe { write_element(base, String::from("element"), &arena) }.unwrap();
			let (name, _) = arena.alloc(String::from("field")).unwrap();
			unsafe { write_field::<&str>(base, layout.offset_of("name", 0).unwrap(), name.as_str()) };
			unsafe { OwnedRecord::copy_out(&layout, Rec::new(base)) }
		};
		buffer.fill(u64::MAX);

		let replay_arena = crate::arena::Arena::new(1024).unwrap();
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(layout.frame_bytes());
		let frames = frame_arena.frames();
		let mut slot = frames.claim(&layout);
		copy.replay_into(&mut slot, &replay_arena).unwrap();
		// SAFETY: the replay completes the record in the claimed frame.
		let value = unsafe { slot.finish() };
		let rec = layout.rec(&value);
		assert_eq!(unsafe { read_element::<String>(rec) }, "element");
		assert_eq!(unsafe { rec.read::<&str>(layout.offset_of("name", 0).unwrap()) }, "field");
	}
}
