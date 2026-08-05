//! The packed-record tier at rank 0. A record is the element at offset 0
//! plus one field per written attribute; its [`Layout`] is computed at
//! wiring from the upstream write set and never serialized. Records live as
//! per-lane views on the per-thread record [`stack`], claimed per
//! evaluation, and kernels route them as opaque [`RecordValue`]s that carry
//! their provenance. Only generated or wiring code touches offsets, so a
//! safe kernel cannot misalign a field.

use crate::attribute;
use crate::gpoll::GPoll;
use crate::node::Node;

/// A field write declared at wiring, carrying the marker's erased-read glue
/// so introspection and persistence never consult the census at runtime.
#[derive(Clone, Copy, Debug)]
pub struct FieldWrite {
	pub name: &'static str,
	pub level: u8,
	pub size: usize,
	pub align: usize,
	pub read_erased: unsafe fn(*const u8) -> Box<dyn crate::list::AnyAttributeValue>,
}

impl FieldWrite {
	pub fn of<A: crate::attribute::Attribute>(level: u8) -> Self {
		Self {
			name: A::NAME,
			level,
			size: size_of::<A::Value<'static>>(),
			align: align_of::<A::Value<'static>>(),
			read_erased: A::read_erased,
		}
	}
}

/// One field of a [`Layout`]: a (name, level) key resolved to an offset.
/// Levels are numbered innermost-out; only level 0 exists at rank 0.
/// Equality is structural: the glue pointer is excluded, since fn-pointer
/// identity is not guaranteed across codegen units and layout equality
/// drives identity forwarding.
#[derive(Clone, Debug)]
pub struct FieldDesc {
	pub name: &'static str,
	pub level: u8,
	pub offset: usize,
	pub size: usize,
	pub align: usize,
	pub read_erased: unsafe fn(*const u8) -> Box<dyn crate::list::AnyAttributeValue>,
}

impl PartialEq for FieldDesc {
	fn eq(&self, other: &Self) -> bool {
		(self.name, self.level, self.offset, self.size, self.align) == (other.name, other.level, other.offset, other.size, other.align)
	}
}

impl Eq for FieldDesc {}

/// A record layout: the element at offset 0, then the written attributes in
/// canonical order (descending alignment, then size, then name, then level).
/// Layouts are derived data, a pure function of the upstream write set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
	pub depth: u8,
	pub element_size: usize,
	pub element_align: usize,
	pub fields: Vec<FieldDesc>,
	pub size: usize,
	pub align: usize,
}

impl Layout {
	pub fn offset_of(&self, name: &str, level: u8) -> Option<usize> {
		self.fields.iter().find(|field| field.name == name && field.level == level).map(|field| field.offset)
	}

	/// The union of this layout's fields and `writes` over an element of
	/// (size, align) at `depth`, in canonical order. A (name, level) written
	/// at a different size is a type conflict and panics; the census keeps
	/// declared names to one type, so this only fires on wiring bugs.
	pub fn with_writes(&self, depth: u8, element: (usize, usize), writes: &[FieldWrite]) -> Layout {
		let mut merged: Vec<FieldWrite> = self
			.fields
			.iter()
			.map(|field| FieldWrite {
				name: field.name,
				level: field.level,
				size: field.size,
				align: field.align,
				read_erased: field.read_erased,
			})
			.collect();
		for &write in writes {
			match merged.iter().find(|field| field.name == write.name && field.level == write.level) {
				Some(existing) => assert_eq!(existing.size, write.size, "attribute `{}` written at two different sizes", write.name),
				None => merged.push(write),
			}
		}
		merged.sort_by(|a, b| b.align.cmp(&a.align).then(b.size.cmp(&a.size)).then(a.name.cmp(b.name)).then(a.level.cmp(&b.level)));
		let (element_size, element_align) = element;
		let mut offset = element_size;
		let mut align = element_align.max(1);
		let fields = merged
			.into_iter()
			.map(|write| {
				offset = offset.next_multiple_of(write.align.max(1));
				align = align.max(write.align);
				let desc = FieldDesc {
					name: write.name,
					level: write.level,
					offset,
					size: write.size,
					align: write.align,
					read_erased: write.read_erased,
				};
				offset += write.size;
				desc
			})
			.collect();
		Layout {
			depth,
			element_size,
			element_align,
			fields,
			size: offset,
			align,
		}
	}

