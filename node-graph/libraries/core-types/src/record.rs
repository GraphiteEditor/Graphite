//! The packed-record tier at rank 0. A record is the element at offset 0
//! plus one field per written attribute; its [`Layout`] is computed at
//! wiring from the upstream write set and never serialized. Records of
//! inline layouts live in the [`RecordValue`] itself; larger ones live as
//! per-lane views on the per-thread record [`stack`], claimed per
//! evaluation. Kernels route them as opaque [`RecordValue`]s that carry
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
	pub repark: Option<unsafe fn(&dyn crate::list::AnyAttributeValue, *mut u8, &crate::arena::Arena) -> Option<()>>,
}

impl FieldWrite {
	pub fn of<A: crate::attribute::Attribute>(level: u8) -> Self {
		Self {
			name: A::NAME,
			level,
			size: size_of::<A::Value<'static>>(),
			align: align_of::<A::Value<'static>>(),
			read_erased: A::read_erased,
			repark: A::REPARK,
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
	pub repark: Option<unsafe fn(&dyn crate::list::AnyAttributeValue, *mut u8, &crate::arena::Arena) -> Option<()>>,
}

impl PartialEq for FieldDesc {
	fn eq(&self, other: &Self) -> bool {
		(self.name, self.level, self.offset, self.size, self.align) == (other.name, other.level, other.offset, other.size, other.align)
	}
}

impl Eq for FieldDesc {}

/// The element slot of a layout: its dimensions plus erased glue bound where
/// the element type is statically known, so generic consumers read or
/// deep-copy the element without it. Equality is structural: glue pointers
/// are excluded for the same reason as [`FieldDesc`]'s.
#[derive(Clone, Copy, Debug)]
pub struct ElementWrite {
	pub size: usize,
	pub align: usize,
	pub parked: bool,
	pub clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	pub repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
}

impl PartialEq for ElementWrite {
	fn eq(&self, other: &Self) -> bool {
		(self.size, self.align, self.parked) == (other.size, other.align, other.parked)
	}
}

impl Eq for ElementWrite {}

impl Default for ElementWrite {
	fn default() -> Self {
		unsafe fn clone_out(_ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
			Box::new(())
		}
		unsafe fn repark(_value: &(dyn std::any::Any + Send + Sync), _dst: *mut u8, _arena: &crate::arena::Arena) -> Option<()> {
			Some(())
		}
		Self {
			size: 0,
			align: 0,
			parked: false,
			clone_out,
			repark,
		}
	}
}

/// A record layout: the element at offset 0, then the written attributes in
/// canonical order (descending alignment, then size, then name, then level).
/// Layouts are derived data, a pure function of the upstream write set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
	pub depth: u8,
	pub element: ElementWrite,
	pub fields: Vec<FieldDesc>,
	pub size: usize,
	pub align: usize,
}

impl Layout {
	pub fn offset_of(&self, name: &str, level: u8) -> Option<usize> {
		self.fields.iter().find(|field| field.name == name && field.level == level).map(|field| field.offset)
	}

	pub fn frame_bytes(&self) -> usize {
		self.size.next_multiple_of(8)
	}

	/// Resolves a value of this layout, which must be its wiring-proven one,
	/// to its record bytes. An empty record carries nothing and resolves to the
	/// value's own storage; every other record spills and rides the pointer.
	pub fn rec(&self, value: &RecordValue<'_>) -> Rec {
		match self.size == 0 {
			true => Rec((&raw const *value).cast()),
			false => Rec(value.ptr),
		}
	}

