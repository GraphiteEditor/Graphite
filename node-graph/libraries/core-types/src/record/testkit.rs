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
	C: crate::dispatch::AsDispatch<'e> + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
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
		C: crate::dispatch::AsDispatch<'e> + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
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
				C: crate::dispatch::AsDispatch<'e> + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
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

/// The node-test fixtures: hand-wired record sources and layout installers,
/// so a node crate's tests drive a generated node without the compiler pass.
pub mod fixtures {
	use super::super::frames::Frames;
	use super::super::layout::{FieldWrite, Layout, LayoutMeta, RecordLayout, element_write};
	use super::super::serve::{FrameClaim, Served};
	use super::test_frames;
	use crate::SourceId;
	use crate::arena::Arena;
	use crate::attribute::{Attribute, Opacity, Transform};
	use crate::context::{ContextImpl, EvalScope, ExtractArena, ExtractIndex, ExtractIndices};
	use crate::gpoll::{Extent, GPoll};
	use crate::node::Node;
	use crate::value::ValueSource;
	use glam::DAffine2;

	/// A one-record source with fixed `f64` fields, optionally served partial.
	pub struct RecordSourceNode<E> {
		pub layout: Layout,
		pub element: E,
		pub fields: Vec<(&'static str, f64)>,
		pub partial: bool,
	}

	impl<C, E: Copy + Send + Sync + dyn_any::StaticTypeSized + 'static> Node<C> for RecordSourceNode<E> {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(self.element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			for (name, field) in &self.fields {
				write_field_at(&mut frame, &self.layout, name, 0, *field);
			}
			// SAFETY: the writes above complete the record of this layout.
			let served = unsafe { frame.finish_served() };
			match self.partial {
				true => GPoll::Partial(served),
				false => GPoll::Final(served),
			}
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// A level of `f64` lanes, each optionally carrying one fixed field.
	pub struct LeveledSourceNode {
		pub layout: Layout,
		pub elements: Vec<f64>,
		pub field: Option<(&'static str, f64)>,
	}

	impl<C: ExtractIndex> Node<C> for LeveledSourceNode {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let element = self.elements[input.innermost_index() as usize % self.elements.len()];
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			if let Some((name, value)) = self.field {
				write_field_at(&mut frame, &self.layout, name, 0, value);
			}
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			GPoll::Final(Extent::Exactly(self.elements.len()))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// A level of `f64` lanes each carrying a transform.
	pub struct LeveledTransformSource {
		pub layout: Layout,
		pub rows: Vec<(f64, DAffine2)>,
	}

	impl<C: ExtractIndex> Node<C> for LeveledTransformSource {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let (element, transform) = self.rows[input.innermost_index() as usize % self.rows.len()];
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<Transform>(&mut frame, &self.layout, transform);
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			GPoll::Final(Extent::Exactly(self.rows.len()))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// Serves lanes carrying both a Transform no gather kernel declares and an
	/// Opacity one does, so a carried column can be told apart from a written one.
	pub struct LeveledCarriedSource {
		pub layout: Layout,
		pub rows: Vec<(f64, DAffine2, f64)>,
	}

	impl<C: ExtractIndex> Node<C> for LeveledCarriedSource {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let (element, transform, opacity) = self.rows[input.innermost_index() as usize % self.rows.len()];
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			write_attr_at::<Transform>(&mut frame, &self.layout, transform);
			write_attr_at::<Opacity>(&mut frame, &self.layout, opacity);
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			GPoll::Final(Extent::Exactly(self.rows.len()))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// A leveled source that keeps its count to itself: the extent is a lower
	/// bound and lanes past the data answer the past-end signal.
	pub struct DrainSourceNode {
		pub layout: Layout,
		pub count: usize,
	}