	/// The union of several layouts over the same element and depth.
	pub fn union(layouts: &[&Layout]) -> Layout {
		let first = layouts.first().expect("a union needs at least one layout");
		let mut union = Layout::default().with_writes(first.depth, (first.element_size, first.element_align), &[]);
		for layout in layouts {
			assert_eq!(union.element_size, layout.element_size, "union layouts must share the element size");
			assert_eq!(union.element_align, layout.element_align, "union layouts must share the element alignment");
			assert_eq!(union.depth, layout.depth, "union layouts must share the depth");
			let writes: Vec<FieldWrite> = layout
				.fields
				.iter()
				.map(|field| FieldWrite {
					name: field.name,
					level: field.level,
					size: field.size,
					align: field.align,
					read_erased: field.read_erased,
				})
				.collect();
			union = union.with_writes(union.depth, (union.element_size, union.element_align), &writes);
		}
		union
	}
}

/// The stand-in fed to a kernel's unbounded `element: T` parameter. The type
/// system forces the kernel to route it to the element position of its return
/// tuple, so the passthrough is explicit in the signature while the lowering
/// carries the element bytes untyped through the copy plan.
#[derive(Clone, Copy, Debug, Default)]
pub struct ElToken;

/// A view of one record: a pointer whose layout is proven at wiring.
#[derive(Clone, Copy, Debug)]
pub struct Rec(*const u8);

impl Rec {
	/// # Safety
	/// `ptr` must point to a live record of the layout the consumer resolved
	/// at wiring, valid until the owning slot is next written.
	pub unsafe fn new(ptr: *const u8) -> Self {
		Rec(ptr)
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

/// An opaque record value: element and attributes traveling as one unit.
/// Lazy record inputs yield one per evaluation, kernels route them as
/// ordinary values, and the returned value's record is the node's output, so
/// provenance rides the value itself. The eval lifetime keeps it out of node
/// state; the field is private, so it is unforgeable and uninspectable.
#[derive(Clone, Copy, Debug)]
pub struct RecordValue<'e>(Rec, std::marker::PhantomData<&'e ()>);

impl<'e> RecordValue<'e> {
	#[doc(hidden)]
	pub fn from_rec(rec: Rec) -> Self {
		RecordValue(rec, std::marker::PhantomData)
	}

	#[doc(hidden)]
	pub fn rec(self) -> Rec {
		self.0
	}
}

/// The per-thread record stack: every record evaluation claims its activation
/// frame at the stack pointer and evaluates its carrier beyond it, so slot
/// addresses are a property of the evaluating thread and no global assignment
/// exists. Thread-local by construction, so access is single-threaded without
/// claims or gates. Records are overwritten per lane and never touch the
/// arena.
pub mod stack {
	use std::cell::Cell;

	struct Stack {
		base: Cell<*mut u8>,
		capacity: Cell<usize>,
		sp: Cell<usize>,
	}

	impl Stack {
		fn free(&self) {
			let base = self.base.get();
			if !base.is_null() {
				drop(unsafe { Vec::from_raw_parts(base.cast::<u64>(), 0, self.capacity.get() / 8) });
			}
		}
	}

	impl Drop for Stack {
		fn drop(&mut self) {
			self.free();
		}
	}

	thread_local! {
		static STACK: Stack = const {
			Stack {
				base: Cell::new(std::ptr::null_mut()),
				capacity: Cell::new(0),
				sp: Cell::new(0),
			}
		};
	}

	/// Ensures the calling thread's stack holds `bytes`, the root's wiring-
	/// derived stack need, and resets the stack pointer. Called only between
	/// evaluations, like the arena reset: nothing survives it, so frames
	/// leaked by an interrupted evaluation are reclaimed here.
	pub fn reserve(bytes: usize) {
		STACK.with(|stack| {
			stack.sp.set(0);
			if stack.capacity.get() >= bytes {
				return;
			}
			let words = bytes.div_ceil(8).max(1);
			let mut memory = vec![0u64; words];
			let base = memory.as_mut_ptr().cast::<u8>();
			std::mem::forget(memory);
			stack.free();
			stack.base.set(base);
			stack.capacity.set(words * 8);
		});
	}

	/// Claims `bytes` (rounded to word alignment) at the stack pointer and
	/// advances past them. The region stays claimed until [`pop`], and stays
	/// readable until the next `push`. `reserve` derived from the root's
	/// stack need makes the capacity bound exact, so overflow is a debug
	/// assertion, not a hot-path branch.
	pub fn push(bytes: usize) -> *mut u8 {
		STACK.with(|stack| {
			let sp = stack.sp.get();
			let next = sp + bytes.next_multiple_of(8);
			debug_assert!(next <= stack.capacity.get(), "record stack overflow: reserve() must cover the root's stack need");
			stack.sp.set(next);
			unsafe { stack.base.get().add(sp) }
		})
	}