	/// The union of this layout's fields and `writes` over `element` at
	/// `depth`, in canonical order. A (name, level) written at a different
	/// size is a type conflict and panics; the census keeps declared names to
	/// one type, so this only fires on wiring bugs.
	pub fn with_writes(&self, depth: u8, element: ElementWrite, writes: &[FieldWrite]) -> Layout {
		let mut merged: Vec<FieldWrite> = self
			.fields
			.iter()
			.map(|field| FieldWrite {
				name: field.name,
				level: field.level,
				size: field.size,
				align: field.align,
				read_erased: field.read_erased,
				repark: field.repark,
			})
			.collect();
		for &write in writes {
			match merged.iter().find(|field| field.name == write.name && field.level == write.level) {
				Some(existing) => assert_eq!(existing.size, write.size, "attribute `{}` written at two different sizes", write.name),
				None => merged.push(write),
			}
		}
		merged.sort_by(|a, b| b.align.cmp(&a.align).then(b.size.cmp(&a.size)).then(a.name.cmp(b.name)).then(a.level.cmp(&b.level)));
		let mut offset = element.size;
		let mut align = element.align.max(1);
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
					repark: write.repark,
				};
				offset += write.size;
				desc
			})
			.collect();
		Layout {
			depth,
			element,
			fields,
			size: offset,
			align,
		}
	}

	/// This layout minus the named fields, offsets recomputed. Removing an
	/// absent name is a no-op: downstream reads yield the default either way.
	pub fn without(&self, removes: &[(&str, u8)]) -> Layout {
		let retained: Vec<FieldWrite> = self
			.fields
			.iter()
			.filter(|field| !removes.contains(&(field.name, field.level)))
			.map(|field| FieldWrite {
				name: field.name,
				level: field.level,
				size: field.size,
				align: field.align,
				read_erased: field.read_erased,
				repark: field.repark,
			})
			.collect();
		Layout::default().with_writes(self.depth, self.element, &retained)
	}

	/// The union of several layouts over the same element and depth.
	pub fn union(layouts: &[&Layout]) -> Layout {
		let first = layouts.first().expect("a union needs at least one layout");
		let mut union = Layout::default().with_writes(first.depth, first.element, &[]);
		for layout in layouts {
			assert_eq!(union.element, layout.element, "union layouts must share the element");
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
					repark: field.repark,
				})
				.collect();
			union = union.with_writes(union.depth, union.element, &writes);
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

/// The shared empty layout: `depth` 0, no element, no fields, so `frame_bytes`
/// is 0. The `Node::layout` default returns it for element-only and test nodes,
/// which carry no record.
pub fn empty_layout() -> &'static Layout {
	static EMPTY: std::sync::OnceLock<Layout> = std::sync::OnceLock::new();
	EMPTY.get_or_init(Layout::default)
}

/// Declarative record-io metadata for a node type, emitted by the macro into
/// A record node's output layout with the frame size and carrier copy plan derived from it.
#[derive(Clone, Debug, Default)]
pub struct RecordLayout {
	pub layout: Layout,
	pub frame_bytes: usize,
	pub plan: Vec<(usize, usize, usize)>,
}

/// its registry entry so the compiler can fold each wire's layout without
/// running the node's constructor. [`fold`](LayoutMeta::fold) reproduces the
/// layout the constructor derives at wiring today; the compiler layout pass
/// calls it over the proto graph instead.
#[derive(Clone, Debug)]
pub struct LayoutMeta {
	/// Input indices whose layouts union to form the base: `[]` writes a fresh
	/// record, `[i]` derives from a single carrier, `[i, j, ..]` unions routing
	/// sources.
	pub sources: Vec<u8>,
	/// Attributes read from each input. Unused by [`fold`](LayoutMeta::fold);
	/// recorded for later compiler analysis (read-offset resolution, per-name
	/// cache dependencies, residency).
	pub reads: Vec<InputReads>,
	/// The output element: a concrete write, or carried through from the base.
	pub element: ElementSpec,
	/// The attributes the node writes at its acting level.
	pub writes: Vec<FieldWrite>,
	/// The attributes removed from the base layout, as `(name, level)`.
	pub removes: Vec<(&'static str, u8)>,
	/// The depth change the node applies: `0` for elementwise and flip nodes,
	/// `+1` for a creator, `-1` for a reducer.
	pub level_delta: i8,
}

/// The attributes a node reads from one input, recorded on [`LayoutMeta`] for
/// later compiler analysis. A read and a write of an attribute carry the same
/// [`FieldWrite`] descriptor; the direction is the position on the node.
#[derive(Clone, Debug)]
pub struct InputReads {
	pub input: u8,
	pub reads: Vec<FieldWrite>,
}

/// Where a node's output element comes from, for [`LayoutMeta`].
#[derive(Clone, Debug)]
pub enum ElementSpec {
	/// The node writes this concrete element.
	Concrete(ElementWrite),
	/// The node carries the carrier's element through unchanged.
	Carried,
}

impl LayoutMeta {
	/// Keeps input 0's layout but replaces its element.
	pub fn retype(element: ElementWrite) -> Self {
		Self {
			sources: vec![0],
			reads: Vec::new(),
			element: ElementSpec::Concrete(element),
			writes: Vec::new(),
			removes: Vec::new(),
			level_delta: 0,
		}
	}