	impl<C: ExtractIndex> Node<C> for DrainSourceNode {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let lane = input.innermost_index();
			if lane >= self.count as u64 {
				return GPoll::past_end();
			}
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(lane as f64, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn extent_at<'x>(&self, _input: &C, _level: u8, _frames: &Frames<'x>) -> GPoll<Extent>
		where
			C: ExtractArena<ArenaRef = &'x Arena>,
		{
			GPoll::Final(Extent::AtLeast(0))
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// Depth-0 content varying per copy: serves the enclosing (pushed) level's
	/// index, which sits one link above the content's own innermost lane.
	pub struct IndexSourceNode {
		pub layout: Layout,
	}

	impl<C: ExtractIndex + ExtractIndices> Node<C> for IndexSourceNode {
		fn serve<'e, 'l>(&self, input: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
		where
			C: ExtractArena<ArenaRef = &'e Arena>,
		{
			let element = input.try_index().and_then(|mut indices| indices.nth(1)).unwrap_or(0) as f64;
			let mut frame = slot;
			let arena = ExtractArena::arena(input);
			if frame.element(element, arena).is_none() {
				return GPoll::arena_exhausted();
			}
			// SAFETY: the writes above complete the record of this layout.
			GPoll::Final(unsafe { frame.finish_served() })
		}

		fn layout(&self) -> &Layout {
			&self.layout
		}
	}

	/// Writes a field at the layout's resolved offset, the wiring-proven pairing
	/// a generated node performs.
	pub fn write_field_at<T: Copy + 'static>(frame: &mut FrameClaim<'_, '_>, layout: &Layout, name: &str, level: u8, value: T) {
		let field = layout
			.fields
			.iter()
			.find(|field| field.name == name && field.level == level)
			.expect("the layout carries the written field");
		assert_eq!(field.type_id, std::any::TypeId::of::<T>(), "the field was declared at this value type");
		// SAFETY: the offset is this layout's own, at the field's declared type.
		unsafe { frame.attr_at(field.offset, value) };
	}

	/// [`write_field_at`] for a census marker at level 0.
	pub fn write_attr_at<A: Attribute>(frame: &mut FrameClaim<'_, '_>, layout: &Layout, value: A::Value<'static>)
	where
		A::Value<'static>: Copy + 'static,
	{
		write_field_at(frame, layout, A::NAME, 0, value);
	}

	pub fn scope_fixture<'a>(generations: &'a [(SourceId, u64)], arena: &'a Arena) -> EvalScope<'a> {
		EvalScope::new(Some(0.5), None, None, generations, arena)
	}

	fn f64_fields(names: &[&'static str]) -> Vec<FieldWrite> {
		names
			.iter()
			.map(|name| FieldWrite {
				name,
				level: 0,
				size: 8,
				align: 8,
				type_id: std::any::TypeId::of::<f64>(),
				read_erased: <Opacity as Attribute>::read_erased,
				repark: None,
				content_hash: None,
				content_eq: None,
			})
			.collect()
	}

	/// A depth-0 `f64` record layout carrying the named `f64` fields.
	pub fn f64_layout(names: &[&'static str]) -> Layout {
		Layout::default().with_writes(0, element_write::<f64>(), &f64_fields(names))
	}

	/// A one-level `f64` layout carrying the named `f64` fields.
	pub fn leveled_f64_layout(names: &[&'static str]) -> Layout {
		Layout::default().with_writes(1, element_write::<f64>(), &f64_fields(names))
	}

	/// Frame space sized for the layouts, with a floor for small fixtures.
	pub fn frames_for(layouts: &[&Layout]) -> Frames<'static> {
		test_frames(layouts.iter().map(|layout| layout.frame_bytes()).sum::<usize>().max(1 << 12))
	}

	/// Installs the layout the compiler pass would resolve for `node` over the
	/// given input layouts. The fixtures wire constants into every eager input,
	/// which the pass records as lane-invariant.
	pub fn install<N: Node<ContextImpl<'static>>>(mut node: N, meta: LayoutMeta, inputs: &[Option<&Layout>]) -> N {
		let resolved = RecordLayout {
			named_writes: Vec::new(),
			named_reads: Vec::new(),
			named_read_defaults: Vec::new(),
			lane_invariant: u32::MAX,
			footprint_free: u32::MAX,
			input_levels: Vec::new(),
			..meta.resolve(inputs)
		};
		<N as Node<ContextImpl<'static>>>::set_layout(&mut node, resolved);
		node
	}

	/// Installs a flipped node's output layout directly.
	pub fn install_flip<N: Node<ContextImpl<'static>>>(mut node: N, layout: &Layout) -> N {
		let bundle = RecordLayout {
			named_writes: Vec::new(),
			named_reads: Vec::new(),
			named_read_defaults: Vec::new(),
			frame_bytes: layout.frame_bytes(),
			plan: Vec::new(),
			layout: layout.clone(),
			lane_invariant: u32::MAX,
			footprint_free: u32::MAX,
			input_levels: Vec::new(),
		};
		<N as Node<ContextImpl<'static>>>::set_layout(&mut node, bundle);
		node
	}

	/// A constant as a value source with its layout.
	pub fn lifted_value<T: Clone + Send + Sync + dyn_any::StaticTypeSized + 'static>(value: T) -> (ValueSource<T>, Layout)
	where
		T::Static: Clone + Send + Sync,
	{
		let lift = ValueSource::new(value);
		let layout = Node::<ContextImpl>::layout(&lift).clone();
		(lift, layout)
	}

	pub fn bare_source(layout: &Layout, element: f64) -> RecordSourceNode<f64> {
		RecordSourceNode {
			layout: layout.clone(),
			element,
			fields: vec![],
			partial: false,
		}
	}
}
