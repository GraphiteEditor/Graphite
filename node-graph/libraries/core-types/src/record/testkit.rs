//! Law-test scaffolding over the record tier.

use super::frames::{FrameArena, Frames};
use super::layout::{Layout, element_write};
use super::owned::OwnedRecord;
use super::serve::{FrameClaim, Served, serve_input};
use crate::gpoll::GPoll;
use crate::node::Node;

/// Law-test scaffolding: a frame space of `bytes`, leaked so a fixture holds
/// it for the whole test without threading the buffer's own borrow. Production
/// roots own their buffer and lend it by `&mut`.
#[doc(hidden)]
pub fn test_frames(bytes: usize) -> Frames<'static> {
	let arena: &'static mut FrameArena = Box::leak(Box::new(FrameArena::new()));
	arena.reserve(bytes);
	arena.frames()
}

/// One record captured out of a poll: the deep copy plus the layout it was
/// served at, so assertions read owned storage with no tie to the record
/// stack. [`capture`] is the only constructor.
pub struct ServedRecord {
	layout: Layout,
	record: OwnedRecord,
}

impl ServedRecord {
	pub fn layout(&self) -> &Layout {
		&self.layout
	}

	/// The element, cloned out of the capture. Panics unless `T` is the
	/// layout's element type.
	pub fn element<T: Clone + 'static>(&self) -> T {
		assert_eq!(std::any::TypeId::of::<T>(), self.layout.element.type_id, "the read type must match the layout's element type");
		match &self.record.element {
			Some(parked) => parked.downcast_ref::<T>().expect("the element parked at its own type").clone(),
			// SAFETY: the captured bytes image a record of this layout; a
			// byte-carried element is a `T` with no drop glue.
			None => unsafe { self.record.bytes.as_ptr().cast::<T>().read_unaligned() },
		}
	}

	/// The marker's level-0 field value. Panics unless the layout declares
	/// the marker at its value type.
	pub fn attr<A: crate::attribute::Attribute>(&self) -> A::Value<'static>
	where
		A::Value<'static>: Copy + 'static,
	{
		self.field(A::NAME, 0)
	}

	/// A field by name and level, for layouts whose fields are not census
	/// markers. Panics unless the field is declared at `T` and byte-carried;
	/// a parked field reads through replay, not the captured bytes.
	pub fn field<T: Copy + 'static>(&self, name: &str, level: u8) -> T {
		let field = self
			.layout
			.fields
			.iter()
			.find(|field| field.name == name && field.level == level)
			.expect("the layout carries the read field");
		assert_eq!(field.type_id, std::any::TypeId::of::<T>(), "the field was declared at this value type");
		assert!(field.repark.is_none(), "a parked field reads through replay, not the captured bytes");
		// SAFETY: the captured bytes image a record of this layout and the
		// field is byte-carried at `T`.
		unsafe { self.record.bytes.as_ptr().add(field.offset).cast::<T>().read_unaligned() }
	}
}

/// Polls `node` once and captures any served record: the record is
/// deep-copied at the node's own declared layout inside a frame scope, so the
/// result is owned and every claimed frame is free again. Assertion
/// scaffolding for law tests; production consumers read served records in
/// place.
pub fn capture<'e, C, N>(node: &N, ctx: &C, frames: &Frames<'e>) -> GPoll<ServedRecord>
where
	N: Node<C> + ?Sized,
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
{
	let scope = frames.scope();
	let layout = node.layout().clone();
	serve_input(node, ctx, &scope).map(|value| ServedRecord {
		// SAFETY: the poll served `value` at the node's declared layout and
		// nothing has claimed frames since.
		record: unsafe { OwnedRecord::copy_out(&layout, layout.rec(&value)) },
		layout: layout.clone(),
	})
}

/// Law-test scaffolding: a kernel closure served onto an element-only record
/// input (the element lands at offset 0, parked when it carries drop glue). No
/// production path constructs one; value sources are
/// [`crate::value::ValueSource`].
pub struct LiftedSource<El, F> {
	kernel: F,
	layout: Layout,
	_marker: std::marker::PhantomData<fn() -> El>,
}

impl<El: Clone + Send + Sync + dyn_any::StaticTypeSized, F> LiftedSource<El, F>
where
	El::Static: Clone + Send + Sync,
{
	pub fn new(kernel: F) -> Self {
		Self {
			kernel,
			layout: Layout::default().with_writes(0, element_write::<El>(), &[]),
			_marker: std::marker::PhantomData,
		}
	}
}

impl<C, El, F> Node<C> for LiftedSource<El, F>
where
	El: Send + Sync + dyn_any::StaticTypeSized,
	F: Fn(&C) -> GPoll<El>,
{
	fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
	where
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		slot.lift_served((self.kernel)(input), input.arena())
	}

	fn layout(&self) -> &Layout {
		&self.layout
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::record::layout::FieldWrite;

	#[test]
	fn a_captured_frame_serves_its_writes() {
		use crate::attribute::{Attribute, Transform};
		use glam::{DAffine2, DVec2};

		struct Fixture {
			layout: Layout,
		}

		impl<C> Node<C> for Fixture {
			fn serve<'e, 'l>(&self, input: &C, mut slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
			where
				C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
			{
				let offset = self.layout.offset_of(Transform::NAME, 0).expect("the fixture's layout carries the transform");
				if slot.element(String::from("parked"), crate::context::ExtractArena::arena(input)).is_none() {
					return GPoll::error("arena exhausted");
				}
				// SAFETY: the offset is this layout's own, at the marker's value type.
				unsafe { slot.attr_at(offset, DAffine2::from_translation(DVec2::new(3., 4.))) };
				// SAFETY: the writes above complete the record.
				GPoll::Final(unsafe { slot.finish_served() })
			}

			fn layout(&self) -> &Layout {
				&self.layout
			}
		}

		let layout = Layout::default().with_writes(0, element_write::<String>(), &[FieldWrite::of::<Transform>(0)]);
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(1 << 10);
		let frames = frame_arena.frames();
		let arena = crate::arena::Arena::new(1024).unwrap();
		let generations = [];
		let scope = crate::context::EvalScope::new(None, None, None, &generations, &arena);
		let ctx = crate::context::ContextImpl::root(&scope);
		let free = frames.free_words();
		let GPoll::Final(served) = capture(&Fixture { layout }, &ctx, &frames) else {
			panic!("the fixture serves finally");
		};
		assert_eq!(frames.free_words(), free, "capture returns every claimed slot");
		assert_eq!(served.element::<String>(), "parked");
		assert_eq!(served.attr::<Transform>(), DAffine2::from_translation(DVec2::new(3., 4.)));
	}
}