	/// Folds the node's output layout from its inputs', reproducing what the
	/// node's constructor derives at wiring. `inputs` is indexed by proto-input
	/// position; [`sources`](LayoutMeta::sources) selects the base layouts, which
	/// union (empty writes a fresh record).
	pub fn fold(&self, inputs: &[Option<&Layout>]) -> Layout {
		let sources: Vec<&Layout> = self.sources.iter().map(|&i| inputs[i as usize].expect("layout fold source input has no layout")).collect();
		let base = match sources.as_slice() {
			[] => Layout::default(),
			sources => Layout::union(sources),
		}
		.without(&self.removes);
		let depth = (base.depth as i8 + self.level_delta).max(0) as u8;
		let element = match &self.element {
			ElementSpec::Concrete(element) => *element,
			ElementSpec::Carried => base.element,
		};
		base.with_writes(depth, element, &self.writes)
	}

	/// [`fold`](LayoutMeta::fold) with the frame size and carrier copy plan derived from it.
	pub fn resolve(&self, inputs: &[Option<&Layout>]) -> RecordLayout {
		let layout = self.fold(inputs);
		let frame_bytes = layout.frame_bytes();
		let plan = match self.sources.first() {
			// A reducer collapses its carrier's levels, so it writes a fresh record rather than copying fields down.
			Some(&source) if self.level_delta >= 0 => {
				let from = inputs[source as usize].expect("layout resolve source input has no layout");
				let carry_element = matches!(self.element, ElementSpec::Carried);
				let removes: Vec<(&str, u8)> = self.removes.clone();
				copy_plan(from, &layout, carry_element, &removes)
			}
			_ => Vec::new(),
		};
		RecordLayout { layout, frame_bytes, plan }
	}
}

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

/// An opaque record value: every non-empty record spills to the record stack
/// and the value carries its pointer, while an empty record carries nothing.
/// Only [`Layout::rec`] reads it, against the wiring-proven layout.
#[derive(Clone, Copy)]
pub struct RecordValue<'e> {
	ptr: *const u8,
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
	pub fn spilled(rec: Rec) -> Self {
		RecordValue {
			ptr: rec.ptr(),
			_lifetime: std::marker::PhantomData,
		}
	}

	/// Rebinds the eval lifetime; validity stays stack and arena discipline,
	/// which derived scopes share with their parent evaluation.
	fn rebind<'a>(self) -> RecordValue<'a> {
		RecordValue {
			ptr: self.ptr,
			_lifetime: std::marker::PhantomData,
		}
	}
}

/// A record edge evaluable at a derived context, yielding the record at that
/// context's lifetime. The lifetime is a trait parameter because a bound like
/// `for<'d> Node<Derived<'d, C>, Output = RecordValue<'d>>` is rejected: in a
/// higher-ranked bound the lifetime must appear in a constrained input
/// position, and both the `Derived` projection and the `Output` binding are
/// unconstrained ones.
pub trait DerivedRecordEdge<'derived, C> {
	fn eval_derived(&self, cell: &crate::node::StatusCell, input_index: usize, ctx: &C) -> Result<RecordValue<'derived>, crate::gpoll::Interrupt>;
}

impl<'derived, C, N> DerivedRecordEdge<'derived, C> for N
where
	N: Node<C, Output = RecordValue<'derived>>,
{
	fn eval_derived(&self, cell: &crate::node::StatusCell, input_index: usize, ctx: &C) -> Result<RecordValue<'derived>, crate::gpoll::Interrupt> {
		cell.eval_input(input_index, self, ctx)
	}
}

/// A record edge at a caller-chosen lifetime; the lifetime is a trait
/// parameter for the same constrained-position reason as
/// [`DerivedRecordEdge`].
pub trait RecordEdge<'e, C>: Node<C, Output = RecordValue<'e>> {}

impl<'e, C, N: Node<C, Output = RecordValue<'e>>> RecordEdge<'e, C> for N {}

/// Builds an element-only record from a kernel's poll: inline layouts land
/// in the value, larger ones spill to the record stack, arena exhaustion of
/// a parked element reports as an error poll.
pub fn lift_poll<'e, T: Send + Sync + 'static>(poll: GPoll<T>, layout: &Layout, arena: &'e crate::arena::Arena) -> GPoll<RecordValue<'e>> {
	let build = |element: T| {
		if layout.frame_bytes() == 0 {
			let mut value = RecordValue::zeroed();
			unsafe { write_element(value.as_mut_ptr(), element, arena)? };
			Some(value)
		} else {
			let dst = stack::push(layout.frame_bytes());
			let written = unsafe { write_element(dst, element, arena) };
			stack::truncate_above(dst, layout.frame_bytes());
			written.map(|()| RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) }))
		}
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

/// The raw lazy record edge handed to a record-opaque kernel: the wire plus
/// its wiring-proven layout, the pairing the kernel's unsafe record
/// operations rely on. The kernel must only pair the layout with values this
/// edge produced.
pub struct RecordEdgeInput<'a, N> {
	node: &'a N,
	layout: &'a Layout,
}