	/// Returns the stack pointer to `frame`, a pointer earlier returned by
	/// [`push`] on this thread, releasing everything above it.
	pub fn pop(frame: *mut u8) {
		STACK.with(|stack| {
			let offset = frame as usize - stack.base.get() as usize;
			debug_assert!(offset <= stack.sp.get(), "pop target must lie within the claimed stack");
			stack.sp.set(offset);
		});
	}
}

/// Field-by-field carry from `from`'s layout into `to`'s, computed at
/// wiring. The element copy is included when `carry_element` holds, which is
/// exactly when the node does not write a concrete element itself.
pub fn copy_plan(from: &Layout, to: &Layout, carry_element: bool) -> Vec<(usize, usize, usize)> {
	let mut plan = Vec::new();
	if carry_element {
		assert_eq!(from.element_size, to.element_size, "a carried element must keep its size");
		if from.element_size > 0 {
			plan.push((0, 0, from.element_size));
		}
	}
	for field in &from.fields {
		let target = to.offset_of(field.name, field.level).expect("carried field missing from the output layout");
		plan.push((field.offset, target, field.size));
	}
	plan
}

/// # Safety
/// `offset` must be a field offset of the layout of the record under
/// construction at `dst` and `T` the field's type; both are proven at wiring.
pub unsafe fn write_field<T>(dst: *mut u8, offset: usize, value: T) {
	unsafe { dst.add(offset).cast::<T>().write(value) }
}

/// # Safety
/// `src` must be a record of the plan's source layout and `dst` a buffer of
/// the plan's target layout; both are proven at wiring.
pub unsafe fn apply_plan(src: Rec, dst: *mut u8, plan: &[(usize, usize, usize)]) {
	for &(from, to, size) in plan {
		unsafe { std::ptr::copy_nonoverlapping(src.ptr().add(from), dst.add(to), size) };
	}
}

/// The default bytes for a union field the source does not carry: the census
/// default for declared names, zeroes otherwise.
fn default_fill_bytes(name: &str, size: usize) -> Box<[u8]> {
	let mut bytes = vec![0u8; size].into_boxed_slice();
	if let Some(info) = attribute::info(name)
		&& info.size == size
	{
		(info.write_default_bytes)(&mut bytes);
	}
	bytes
}

/// A routing source's wiring-resolved translation: field moves into the
/// union layout plus census default fill for union fields the source lacks.
/// Absent when the source's layout already equals the union, in which case
/// the record pointer forwards untouched.
#[derive(Debug)]
pub struct SourcePlan {
	moves: Vec<(usize, usize, usize)>,
	fills: Vec<(usize, Box<[u8]>)>,
	union_bytes: usize,
}

impl SourcePlan {
	pub fn new(source: &Layout, union: &Layout) -> Option<SourcePlan> {
		if source == union {
			return None;
		}
		let moves = copy_plan(source, union, true);
		let fills = union
			.fields
			.iter()
			.filter(|field| source.offset_of(field.name, field.level).is_none())
			.map(|field| (field.offset, default_fill_bytes(field.name, field.size)))
			.collect();
		Some(SourcePlan {
			moves,
			fills,
			union_bytes: union.size,
		})
	}

	/// # Safety
	/// `src` must be a record of this plan's source layout and `dst` a
	/// buffer of the plan's union layout.
	pub unsafe fn translate(&self, src: Rec, dst: *mut u8) -> Rec {
		unsafe {
			apply_plan(src, dst, &self.moves);
			for (offset, bytes) in &self.fills {
				std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.add(*offset), bytes.len());
			}
			Rec::new(dst)
		}
	}
}

/// A routing input's claimed edge plus its wiring-resolved [`SourcePlan`].
/// Evaluating it yields the source's record translated to the union layout
/// (or forwarded untouched when the layouts already agree), so the kernel
/// holds and returns record values without ever seeing the representation.
/// A translation claims its landing region from the record stack without
/// popping, so the value survives sibling evaluations; the region is
/// released with the enclosing frame, which bounds claims at one per source
/// evaluation the kernel performs.
pub struct RecordSource<N> {
	edge: N,
	plan: Option<SourcePlan>,
}