impl<'a, N> RecordEdgeInput<'a, N> {
	pub fn new(node: &'a N, layout: &'a Layout) -> Self {
		Self { node, layout }
	}

	pub fn layout(&self) -> &Layout {
		self.layout
	}

	pub fn eval<'e, C>(&self, ctx: &C) -> GPoll<RecordValue<'e>>
	where
		N: Node<C, Output = RecordValue<'e>>,
	{
		self.node.eval(ctx)
	}
}

/// The raw lazy edge handed to a poll kernel whose wire rides records while
/// the kernel consumes the plain element.
/// # Safety
/// `rec` must be a record of the layout the offsets were resolved against
/// and `El` its element type; both are proven at wiring.
unsafe fn element_only<El: Clone>(rec: Rec, _reads: &[Option<usize>]) -> El {
	unsafe { read_element::<El>(rec) }
}

pub struct ElementEdge<'a, Out, N> {
	node: &'a N,
	layout: &'a Layout,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec, &[Option<usize>]) -> Out,
}

impl<'a, El: Clone, N> ElementEdge<'a, El, N> {
	pub fn new(node: &'a N, layout: &'a Layout) -> Self {
		Self {
			node,
			layout,
			reads: &[],
			read: element_only::<El>,
		}
	}
}

impl<'a, Out, N> ElementEdge<'a, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn with_reads(node: &'a N, layout: &'a Layout, reads: &'a [Option<usize>], read: unsafe fn(Rec, &[Option<usize>]) -> Out) -> Self {
		Self { node, layout, reads, read }
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> GPoll<Out>
	where
		N: Node<C, Output = RecordValue<'d>>,
	{
		let mark = stack::sp();
		self.node.eval(ctx).map(|value| {
			let out = unsafe { (self.read)(self.layout.rec(&value), self.reads) };
			// SAFETY: the read copied out by value, so no record above `mark` (the
			// edge's own frame) is live.
			unsafe { stack::rewind(mark) };
			out
		})
	}
}

/// The lazy input handed to a kernel whose edge rides a record wire while
/// the kernel consumes the plain element, or the element beside its declared
/// attribute reads.
#[derive(Clone, Copy)]
pub struct ElementLazyInput<'a, Out, N> {
	node: &'a N,
	cell: &'a crate::node::StatusCell,
	input_index: usize,
	layout: &'a Layout,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec, &[Option<usize>]) -> Out,
}

impl<'a, El: Clone, N> ElementLazyInput<'a, El, N> {
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, layout: &'a Layout) -> Self {
		Self {
			node,
			cell,
			input_index,
			layout,
			reads: &[],
			read: element_only::<El>,
		}
	}
}

impl<'a, Out, N> ElementLazyInput<'a, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn with_reads(
		node: &'a N,
		cell: &'a crate::node::StatusCell,
		input_index: usize,
		layout: &'a Layout,
		reads: &'a [Option<usize>],
		read: unsafe fn(Rec, &[Option<usize>]) -> Out,
	) -> Self {
		Self {
			node,
			cell,
			input_index,
			layout,
			reads,
			read,
		}
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<Out, crate::gpoll::Interrupt>
	where
		N: Node<C, Output = RecordValue<'d>>,
	{
		let mark = stack::sp();
		let value = self.cell.eval_input(self.input_index, self.node, ctx)?;
		let out = unsafe { (self.read)(self.layout.rec(&value), self.reads) };
		// SAFETY: the read copied the element and declared attributes out by value,
		// so no record above `mark` (the edge's own frame) is live.
		unsafe { stack::rewind(mark) };
		Ok(out)
	}
}

/// The lazy record input handed to a kernel that evaluates its edges under
/// derived contexts: evaluating rebinds the record to the kernel's routing
/// lifetime, so the value escapes the derivation scope.
#[derive(Clone, Copy)]
pub struct RecordLazyInput<'a, 'e, N> {
	node: &'a N,
	cell: &'a crate::node::StatusCell,
	input_index: usize,
	_lifetime: std::marker::PhantomData<fn() -> RecordValue<'e>>,
}