impl<N> RecordSource<N> {
	pub fn new(edge: N, source: &Layout, union: &Layout) -> Self {
		Self {
			edge,
			plan: SourcePlan::new(source, union),
		}
	}
}

/// A captured record: the layout plus a generation-checked handle to the
/// arena copy, materialized by the introspection holder, which owns the
/// arena. A dead generation materializes to `None`, never to a stale read.
#[derive(Clone)]
pub struct RecordCapture {
	layout: Layout,
	bytes: crate::arena::ArenaWeak<Box<[u8]>>,
}

impl RecordCapture {
	pub fn materialize(&self, arena: &crate::arena::Arena) -> Option<Vec<(&'static str, Box<dyn crate::list::AnyAttributeValue>)>> {
		let bytes = self.bytes.upgrade(arena)?;
		Some(
			self.layout
				.fields
				.iter()
				.map(|field| (field.name, unsafe { (field.read_erased)(bytes.as_ptr().add(field.offset)) }))
				.collect(),
		)
	}
}

// TODO: Convert to a `#[node_macro::node]` node once routing nodes forward
// layouts and the macro grows a capture capability.
/// The monitor over a record wire: forwards the record and captures an arena
/// copy readable through the introspection window, like a frame memo.
pub struct RecordMonitor<N> {
	edge: N,
	layout: Layout,
	capture: std::sync::Mutex<Option<RecordCapture>>,
}

impl<N> RecordMonitor<N> {
	pub fn new(edge: N, layout: &Layout) -> Self {
		Self {
			edge,
			layout: layout.clone(),
			capture: std::sync::Mutex::new(None),
		}
	}
}

impl<'e, C, N> Node<C> for RecordMonitor<N>
where
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	N: Node<C, Output = RecordValue<'e>>,
{
	type Output = RecordValue<'e>;

	fn eval(&self, input: &C) -> GPoll<RecordValue<'e>> {
		let value = self.edge.eval(input);
		if let GPoll::Final(record) | GPoll::Partial(record) = &value {
			let bytes: Box<[u8]> = unsafe { std::slice::from_raw_parts(record.rec().ptr(), self.layout.size) }.into();
			let capture = input.arena().alloc(bytes).map(|(_, weak)| RecordCapture {
				layout: self.layout.clone(),
				bytes: weak,
			});
			*self.capture.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = capture;
		}
		value
	}

	fn layout(&self) -> Option<&Layout> {
		Some(&self.layout)
	}

	fn serialize(&self) -> Option<std::sync::Arc<dyn std::any::Any + Send + Sync>> {
		let capture = self.capture.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()?;
		Some(std::sync::Arc::new(capture))
	}
}

/// Lifts a plain producer onto a record wire: the element lands at offset 0
/// of a fresh element-only record. `Copy` elements only until droppable
/// elements ride records.
pub struct RecordLift<El, N> {
	edge: N,
	layout: Layout,
	frame_bytes: usize,
	_marker: std::marker::PhantomData<fn() -> El>,
}

impl<El: Copy + 'static, N> RecordLift<El, N> {
	pub fn new(edge: N) -> Self {
		let layout = Layout::default().with_writes(0, (size_of::<El>(), align_of::<El>()), &[]);
		let frame_bytes = layout.size.next_multiple_of(8);
		Self {
			edge,
			layout,
			frame_bytes,
			_marker: std::marker::PhantomData,
		}
	}
}

impl<'e, C, El, N> Node<C> for RecordLift<El, N>
where
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	El: Copy + 'static,
	N: Node<C, Output = El>,
{
	type Output = RecordValue<'e>;

	fn eval(&self, input: &C) -> GPoll<RecordValue<'e>> {
		let dst = stack::push(self.frame_bytes);
		let value = self.edge.eval(input).map(|element| {
			unsafe { write_field(dst, 0, element) };
			RecordValue::from_rec(unsafe { Rec::new(dst.cast_const()) })
		});
		stack::pop(dst);
		value
	}

	fn layout(&self) -> Option<&Layout> {
		Some(&self.layout)
	}
}

/// Extracts the element from a record wire for a plain consumer.
pub struct RecordExtract<El, N> {
	edge: N,
	_marker: std::marker::PhantomData<fn() -> El>,
}

impl<El, N> RecordExtract<El, N> {
	pub fn new(edge: N) -> Self {
		Self {
			edge,
			_marker: std::marker::PhantomData,
		}
	}
}