impl<'a, 'e, N> RecordLazyInput<'a, 'e, N> {
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize) -> Self {
		Self {
			node,
			cell,
			input_index,
			_lifetime: std::marker::PhantomData,
		}
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<RecordValue<'e>, crate::gpoll::Interrupt>
	where
		N: DerivedRecordEdge<'d, C>,
	{
		Ok(self.node.eval_derived(self.cell, self.input_index, ctx)?.rebind())
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
	/// [`push`] on this thread, releasing it and everything above it. Resets to
	/// a checkpoint between repeated evaluations.
	pub fn pop(frame: *mut u8) {
		STACK.with(|stack| {
			let offset = frame as usize - stack.base.get() as usize;
			debug_assert!(offset <= stack.sp.get(), "pop target must lie within the claimed stack");
			stack.sp.set(offset);
		});
	}

	/// Releases everything above `frame`'s `bytes`-sized region, keeping the
	/// region itself. A node reclaims its inputs' frames on return but leaves
	/// its own output readable for its consumer.
	pub fn truncate_above(frame: *mut u8, bytes: usize) {
		STACK.with(|stack| {
			let top = frame as usize - stack.base.get() as usize + bytes.next_multiple_of(8);
			debug_assert!(top <= stack.sp.get(), "truncate target must lie within the claimed stack");
			stack.sp.set(top);
		});
	}

	/// The current stack pointer, a checkpoint to [`rewind`] to.
	pub fn sp() -> usize {
		STACK.with(|stack| stack.sp.get())
	}

	/// Resets the stack pointer to an earlier [`sp`] checkpoint, so a loop that
	/// evaluates a subtree per iteration reuses the same slots each time.
	///
	/// # Safety
	/// No `Rec` or `RecordValue` into the region above `mark` may be used after
	/// this call. The caller must have copied out everything it still needs.
	pub unsafe fn rewind(mark: usize) {
		STACK.with(|stack| {
			debug_assert!(mark <= stack.sp.get(), "rewind target above the stack pointer");
			stack.sp.set(mark);
		});
	}
}

/// Reclaims the frames an inline node's inputs push. An inline node returns
/// its output by value rather than on the stack, so it has no frame whose
/// `truncate_above` would release its inputs; this guard captures the entry
/// pointer and rewinds to it on drop instead. Inactive (a no-op) for spilled
/// nodes, which release their inputs through their own frame.
pub struct ReclaimGuard {
	target: usize,
}

impl ReclaimGuard {
	/// # Safety
	/// When `active`, the node must return its output by value, so that no
	/// record into the region above the entry pointer is live once its eval
	/// returns and the guard rewinds.
	pub unsafe fn new(active: bool) -> Self {
		Self {
			target: if active { stack::sp() } else { usize::MAX },
		}
	}
}

impl Drop for ReclaimGuard {
	fn drop(&mut self) {
		if self.target != usize::MAX {
			// SAFETY: an inline node returns its output by value, so no record into
			// the reclaimed region is live once its eval returns.
			unsafe { stack::rewind(self.target) };
		}
	}
}

/// Field-by-field carry from `from`'s layout into `to`'s, computed at
/// wiring. The element copy is included when `carry_element` holds, which is
/// exactly when the node does not write a concrete element itself. `removes`
/// names the fields the node deletes, which are exactly the ones allowed to
/// be absent from `to`.
pub fn copy_plan(from: &Layout, to: &Layout, carry_element: bool, removes: &[(&str, u8)]) -> Vec<(usize, usize, usize)> {
	let mut plan = Vec::new();
	if carry_element {
		assert_eq!(from.element.size, to.element.size, "a carried element must keep its size");
		if from.element.size > 0 {
			plan.push((0, 0, from.element.size));
		}
	}
	for field in &from.fields {
		if removes.contains(&(field.name, field.level)) {
			continue;
		}
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

/// Finishes a carried record frame: the element lands beside the fields
/// already carried into `dst`, inline frames copy out of the scratch bytes,
/// and the frame releases in every branch, so the frame lifecycle closes
/// here. Arena exhaustion of a parked element reports as an error poll.
///
/// # Safety
/// `dst` must be the claimed frame (or inline scratch when `frame_bytes` is
/// 0) of a record whose element is `T` and whose frame size is `frame_bytes`,
/// with every carried field already written.
pub unsafe fn lift_poll_into<'e, T: Send + Sync + 'static>(poll: GPoll<T>, dst: *mut u8, frame_bytes: usize, arena: &'e crate::arena::Arena) -> GPoll<RecordValue<'e>> {
	let release = || {
		if frame_bytes != 0 {
			stack::truncate_above(dst, frame_bytes);
		}
	};
	let build = |element: T| {
		let written = unsafe { write_element(dst, element, arena) };
		release();
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
		GPoll::Pending => {
			release();
			GPoll::Pending
		}
		GPoll::Error(error) => {
			release();
			GPoll::Error(error)
		}
	}
}

/// Whether elements of `T` move once into the arena and ride as references:
/// records byte-copy their contents and never run drop glue, so a type is
/// byte-carried exactly when it has none.
pub const fn element_parked<T>() -> bool {
	std::mem::needs_drop::<T>()
}

/// The element (size, align) a record wire of `T` carries.
pub fn element_dims<T>() -> (usize, usize) {
	match element_parked::<T>() {
		true => (size_of::<*const u8>(), align_of::<*const u8>()),
		false => (size_of::<T>(), align_of::<T>()),
	}
}

/// The element slot a record wire of `T` carries, its erased glue bound at
/// the statically-known type.
pub fn element_write<T: Clone + Send + Sync + 'static>() -> ElementWrite {
	unsafe fn clone_out<T: Clone + Send + Sync + 'static>(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
		Box::new(unsafe { read_element::<T>(Rec::new(ptr)) })
	}
	unsafe fn repark<T: Clone + Send + Sync + 'static>(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &crate::arena::Arena) -> Option<()> {
		let value = value.downcast_ref::<T>().expect("an element replays at its own type");
		unsafe { write_element(dst, value.clone(), arena) }
	}
	let (size, align) = element_dims::<T>();
	ElementWrite {
		size,
		align,
		parked: element_parked::<T>(),
		clone_out: clone_out::<T>,
		repark: repark::<T>,
	}
}

/// # Safety
/// The record's element must be a `T` in the form [`element_parked`] picks,
/// and the borrow is only valid while the record is.
pub unsafe fn borrow_element<'e, T>(rec: Rec) -> &'e T {
	match element_parked::<T>() {
		true => unsafe { rec.element::<&T>() },
		false => unsafe { &*rec.ptr().cast::<T>() },
	}
}

/// # Safety
/// The record's element must be a `T` in the form [`element_parked`] picks.
pub unsafe fn read_element<T: Clone>(rec: Rec) -> T {
	unsafe { borrow_element::<T>(rec) }.clone()
}


/// # Safety
/// `dst` must be fresh element storage of a record whose element is `T`.
/// `None` reports arena exhaustion for a parked element.
pub unsafe fn write_element<T: Send + Sync + 'static>(dst: *mut u8, value: T, arena: &crate::arena::Arena) -> Option<()> {
	match element_parked::<T>() {
		true => {
			let (parked, _) = arena.alloc(value)?;
			unsafe { dst.cast::<&T>().write(parked) };
			Some(())
		}
		false => {
			unsafe { dst.cast::<T>().write(value) };
			Some(())
		}
	}
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
	source: Layout,
	union: Layout,
}