impl<'e, C, El, N> Node<C> for RecordExtract<El, N>
where
	El: Copy + 'static,
	N: Node<C, Output = RecordValue<'e>>,
{
	type Output = El;

	fn eval(&self, input: &C) -> GPoll<El> {
		self.edge.eval(input).map(|value| unsafe { value.rec().element::<El>() })
	}
}

impl<'e, C, N> Node<C> for RecordSource<N>
where
	N: Node<C, Output = RecordValue<'e>>,
{
	type Output = RecordValue<'e>;

	fn eval(&self, input: &C) -> GPoll<RecordValue<'e>> {
		match &self.plan {
			None => self.edge.eval(input),
			Some(plan) => {
				let dst = stack::push(plan.union_bytes);
				let value = self.edge.eval(input);
				value.map(|value| RecordValue::from_rec(unsafe { plan.translate(value.rec(), dst) }))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	unsafe fn unread(_: *const u8) -> Box<dyn crate::list::AnyAttributeValue> {
		unreachable!("layout-only test field")
	}

	fn sized_field(name: &'static str, size: usize, align: usize) -> FieldWrite {
		FieldWrite { name, level: 0, size, align, read_erased: unread }
	}

	fn f64_field(name: &'static str) -> FieldWrite {
		sized_field(name, 8, 8)
	}

	#[test]
	fn canonical_order_and_offsets() {
		let layout = Layout::default().with_writes(0, (8, 8), &[sized_field("tint", 4, 4), f64_field("opacity"), sized_field("flag", 1, 1)]);
		assert_eq!(layout.offset_of("opacity", 0), Some(8));
		assert_eq!(layout.offset_of("tint", 0), Some(16));
		assert_eq!(layout.offset_of("flag", 0), Some(20));
		assert_eq!(layout.size, 21);
		assert_eq!(layout.align, 8);
	}

	#[test]
	#[should_panic(expected = "two different sizes")]
	fn size_conflicts_panic() {
		let layout = Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")]);
		layout.with_writes(0, (8, 8), &[sized_field("opacity", 4, 4)]);
	}

	#[test]
	fn union_is_order_independent() {
		let a = Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")]);
		let b = Layout::default().with_writes(0, (8, 8), &[f64_field("length")]);
		assert_eq!(Layout::union(&[&a, &b]), Layout::union(&[&b, &a]));
		assert!(Layout::union(&[&a, &b]).offset_of("length", 0).is_some());
	}

	#[test]
	fn translation_moves_fields_and_fills_census_defaults() {
		let source = Layout::default().with_writes(0, (8, 8), &[f64_field("length")]);
		let union = Layout::union(&[&source, &Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")])]);

		let plan = SourcePlan::new(&source, &union).unwrap();
		let record = [5f64, 7f64];
		let mut buffer = vec![0u64; union.size.div_ceil(8)];
		let translated = unsafe { plan.translate(Rec::new(record.as_ptr().cast()), buffer.as_mut_ptr().cast()) };
		assert_eq!(unsafe { translated.element::<f64>() }, 5.);
		assert_eq!(unsafe { translated.read::<f64>(union.offset_of("length", 0).unwrap()) }, 7.);
		assert_eq!(unsafe { translated.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 1.);
	}

	#[test]
	fn identity_layouts_forward() {
		let layout = Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")]);
		assert!(SourcePlan::new(&layout, &layout.clone()).is_none());
	}

	#[test]
	fn stack_frames_nest_and_release() {
		stack::reserve(64);
		let outer = stack::push(24);
		let inner = stack::push(8);
		assert_eq!(inner as usize - outer as usize, 24);
		stack::pop(outer);
		assert_eq!(stack::push(8), outer);
		stack::pop(outer);
	}

	#[test]
	fn stack_rounds_frames_to_word_alignment() {
		stack::reserve(64);
		let first = stack::push(21);
		let second = stack::push(8);
		assert_eq!(second as usize - first as usize, 24);
		stack::pop(first);
	}

	#[test]
	fn each_thread_gets_its_own_stack() {
		stack::reserve(64);
		let here = stack::push(8);
		let here_address = here as usize;
		std::thread::scope(|scope| {
			scope
				.spawn(move || {
					stack::reserve(64);
					let there = stack::push(8);
					assert_ne!(here_address, there as usize, "stacks are per thread");
					stack::pop(there);
				})
				.join()
				.unwrap();
		});
		stack::pop(here);
	}
}