impl SourcePlan {
	pub fn new(source: &Layout, union: &Layout) -> Option<SourcePlan> {
		if source == union {
			return None;
		}
		let moves = copy_plan(source, union, true, &[]);
		let fills = union
			.fields
			.iter()
			.filter(|field| source.offset_of(field.name, field.level).is_none())
			.map(|field| (field.offset, default_fill_bytes(field.name, field.size)))
			.collect();
		Some(SourcePlan {
			moves,
			fills,
			source: source.clone(),
			union: union.clone(),
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
	union: Layout,
}

impl<N> RecordSource<N> {
	pub fn new(edge: N, source: &Layout, union: &Layout) -> Self {
		Self {
			edge,
			plan: SourcePlan::new(source, union),
			union: union.clone(),
		}
	}
}

/// # Safety
/// `rec` must be a live record of `layout`.
pub unsafe fn copy_record_bytes(layout: &Layout, rec: Rec) -> Box<[u8]> {
	unsafe { std::slice::from_raw_parts(rec.ptr(), layout.size) }.into()
}

/// Builds a record value over `bytes`, a record of `layout` copied out
/// earlier: inline layouts copy into the value, spilled ones alias the
/// bytes.
///
/// # Safety
/// `bytes` must hold a record of `layout` whose parked references are still
/// live; both hold for a copy taken in the same evaluation frame.
pub unsafe fn record_from_bytes<'e>(layout: &Layout, bytes: &'e [u8]) -> RecordValue<'e> {
	if layout.frame_bytes() == 0 {
		let mut value = RecordValue::zeroed();
		unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), value.as_mut_ptr(), bytes.len()) };
		value
	} else {
		RecordValue::spilled(unsafe { Rec::new(bytes.as_ptr()) })
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

impl std::fmt::Debug for RecordCapture {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("RecordCapture(..)")
	}
}

impl RecordCapture {
	/// # Safety
	/// `rec` must be a live record of `layout`.
	pub unsafe fn capture(layout: &Layout, rec: Rec, arena: &crate::arena::Arena) -> Option<RecordCapture> {
		let bytes = unsafe { copy_record_bytes(layout, rec) };
		arena.alloc(bytes).map(|(_, weak)| RecordCapture { layout: layout.clone(), bytes: weak })
	}

	/// The captured element, cloned out through the layout's erased glue.
	pub fn materialize_element(&self, arena: &crate::arena::Arena) -> Option<Box<dyn std::any::Any + Send + Sync>> {
		let bytes = self.bytes.upgrade(arena)?;
		Some(unsafe { (self.layout.element.clone_out)(bytes.as_ptr()) })
	}

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

/// A record deep-copied out of its evaluation: the packed bytes plus owned
/// clones of every parked payload, replayable into a later evaluation's
/// storage through the layout's erased glue. The layout stays with the
/// holder, which proved it at wiring.
pub struct OwnedRecord {
	bytes: Box<[u8]>,
	element: Option<Box<dyn std::any::Any + Send + Sync>>,
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
	pub unsafe fn copy_out(layout: &Layout, rec: Rec) -> OwnedRecord {
		let bytes: Box<[u8]> = unsafe { std::slice::from_raw_parts(rec.ptr(), layout.size) }.into();
		let element = layout.element.parked.then(|| unsafe { (layout.element.clone_out)(rec.ptr()) });
		let fields = layout
			.fields
			.iter()
			.enumerate()
			.filter(|(_, field)| field.repark.is_some())
			.map(|(index, field)| (index, unsafe { (field.read_erased)(rec.ptr().add(field.offset)) }))
			.collect();
		OwnedRecord { bytes, element, fields }
	}

	/// Replays the copy into fresh storage of `layout`, the layout it was
	/// copied out under, re-parking droppable payloads against `arena`;
	/// `None` reports arena exhaustion.
	pub fn replay<'e>(&self, layout: &Layout, arena: &'e crate::arena::Arena) -> Option<RecordValue<'e>> {
		let mut value = RecordValue::zeroed();
		let dst = match layout.frame_bytes() {
			0 => value.as_mut_ptr(),
			bytes => stack::push(bytes),
		};
		let written = self.write_into(layout, dst, arena);
		if layout.frame_bytes() != 0 {
			stack::truncate_above(dst, layout.frame_bytes());
			value = RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) });
		}
		written.map(|()| value)
	}

	fn write_into(&self, layout: &Layout, dst: *mut u8, arena: &crate::arena::Arena) -> Option<()> {
		unsafe { std::ptr::copy_nonoverlapping(self.bytes.as_ptr(), dst, self.bytes.len()) };
		if let Some(element) = &self.element {
			unsafe { (layout.element.repark)(&**element, dst, arena) }?;
		}
		for (index, value) in &self.fields {
			let field = &layout.fields[*index];
			let repark = field.repark.expect("copied fields carry re-park glue");
			unsafe { repark(&**value, dst.add(field.offset), arena) }?;
		}
		Some(())
	}
}

/// Law-test scaffolding: wraps an arbitrary plain node onto a record wire
/// (the element lands at offset 0 of a fresh element-only record, parked when
/// it carries drop glue). No production path constructs one; value edges are
/// [`crate::value::ValueSource`].
pub struct RecordLift<El, N> {
	edge: N,
	layout: Layout,
	_marker: std::marker::PhantomData<fn() -> El>,
}

impl<El: Clone + Send + Sync + 'static, N> RecordLift<El, N> {
	pub fn new(edge: N) -> Self {
		Self {
			edge,
			layout: Layout::default().with_writes(0, element_write::<El>(), &[]),
			_marker: std::marker::PhantomData,
		}
	}
}

impl<'e, C, El, N> Node<C> for RecordLift<El, N>
where
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	El: Send + Sync + 'static,
	N: Node<C, Output = El>,
{
	type Output = RecordValue<'e>;

	fn eval(&self, input: &C) -> GPoll<RecordValue<'e>> {
		lift_poll(self.edge.eval(input), &self.layout, input.arena())
	}

	fn layout(&self) -> &Layout {
		&self.layout
	}
}

/// Law-test scaffolding: a plain probe over a record wire, cloning the
/// element out of the parked reference when it carries drop glue. No
/// production path constructs one.
pub struct RecordExtract<El, N> {
	edge: N,
	layout: Layout,
	_marker: std::marker::PhantomData<fn() -> El>,
}

impl<El, N> RecordExtract<El, N> {
	pub fn new(edge: N, layout: &Layout) -> Self {
		Self {
			edge,
			layout: layout.clone(),
			_marker: std::marker::PhantomData,
		}
	}
}

impl<'e, C, El, N> Node<C> for RecordExtract<El, N>
where
	El: Clone + 'static,
	N: Node<C, Output = RecordValue<'e>>,
{
	type Output = El;

	fn eval(&self, input: &C) -> GPoll<El> {
		self.edge.eval(input).map(|value| unsafe { read_element::<El>(self.layout.rec(&value)) })
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
			Some(plan) if plan.union.frame_bytes() == 0 => self.edge.eval(input).map(|value| {
				let mut out = RecordValue::zeroed();
				unsafe { plan.translate(plan.source.rec(&value), out.as_mut_ptr()) };
				out
			}),
			Some(plan) => {
				let dst = stack::push(plan.union.frame_bytes());
				let value = self.edge.eval(input);
				value.map(|value| RecordValue::spilled(unsafe { plan.translate(plan.source.rec(&value), dst) }))
			}
		}
	}

	fn layout(&self) -> &Layout {
		&self.union
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	unsafe fn unread(_: *const u8) -> Box<dyn crate::list::AnyAttributeValue> {
		unreachable!("layout-only test field")
	}

	fn sized_field(name: &'static str, size: usize, align: usize) -> FieldWrite {
		FieldWrite {
			name,
			level: 0,
			size,
			align,
			read_erased: unread,
			repark: None,
		}
	}

	fn f64_field(name: &'static str) -> FieldWrite {
		sized_field(name, 8, 8)
	}

	#[test]
	fn canonical_order_and_offsets() {
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[sized_field("tint", 4, 4), f64_field("opacity"), sized_field("flag", 1, 1)]);
		assert_eq!(layout.offset_of("opacity", 0), Some(8));
		assert_eq!(layout.offset_of("tint", 0), Some(16));
		assert_eq!(layout.offset_of("flag", 0), Some(20));
		assert_eq!(layout.size, 21);
		assert_eq!(layout.align, 8);
	}

	#[test]
	#[should_panic(expected = "two different sizes")]
	fn size_conflicts_panic() {
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		layout.with_writes(0, element_write::<f64>(), &[sized_field("opacity", 4, 4)]);
	}

	#[test]
	fn union_is_order_independent() {
		let a = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		let b = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("length")]);
		assert_eq!(Layout::union(&[&a, &b]), Layout::union(&[&b, &a]));
		assert!(Layout::union(&[&a, &b]).offset_of("length", 0).is_some());
	}

	#[test]
	fn translation_moves_fields_and_fills_census_defaults() {
		let source = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("length")]);
		let union = Layout::union(&[&source, &Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")])]);

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
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		assert!(SourcePlan::new(&layout, &layout.clone()).is_none());
	}

	#[test]
	fn elements_ride_as_bytes_exactly_without_drop_glue() {
		assert!(!element_parked::<f64>());
		assert!(!element_parked::<[f64; 4]>());
		assert!(element_parked::<String>());
		assert!(element_parked::<std::sync::Arc<str>>());
		assert_eq!(element_dims::<[f64; 4]>(), (32, 8));
		assert_eq!(element_dims::<String>(), (8, 8));
	}

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
		stack::reserve(layout.frame_bytes());
		let value = copy.replay(&layout, &replay_arena).unwrap();
		let rec = layout.rec(&value);
		assert_eq!(unsafe { read_element::<String>(rec) }, "element");
		assert_eq!(unsafe { rec.read::<&str>(layout.offset_of("name", 0).unwrap()) }, "field");
	}

	#[test]
	fn record_values_are_one_word() {
		assert_eq!(size_of::<RecordValue>(), 8);
		assert_eq!(align_of::<RecordValue>(), 8);
	}

	#[test]
	fn layouts_resolve_spilled_values() {
		let small = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		assert_eq!(small.frame_bytes(), 16);
		let backing = [4f64, 0.5];
		let value = RecordValue::spilled(unsafe { Rec::new(backing.as_ptr().cast()) });
		assert_eq!(unsafe { small.rec(&value).element::<f64>() }, 4.);
		assert_eq!(unsafe { small.rec(&value).read::<f64>(small.offset_of("opacity", 0).unwrap()) }, 0.5);

		let spilled = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity"), f64_field("length")]);
		assert_eq!(spilled.frame_bytes(), 24);
		let record = [1f64, 2., 3.];
		let value = RecordValue::spilled(unsafe { Rec::new(record.as_ptr().cast()) });
		assert_eq!(unsafe { spilled.rec(&value).element::<f64>() }, 1.);
		assert_eq!(unsafe { spilled.rec(&value).read::<f64>(spilled.offset_of("length", 0).unwrap()) }, 2.);
		assert_eq!(unsafe { spilled.rec(&value).read::<f64>(spilled.offset_of("opacity", 0).unwrap()) }, 3.);
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
