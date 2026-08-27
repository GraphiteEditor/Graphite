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
	/// Hashes the field's content. `None` means the stored bytes are the
	/// content, which holds for every unparked value.
	pub content_hash: Option<unsafe fn(*const u8, &mut dyn core::hash::Hasher)>,
	/// Compares two fields' content. `None` as for `content_hash`.
	pub content_eq: Option<unsafe fn(*const u8, *const u8) -> bool>,
}

impl FieldWrite {
	pub fn of<A: crate::attribute::Attribute>(level: u8) -> Self
	where
		A::Value<'static>: graphene_hash::CacheHash + PartialEq,
	{
		unsafe fn content_hash<V: graphene_hash::CacheHash>(ptr: *const u8, state: &mut dyn core::hash::Hasher) {
			let mut state = state;
			unsafe { &*ptr.cast::<V>() }.cache_hash(&mut state);
		}
		unsafe fn content_eq<V: PartialEq>(a: *const u8, b: *const u8) -> bool {
			unsafe { &*a.cast::<V>() == &*b.cast::<V>() }
		}
		Self {
			name: A::NAME,
			level,
			size: size_of::<A::Value<'static>>(),
			align: align_of::<A::Value<'static>>(),
			read_erased: A::read_erased,
			repark: A::REPARK,
			content_hash: Some(content_hash::<A::Value<'static>>),
			content_eq: Some(content_eq::<A::Value<'static>>),
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
	/// Hashes the field's content. `None` means the stored bytes are the
	/// content, which holds for every unparked value.
	pub content_hash: Option<unsafe fn(*const u8, &mut dyn core::hash::Hasher)>,
	/// Compares two fields' content. `None` as for `content_hash`.
	pub content_eq: Option<unsafe fn(*const u8, *const u8) -> bool>,
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
	pub type_id: std::any::TypeId,
	pub clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	pub repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
	/// Hashes the element's content. `None` means the stored bytes are the
	/// content, which holds for every unparked element.
	pub content_hash: Option<unsafe fn(*const u8, &mut dyn core::hash::Hasher)>,
	/// Compares two elements' content. `None` as for `content_hash`.
	pub content_eq: Option<unsafe fn(*const u8, *const u8) -> bool>,
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
			type_id: std::any::TypeId::of::<()>(),
			clone_out,
			repark,
			content_hash: None,
			content_eq: None,
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

	/// One batch lane's stride: a spilled record's frame, or the value itself
	/// for records this layout keeps inline (`size == 0`), whose payload rides
	/// in the `RecordValue`'s own storage exactly as [`Layout::rec`] resolves.
	pub fn lane_stride(&self) -> usize {
		match self.size == 0 {
			true => size_of::<RecordValue<'static>>(),
			false => self.frame_bytes(),
		}
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
				content_hash: field.content_hash,
				content_eq: field.content_eq,
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
					content_hash: write.content_hash,
					content_eq: write.content_eq,
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
				content_hash: field.content_hash,
				content_eq: field.content_eq,
			})
			.collect();
		Layout::default().with_writes(self.depth, self.element, &retained)
	}

	/// The union of several layouts over the same element and depth.
	pub fn union(layouts: &[&Layout]) -> Layout {
		let first = layouts.first().expect("a union needs at least one layout");
		// A shallower source joins the union lifted: a scalar concatenates as
		// one lane, its fields sitting at the innermost level like any lane's.
		let depth = layouts.iter().map(|layout| layout.depth).max().unwrap_or(first.depth);
		let mut union = Layout::default().with_writes(depth, first.element, &[]);
		for layout in layouts {
			assert_eq!(union.element, layout.element, "union layouts must share the element");
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
					content_hash: field.content_hash,
					content_eq: field.content_eq,
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
	/// Inputs whose value cannot change with the innermost index, as a bitmask
	/// over input positions. Empty is the safe default: an uninstalled layout
	/// rebinds every input per lane.
	pub lane_invariant: u32,
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
	/// The materialized subject a reducer folds, as `(input, levels)`. The fold
	/// consumes the whole subject wire, so only the node's own levels remain.
	pub folded: Option<(u8, u8)>,
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
			folded: None,
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
		let depth = match self.folded {
			// A fold consumes the whole subject wire (a deeper wire folds its
			// total flat span), so only the node's own levels remain.
			Some(_) => self.level_delta.max(0) as u8,
			None => (base.depth as i8 + self.level_delta).max(0) as u8,
		};
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
		RecordLayout {
			layout,
			frame_bytes,
			plan,
			lane_invariant: 0,
		}
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
	fn extent_at_derived(&self, ctx: &C, level: u8) -> GPoll<crate::gpoll::Extent>;
}

impl<'derived, C, N> DerivedRecordEdge<'derived, C> for N
where
	N: Node<C, Output = RecordValue<'derived>>,
{
	fn eval_derived(&self, cell: &crate::node::StatusCell, input_index: usize, ctx: &C) -> Result<RecordValue<'derived>, crate::gpoll::Interrupt> {
		cell.eval_input(input_index, self, ctx)
	}

	fn extent_at_derived(&self, ctx: &C, level: u8) -> GPoll<crate::gpoll::Extent> {
		self.extent_at(ctx, level)
	}
}

/// Fills caller scratch with one frame per lane of `range`: the edge
/// evaluates at each index, the record's frame copies out, and the stack
/// rewinds, so the stack peak stays at one lane's need and every lane's bytes
/// are distinct. Frame bytes carry no drop glue, so the copy is a move.
pub fn fill_frames<'a, 'e, C, N>(node: &'a N, input: &C, range: std::ops::Range<u64>, scratch: Option<&'a mut [std::mem::MaybeUninit<u64>]>) -> crate::node::BatchStatus<'a>
where
	C: crate::context::InjectIndex + Copy,
	N: Node<C, Output = RecordValue<'e>>,
{
	use crate::node::BatchStatus;
	let Some(scratch) = scratch else {
		return BatchStatus::NeedBuffer;
	};
	let Some(len) = range.end.checked_sub(range.start).and_then(|len| usize::try_from(len).ok()) else {
		return BatchStatus::InvalidRange;
	};
	let layout = node.layout();
	let stride = layout.lane_stride();
	if scratch.len() * 8 < len * stride {
		return BatchStatus::InvalidRange;
	}
	let base = scratch.as_mut_ptr().cast::<u8>();
	let mut local = *input;
	let mut finality = crate::gpoll::Finality::AllFinal;
	let mut filled = len;
	let mut hint = crate::gpoll::Extent::AtLeast(range.end as usize);
	for lane in 0..len {
		local.set_index(range.start + lane as u64);
		let mark = stack::sp();
		let value = match node.eval(&local) {
			GPoll::Final(value) => value,
			GPoll::Partial(value) => {
				finality = crate::gpoll::Finality::Partial;
				value
			}
			GPoll::Pending => return BatchStatus::Pending,
			GPoll::Fallback(boxed) => return BatchStatus::Error(boxed.1),
			// A lane past a lower-bound level ends the data: the fill comes
			// back short and the hint turns exact.
			GPoll::Error(error) if error.kind == crate::gpoll::ErrorKind::PastEnd => {
				filled = lane;
				hint = crate::gpoll::Extent::Exactly(range.start as usize + lane);
				// SAFETY: the failed lane produced no record, so nothing above
				// its mark is live.
				unsafe { stack::rewind(mark) };
				break;
			}
			GPoll::Error(error) => return BatchStatus::Error(*error),
		};
		// SAFETY: the lane region is in-bounds by the scratch check, and the
		// frame is fully copied out before the rewind releases it.
		unsafe {
			std::ptr::copy_nonoverlapping(layout.rec(&value).ptr(), base.add(lane * stride), stride);
			stack::rewind(mark);
		}
	}
	// SAFETY: the first `filled` lanes were filled above with records of `layout`.
	BatchStatus::Filled(unsafe { crate::node::RecordBatchMut::new(scratch, filled, layout) }, finality, hint)
}

/// The driver a consumer runs on a record edge: a resident batch returns with
/// no allocation, a node's own batch impl gets `n * frame_bytes` of arena
/// scratch, and an unbatched edge falls back to the [`fill_frames`] loop.
pub fn materialize_batch<'a, 'e, C, N>(node: &'a N, input: &'a C, range: std::ops::Range<u64>, arena: &'a crate::arena::Arena) -> crate::node::BatchStatus<'a>
where
	C: crate::context::InjectIndex + Copy,
	N: Node<C, Output = RecordValue<'e>>,
{
	use crate::node::BatchStatus;
	let Some(len) = range.end.checked_sub(range.start).and_then(|len| usize::try_from(len).ok()) else {
		return BatchStatus::InvalidRange;
	};
	let words = len * node.layout().lane_stride() / 8;
	let exhausted = || {
		BatchStatus::Error(crate::gpoll::GraphError {
			kind: crate::gpoll::ErrorKind::ArenaExhausted,
			trace: Vec::new(),
		})
	};
	match node.eval_batch(input, range.clone(), None) {
		BatchStatus::Unbatched => match arena.alloc_scratch::<u64>(words) {
			Some(scratch) => fill_frames(node, input, range, Some(scratch)),
			None => exhausted(),
		},
		BatchStatus::NeedBuffer => match arena.alloc_scratch::<u64>(words) {
			Some(scratch) => node.eval_batch(input, range, Some(scratch)),
			None => exhausted(),
		},
		status => status,
	}
}

/// The outcome of materializing a leveled edge's whole flat span.
pub enum LevelStatus<'a> {
	Batch(crate::node::RecordBatch<'a>, crate::gpoll::Finality),
	Pending,
	Error(crate::gpoll::GraphError),
}

/// Evaluates a leveled edge's whole flat span into one batch: an exact total
/// fills once, a lower bound drains by guess-and-double until a short fill,
/// each reply's hint seeding the next guess. The boundary consumers' driver;
/// reducers inline the same protocol with their span offsets.
pub fn materialize_level<'a, 'e, C, N>(node: &'a N, input: &'a C, arena: &'a crate::arena::Arena) -> LevelStatus<'a>
where
	C: crate::context::InjectIndex + Copy,
	N: Node<C, Output = RecordValue<'e>>,
{
	use crate::gpoll::{Extent, GraphError, Level};
	use crate::node::BatchStatus;
	let sized = match node.extent(input, Level::Total) {
		GPoll::Final(Extent::Exactly(count)) => Ok(count),
		GPoll::Final(Extent::AtLeast(bound)) => Err(bound),
		GPoll::Pending => return LevelStatus::Pending,
		_ => return LevelStatus::Error(GraphError::new("materialize over a non-exact extent")),
	};
	match sized {
		Ok(count) => match materialize_batch(node, input, 0..count as u64, arena) {
			BatchStatus::Lent(batch, finality, _) => LevelStatus::Batch(batch, finality),
			BatchStatus::Filled(batch, finality, _) => LevelStatus::Batch(batch.into_shared(), finality),
			BatchStatus::Pending => LevelStatus::Pending,
			BatchStatus::Error(error) => LevelStatus::Error(error),
			_ => LevelStatus::Error(GraphError::new("materialize batch failed")),
		},
		Err(bound) => {
			let mut guess = bound.max(16);
			loop {
				let (batch, finality, hint) = match materialize_batch(node, input, 0..guess as u64, arena) {
					BatchStatus::Lent(batch, finality, hint) => (batch, finality, hint),
					BatchStatus::Filled(batch, finality, hint) => (batch.into_shared(), finality, hint),
					BatchStatus::Pending => return LevelStatus::Pending,
					BatchStatus::Error(error) => return LevelStatus::Error(error),
					_ => return LevelStatus::Error(GraphError::new("materialize batch failed")),
				};
				let filled = batch.len();
				if filled < guess {
					break LevelStatus::Batch(batch, finality);
				}
				match hint {
					Extent::Exactly(total) if total <= filled => break LevelStatus::Batch(batch, finality),
					Extent::Exactly(total) => guess = total,
					Extent::AtLeast(more) => guess = (guess * 2).max(more),
					Extent::Free => guess *= 2,
				}
			}
		}
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

	/// [`materialize_level`] over the edge: the wire's whole flat span as one
	/// batch.
	pub fn materialize_level<'e, 'b, C>(&'b self, ctx: &'b C, arena: &'b crate::arena::Arena) -> LevelStatus<'b>
	where
		N: Node<C, Output = RecordValue<'e>>,
		C: crate::context::InjectIndex + Copy,
	{
		materialize_level(self.node, ctx, arena)
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
	pub fn with_reads(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, layout: &'a Layout, reads: &'a [Option<usize>], read: unsafe fn(Rec, &[Option<usize>]) -> Out) -> Self {
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
	inner_levels: u8,
	_lifetime: std::marker::PhantomData<fn() -> RecordValue<'e>>,
}

impl<'a, 'e, N> RecordLazyInput<'a, 'e, N> {
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, inner_levels: u8) -> Self {
		Self {
			node,
			cell,
			input_index,
			inner_levels,
			_lifetime: std::marker::PhantomData,
		}
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<RecordValue<'e>, crate::gpoll::Interrupt>
	where
		N: DerivedRecordEdge<'d, C>,
	{
		Ok(self.node.eval_derived(self.cell, self.input_index, ctx)?.rebind())
	}

	/// The flat lane count of one copy: the product of the edge's inner-level
	/// extents, queried uniform across copies (at copy 0). The dividend of a
	/// structure node's decompose-and-promote.
	pub fn inner_extent<B>(&self, ctx: &B) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, 0, self.inner_levels, self.input_index)
	}

	/// The flat lane count of the copy at `copy`, for edges whose inner
	/// extents vary per copy.
	pub fn inner_extent_at<B>(&self, ctx: &B, copy: u64) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, copy, self.inner_levels, self.input_index)
	}
}

/// See [`RecordLazyInput::inner_extent`].
fn inner_extent_of<B, N>(node: &N, ctx: &B, copy: u64, levels: u8, input_index: usize) -> Result<u64, crate::gpoll::Interrupt>
where
	B: crate::context::DeriveCtx,
	N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
{
	let mut frame = crate::context::IndexLink { index: 0, outer: None };
	let derived = ctx.push_level(&mut frame, copy, 0);
	let mut inner: u64 = 1;
	for level in 0..levels {
		match node.extent_at_derived(&derived, level) {
			GPoll::Final(crate::gpoll::Extent::Exactly(count)) => inner *= count as u64,
			GPoll::Final(crate::gpoll::Extent::AtLeast(_)) => return probed_inner(node, ctx, copy, input_index),
			GPoll::Pending => return Err(crate::gpoll::Interrupt::Pending),
			_ => return Err(crate::gpoll::GraphError::new("structure decomposition over a non-exact extent").into()),
		}
	}
	Ok(inner)
}

/// The flat lane count of one copy of a lower-bound edge, probed by
/// evaluating lanes to the past-end signal. The probed records are
/// discarded, and their statuses land in a scratch cell.
fn probed_inner<B, N>(node: &N, ctx: &B, copy: u64, input_index: usize) -> Result<u64, crate::gpoll::Interrupt>
where
	B: crate::context::DeriveCtx,
	N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
{
	let cell = crate::node::StatusCell::new();
	let mut count: u64 = 0;
	loop {
		let mark = stack::sp();
		let mut frame = crate::context::IndexLink { index: 0, outer: None };
		let probe = ctx.push_level(&mut frame, copy, count);
		let result = node.eval_derived(&cell, input_index, &probe);
		// SAFETY: the probed record is discarded, so nothing above the mark
		// is live.
		unsafe { stack::rewind(mark) };
		match result {
			Ok(_) => count += 1,
			Err(crate::gpoll::Interrupt::Error(error)) if error.kind == crate::gpoll::ErrorKind::PastEnd => return Ok(count),
			Err(interrupt) => return Err(interrupt),
		}
	}
}

/// The derive-routing carrier beside its declared attribute reads: evaluating
/// at a derived context yields the opaque row token and the read values in one
/// step, so the kernel drives the per-copy eval while reads stay resolved
/// against the source's wired layout.
#[derive(Clone, Copy)]
pub struct DerivedLazyInput<'a, 'e, Out, N> {
	node: &'a N,
	cell: &'a crate::node::StatusCell,
	input_index: usize,
	inner_levels: u8,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec, &[Option<usize>]) -> Out,
	_lifetime: std::marker::PhantomData<fn() -> RecordValue<'e>>,
}

impl<'a, 'e, Out, N> DerivedLazyInput<'a, 'e, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, inner_levels: u8, reads: &'a [Option<usize>], read: unsafe fn(Rec, &[Option<usize>]) -> Out) -> Self {
		Self {
			node,
			cell,
			input_index,
			inner_levels,
			reads,
			read,
			_lifetime: std::marker::PhantomData,
		}
	}

	/// The flat lane count of one copy; see [`RecordLazyInput::inner_extent`].
	pub fn inner_extent<B>(&self, ctx: &B) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, 0, self.inner_levels, self.input_index)
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<Out, crate::gpoll::Interrupt>
	where
		N: DerivedRecordEdge<'d, C>,
	{
		let value: RecordValue<'e> = self.node.eval_derived(self.cell, self.input_index, ctx)?.rebind();
		// SAFETY: declared reads imply a non-empty layout, so the record is
		// spilled and its pointer is the frame the offsets index into.
		Ok(unsafe { (self.read)(Rec::new(value.ptr), self.reads) })
	}
}

/// The read-less [`DerivedLazyInput`] glue: the token alone.
///
/// # Safety
/// `rec` must be a spilled record's frame.
pub unsafe fn token_only<'e>(rec: Rec, _reads: &[Option<usize>]) -> RecordValue<'e> {
	RecordValue::spilled(rec)
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

/// Deep-copy overrides for element types whose plain clone borrows the
/// evaluation's arena (a `Graphic` holding a group interior). The generic
/// element glue consults this registry, so every layout carrying such an
/// element deep-copies at memo and capture seams regardless of which
/// constructor built the glue. The clone-out must produce a value of the
/// element's own type that owns all of its content; the re-park restores that
/// value's arena-resident form before parking it.
#[derive(Clone, Copy)]
struct DeepElementGlue {
	clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
}

static DEEP_ELEMENT_CLONES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, DeepElementGlue>>> = std::sync::LazyLock::new(Default::default);

/// Registers the deep copy-out and re-park pair for elements of `T`. Called
/// at startup from the crate that owns the type.
pub fn register_deep_element_clone<T: 'static>(
	clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
) {
	DEEP_ELEMENT_CLONES.lock().unwrap().insert(std::any::TypeId::of::<T>(), DeepElementGlue { clone_out, repark });
}

fn deep_element_glue(type_id: std::any::TypeId) -> Option<DeepElementGlue> {
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
struct DeepFieldGlue {
	copy_out: fn(&dyn crate::list::AnyAttributeValue) -> Option<Box<dyn crate::list::AnyAttributeValue>>,
	replay: fn(&dyn crate::list::AnyAttributeValue, &crate::arena::Arena) -> Option<Option<Box<dyn crate::list::AnyAttributeValue>>>,
}

static DEEP_FIELD_VALUES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, DeepFieldGlue>>> = std::sync::LazyLock::new(Default::default);

/// Registers the deep copy-out and replay pair for field values of `T`.
/// Called at startup from the crate that owns the type.
pub fn register_deep_field_value<T: 'static>(
	copy_out: fn(&dyn crate::list::AnyAttributeValue) -> Option<Box<dyn crate::list::AnyAttributeValue>>,
	replay: fn(&dyn crate::list::AnyAttributeValue, &crate::arena::Arena) -> Option<Option<Box<dyn crate::list::AnyAttributeValue>>>,
) {
	DEEP_FIELD_VALUES.lock().unwrap().insert(std::any::TypeId::of::<T>(), DeepFieldGlue { copy_out, replay });
}

fn deep_field_glue(type_id: std::any::TypeId) -> Option<DeepFieldGlue> {
	DEEP_FIELD_VALUES.lock().unwrap().get(&type_id).copied()
}

fn deepen_field_value(value: Box<dyn crate::list::AnyAttributeValue>) -> Box<dyn crate::list::AnyAttributeValue> {
	match deep_field_glue(value.as_any().type_id()) {
		Some(glue) => (glue.copy_out)(&*value).unwrap_or(value),
		None => value,
	}
}

/// The element slot a record wire of `T` carries, its erased glue bound at
/// the statically-known type.
pub fn element_write<T: Clone + Send + Sync + 'static>() -> ElementWrite {
	unsafe fn clone_out<T: Clone + Send + Sync + 'static>(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
		if let Some(deep) = deep_element_glue(std::any::TypeId::of::<T>()) {
			return unsafe { (deep.clone_out)(ptr) };
		}
		Box::new(unsafe { read_element::<T>(Rec::new(ptr)) })
	}
	unsafe fn repark<T: Clone + Send + Sync + 'static>(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &crate::arena::Arena) -> Option<()> {
		if let Some(deep) = deep_element_glue(std::any::TypeId::of::<T>()) {
			return unsafe { (deep.repark)(value, dst, arena) };
		}
		let value = value.downcast_ref::<T>().expect("an element replays at its own type");
		unsafe { write_element(dst, value.clone(), arena) }
	}
	let (size, align) = element_dims::<T>();
	ElementWrite {
		size,
		align,
		parked: element_parked::<T>(),
		type_id: std::any::TypeId::of::<T>(),
		clone_out: clone_out::<T>,
		repark: repark::<T>,
		content_hash: None,
		content_eq: None,
	}
}

/// [`element_write`] plus the content hashing and equality glue, for element
/// types that support them.
pub fn element_write_hashed<T: Clone + Send + Sync + graphene_hash::CacheHash + PartialEq + 'static>() -> ElementWrite {
	unsafe fn content_hash<T: graphene_hash::CacheHash>(ptr: *const u8, state: &mut dyn core::hash::Hasher) {
		let mut state = state;
		unsafe { borrow_element::<T>(Rec::new(ptr)) }.cache_hash(&mut state);
	}
	unsafe fn content_eq<T: PartialEq>(a: *const u8, b: *const u8) -> bool {
		unsafe { borrow_element::<T>(Rec::new(a)) == borrow_element::<T>(Rec::new(b)) }
	}
	ElementWrite {
		content_hash: Some(content_hash::<T>),
		content_eq: Some(content_eq::<T>),
		..element_write::<T>()
	}
}

/// Selects [`element_write_hashed`] when the element type supports the
/// content glue and [`element_write`] otherwise, by autoref method
/// resolution: call `(&ElementWritePick::<T>(..)).element_write()` with both
/// traits in scope.
pub struct ElementWritePick<T>(pub std::marker::PhantomData<T>);

pub trait ElementWritePickHashed {
	fn element_write(&self) -> ElementWrite;
}

impl<T: Clone + Send + Sync + graphene_hash::CacheHash + PartialEq + 'static> ElementWritePickHashed for ElementWritePick<T> {
	fn element_write(&self) -> ElementWrite {
		element_write_hashed::<T>()
	}
}

pub trait ElementWritePickPlain {
	fn element_write(&self) -> ElementWrite;
}

impl<T: Clone + Send + Sync + 'static> ElementWritePickPlain for &ElementWritePick<T> {
	fn element_write(&self) -> ElementWrite {
		element_write::<T>()
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

/// Serves a record whose frame lives in storage the current evaluation does
/// not own (a cached batch, published bytes): claims the node's frame over a
/// copy of the source frame, so the contract that every node advances the
/// record stack by exactly its own frame holds for cache hits too.
///
/// # Safety
/// `src` must point at a live record of `layout` whose parked references
/// outlive the serving evaluation.
pub unsafe fn serve_frame<'e>(layout: &Layout, src: *const u8) -> RecordValue<'e> {
	if layout.frame_bytes() == 0 {
		let mut value = RecordValue::zeroed();
		unsafe { std::ptr::copy_nonoverlapping(src, value.as_mut_ptr(), layout.size) };
		value
	} else {
		let dst = stack::push(layout.frame_bytes());
		unsafe { std::ptr::copy_nonoverlapping(src, dst, layout.size) };
		RecordValue::spilled(unsafe { Rec::new(dst.cast_const()) })
	}
}

/// Claims a node's frame with nothing to serve, for exits that yield no
/// record (past-end, pending): the frame contract holds on every exit,
/// error included.
pub fn claim_frame(layout: &Layout) {
	if layout.frame_bytes() != 0 {
		stack::push(layout.frame_bytes());
	}
}

/// Closes an interrupted eval: rewinds to the eval's entry pointer (interrupt
/// exits carry no value, so frames claimed above it are dead) and claims the
/// node's own frame, keeping the frame contract on interrupt exits.
///
/// # Safety
/// `entry` must be the stack pointer at the eval's entry, and no record
/// above it may be referenced after the close.
pub unsafe fn interrupt_frame(entry: usize, layout: &Layout) {
	unsafe { stack::rewind(entry) };
	claim_frame(layout);
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
			.map(|(index, field)| (index, deepen_field_value(unsafe { (field.read_erased)(rec.ptr().add(field.offset)) })))
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
			match deep_field_glue(value.as_any().type_id()) {
				Some(glue) => match (glue.replay)(&**value, arena)? {
					Some(resident) => unsafe { repark(&*resident, dst.add(field.offset), arena) }?,
					None => unsafe { repark(&**value, dst.add(field.offset), arena) }?,
				},
				None => unsafe { repark(&**value, dst.add(field.offset), arena) }?,
			}
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

/// A plain probe over a record wire, cloning the element out of the parked
/// reference when it carries drop glue. Registry constructors wrap a record
/// edge in one to feed a node's plain value input, keeping the wire kind
/// uniform.
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
		let mark = stack::sp();
		let result = self.edge.eval(input).map(|value| unsafe { read_element::<El>(self.layout.rec(&value)) });
		// SAFETY: the element copied out by value, so no record above the mark
		// (the edge's frame) is live; a plain output claims no frame itself.
		unsafe { stack::rewind(mark) };
		result
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
			Some(plan) if plan.union.frame_bytes() == 0 => {
				let mark = stack::sp();
				let result = self.edge.eval(input).map(|value| {
					let mut out = RecordValue::zeroed();
					unsafe { plan.translate(plan.source.rec(&value), out.as_mut_ptr()) };
					out
				});
				// SAFETY: the translation copied the record into the inline
				// value, so no record above the mark is live.
				unsafe { stack::rewind(mark) };
				result
			}
			Some(plan) => {
				let dst = stack::push(plan.union.frame_bytes());
				let value = self.edge.eval(input);
				let result = value.map(|value| RecordValue::spilled(unsafe { plan.translate(plan.source.rec(&value), dst) }));
				// The source's frame dies with the translation; the claimed
				// frame above stays as this node's output.
				stack::truncate_above(dst, plan.union.frame_bytes());
				result
			}
		}
	}

	fn extent_at(&self, input: &C, level: u8) -> GPoll<crate::gpoll::Extent> {
		self.edge.extent_at(input, level)
	}

	fn layout(&self) -> &Layout {
		&self.union
	}
}

/// `len` records stored in the arena at `layout`'s stride. The layout is
/// owned by the value and identifies the run's element type. The records are
/// valid for the current evaluation, like every arena payload. An owned item
/// ([`Self::copy_out`]) survives the generation instead, and must
/// [`Self::replay`] into a serving arena before any read.
#[derive(Clone, Debug)]
pub struct GroupItem {
	layout: Layout,
	storage: ItemStorage,
	len: usize,
}

#[derive(Clone)]
enum ItemStorage {
	Resident(*const u8),
	Owned(std::sync::Arc<OwnedLanes>),
}

impl std::fmt::Debug for ItemStorage {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			ItemStorage::Resident(frames) => write!(f, "Resident({frames:p})"),
			ItemStorage::Owned(_) => f.write_str("Owned(..)"),
		}
	}
}

/// The lanes deep-copied out of their evaluation: the packed bytes plus owned
/// clones of every parked payload, per lane. The byte image's parked
/// references are stale until [`GroupItem::replay`] re-parks the payloads.
struct OwnedLanes {
	bytes: Box<[u8]>,
	elements: Option<Vec<Box<dyn std::any::Any + Send + Sync>>>,
	fields: Vec<(usize, Vec<Box<dyn crate::list::AnyAttributeValue>>)>,
}

// SAFETY: the same argument as for `RecordValue`. The element bounds and the
// parking discipline make the record bytes thread-safe, and their validity
// is tied to the shared arena.
unsafe impl Send for GroupItem {}
// SAFETY: as `Send`.
unsafe impl Sync for GroupItem {}

impl GroupItem {
	/// Copies the batch's lanes into the arena and clones its layout.
	/// Returns `None` when the arena is exhausted. Parked regions must carry
	/// the content glue, so equality and hashing never fall back to pointer
	/// bytes.
	pub fn adopt(batch: crate::node::RecordBatch<'_>, arena: &crate::arena::Arena) -> Option<Self> {
		let layout = batch.layout().clone();
		assert!(!layout.element.parked || layout.element.content_hash.is_some(), "a parked element adopts only with content glue");
		for field in &layout.fields {
			assert!(field.repark.is_none() || field.content_hash.is_some(), "a parked field adopts only with content glue");
		}
		let stride = layout.lane_stride();
		let scratch = arena.alloc_scratch::<u64>((batch.len() * stride).div_ceil(8))?;
		let frames = scratch.as_mut_ptr().cast::<u8>();
		for lane in 0..batch.len() {
			// SAFETY: both sides hold `len` lanes at the shared layout's stride.
			unsafe { std::ptr::copy_nonoverlapping(batch.get(lane).rec().ptr(), frames.add(lane * stride), stride) };
		}
		Some(Self {
			layout,
			storage: ItemStorage::Resident(frames.cast_const()),
			len: batch.len(),
		})
	}

	/// The resident frame base. An owned item has none until it replays.
	fn frames(&self) -> *const u8 {
		match &self.storage {
			ItemStorage::Resident(frames) => *frames,
			ItemStorage::Owned(_) => panic!("an owned item replays into an arena before it is read"),
		}
	}

	pub fn len(&self) -> usize {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	pub fn layout(&self) -> &Layout {
		&self.layout
	}

	/// A `GroupItem` over the batch's frames, without copying.
	///
	/// # Safety
	/// The frames must stay valid for the evaluation. Arena-resident batches
	/// qualify, caller stack scratch does not.
	pub unsafe fn from_resident(batch: crate::node::RecordBatch<'_>) -> Self {
		let layout = batch.layout().clone();
		assert!(!layout.element.parked || layout.element.content_hash.is_some(), "a parked element adopts only with content glue");
		for field in &layout.fields {
			assert!(field.repark.is_none() || field.content_hash.is_some(), "a parked field adopts only with content glue");
		}
		Self {
			storage: ItemStorage::Resident(batch.frames_ptr()),
			len: batch.len(),
			layout,
		}
	}

	/// A batch view over the stored records. Panics on an owned item, which
	/// must [`Self::replay`] first.
	pub fn lanes(&self) -> crate::node::RecordBatch<'_> {
		// SAFETY: the constructors store `len` lanes of `layout` at the
		// layout's stride.
		unsafe { crate::node::RecordBatch::new(self.frames(), self.len, &self.layout) }
	}

	/// The item deep-copied out of its evaluation: lane bytes plus owned
	/// clones of every parked payload, for storage that outlives the arena
	/// generation. An owned item cannot be read until it replays.
	pub fn copy_out(&self) -> GroupItem {
		if let ItemStorage::Owned(_) = &self.storage {
			return self.clone();
		}
		let stride = self.layout.lane_stride();
		let frames = self.frames();
		// SAFETY: the constructors store `len` lanes of `layout` at the
		// layout's stride, and the erased glue reads each lane's own region.
		let bytes: Box<[u8]> = unsafe { std::slice::from_raw_parts(frames, self.len * stride) }.into();
		let elements = self
			.layout
			.element
			.parked
			.then(|| (0..self.len).map(|lane| unsafe { (self.layout.element.clone_out)(frames.add(lane * stride)) }).collect());
		let fields = self
			.layout
			.fields
			.iter()
			.enumerate()
			.filter(|(_, field)| field.repark.is_some())
			.map(|(index, field)| {
				let mut values: Vec<_> = (0..self.len).map(|lane| unsafe { (field.read_erased)(frames.add(lane * stride + field.offset)) }).collect();
				if let Some(glue) = values.first().and_then(|value| deep_field_glue(value.as_any().type_id())) {
					for value in &mut values {
						if let Some(deepened) = (glue.copy_out)(&**value) {
							*value = deepened;
						}
					}
				}
				(index, values)
			})
			.collect();
		GroupItem {
			layout: self.layout.clone(),
			storage: ItemStorage::Owned(std::sync::Arc::new(OwnedLanes { bytes, elements, fields })),
			len: self.len,
		}
	}

	/// Re-parks an owned item's lanes into `arena`, restoring the resident
	/// form; `None` reports arena exhaustion. A resident item returns a plain
	/// clone.
	pub fn replay(&self, arena: &crate::arena::Arena) -> Option<GroupItem> {
		let ItemStorage::Owned(owned) = &self.storage else {
			return Some(self.clone());
		};
		let stride = self.layout.lane_stride();
		let scratch = arena.alloc_scratch::<u64>((self.len * stride).div_ceil(8))?;
		let frames = scratch.as_mut_ptr().cast::<u8>();
		// SAFETY: the scratch holds `len` lanes at the layout's stride, and the
		// re-park glue writes each lane's own region under that layout.
		unsafe { std::ptr::copy_nonoverlapping(owned.bytes.as_ptr(), frames, owned.bytes.len()) };
		if let Some(elements) = &owned.elements {
			for (lane, element) in elements.iter().enumerate() {
				unsafe { (self.layout.element.repark)(&**element, frames.add(lane * stride), arena) }?;
			}
		}
		for (index, values) in &owned.fields {
			let field = &self.layout.fields[*index];
			let repark = field.repark.expect("copied fields carry re-park glue");
			let glue = values.first().and_then(|value| deep_field_glue(value.as_any().type_id()));
			for (lane, value) in values.iter().enumerate() {
				match glue {
					Some(glue) => match (glue.replay)(&**value, arena)? {
						Some(resident) => unsafe { repark(&*resident, frames.add(lane * stride + field.offset), arena) }?,
						None => unsafe { repark(&**value, frames.add(lane * stride + field.offset), arena) }?,
					},
					None => unsafe { repark(&**value, frames.add(lane * stride + field.offset), arena) }?,
				}
			}
		}
		Some(GroupItem {
			layout: self.layout.clone(),
			storage: ItemStorage::Resident(frames.cast_const()),
			len: self.len,
		})
	}

	/// A typed view over the stored records, checked against the layout's
	/// element type.
	pub fn typed_lanes<T: 'static>(&self) -> Option<crate::node::List<'_, T>> {
		match self.layout.element.type_id == std::any::TypeId::of::<T>() {
			// SAFETY: the layout records the element type the lanes hold.
			true => Some(unsafe { crate::node::List::new(self.lanes()) }),
			false => None,
		}
	}
}

/// A run read at its element type. Record fields hold each marker's value
/// verbatim, so lane reads are plain typed reads at a hoisted offset.
pub struct RunView<'a, T> {
	item: &'a GroupItem,
	lanes: crate::node::List<'a, T>,
}

impl<'a, T: 'static> RunView<'a, T> {
	/// `None` where the run holds another element type.
	pub fn new(item: &'a GroupItem) -> Option<Self> {
		item.typed_lanes::<T>().map(|lanes| Self { item, lanes })
	}
}

/// A marker's field on a run, its offset resolved once.
pub struct RunColumn<'a, A: crate::attribute::Attribute> {
	item: &'a GroupItem,
	offset: Option<usize>,
	marker: std::marker::PhantomData<A>,
}

impl<'a, A: crate::attribute::Attribute> crate::lane::LaneColumn<'a, A> for RunColumn<'a, A> {
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>> {
		// SAFETY: the offset comes from the item's own layout, whose field at
		// this name holds this marker's value type by census registration.
		self.offset.map(|offset| unsafe { self.item.lanes().get(lane).rec().ptr().add(offset).cast::<A::Value<'a>>().read() })
	}
}

impl<'a, T: 'static> crate::lane::LaneSource for RunView<'a, T> {
	type Element = T;
	type Column<'b, A: crate::attribute::Attribute>
		= RunColumn<'b, A>
	where
		Self: 'b;

	fn lane_count(&self) -> usize {
		self.lanes.len()
	}

	fn element(&self, lane: usize) -> Option<&T> {
		(lane < self.lanes.len()).then(|| self.lanes.element_ref(lane))
	}

	fn column<A: crate::attribute::Attribute>(&self) -> RunColumn<'_, A> {
		RunColumn {
			item: self.item,
			offset: self.item.layout().offset_of(A::NAME, 0),
			marker: std::marker::PhantomData,
		}
	}
}

impl<T: crate::render_complexity::RenderComplexity + 'static> crate::render_complexity::RenderComplexity for RunView<'_, T> {
	fn render_complexity(&self) -> usize {
		use crate::lane::LaneSource;
		(0..self.lane_count()).filter_map(|lane| self.element(lane)).map(crate::render_complexity::RenderComplexity::render_complexity).sum()
	}
}

impl<T: crate::bounds::BoundingBox + 'static> crate::bounds::BoundingBox for RunView<'_, T> {
	fn bounding_box(&self, transform: glam::DAffine2, include_stroke: bool) -> crate::bounds::RenderBoundingBox {
		crate::bounds::lane_bounding_box(self, transform, include_stroke)
	}

	fn thumbnail_bounding_box(&self, transform: glam::DAffine2, include_stroke: bool) -> crate::bounds::RenderBoundingBox {
		crate::bounds::lane_thumbnail_bounding_box(self, transform, include_stroke)
	}
}

/// Records nested inside one element: a single homogeneous run. The `row`
/// holds the group's own attribute record. A group that sits on a lane leaves
/// it `None`, because that lane's record carries the attributes. The typed
/// segment stack returns when merge constructs segments.
#[derive(Clone, Debug)]
pub struct Group {
	pub row: Option<GroupItem>,
	pub content: GroupItem,
}

impl Group {
	/// The group deep-copied out of its evaluation, every run in owned form.
	pub fn copy_out(&self) -> Group {
		Group {
			row: self.row.as_ref().map(GroupItem::copy_out),
			content: self.content.copy_out(),
		}
	}

	/// Re-parks an owned group's runs into `arena`; `None` reports arena
	/// exhaustion.
	pub fn replay(&self, arena: &crate::arena::Arena) -> Option<Group> {
		let row = match &self.row {
			Some(row) => Some(row.replay(arena)?),
			None => None,
		};
		Some(Group {
			row,
			content: self.content.replay(arena)?,
		})
	}
}

/// Compares one record region of `layout` by content. Regions without glue
/// compare as bytes, which is the content for unparked values.
unsafe fn record_content_eq(layout: &Layout, a: *const u8, b: *const u8) -> bool {
	let bytes_eq = |offset: usize, size: usize| unsafe { std::slice::from_raw_parts(a.add(offset), size) == std::slice::from_raw_parts(b.add(offset), size) };
	let element = match layout.element.content_eq {
		Some(eq) => unsafe { eq(a, b) },
		None => bytes_eq(0, layout.element.size),
	};
	element
		&& layout.fields.iter().all(|field| match field.content_eq {
			Some(eq) => unsafe { eq(a.add(field.offset), b.add(field.offset)) },
			None => bytes_eq(field.offset, field.size),
		})
}

/// Hashes one record region of `layout` by content, with the byte fallback
/// of [`record_content_eq`].
unsafe fn record_content_hash(layout: &Layout, ptr: *const u8, state: &mut dyn core::hash::Hasher) {
	match layout.element.content_hash {
		Some(hash) => unsafe { hash(ptr, state) },
		None => state.write(unsafe { std::slice::from_raw_parts(ptr, layout.element.size) }),
	}
	for field in &layout.fields {
		match field.content_hash {
			Some(hash) => unsafe { hash(ptr.add(field.offset), state) },
			None => state.write(unsafe { std::slice::from_raw_parts(ptr.add(field.offset), field.size) }),
		}
	}
}

fn layout_shape_hash(layout: &Layout, state: &mut dyn core::hash::Hasher) {
	state.write_u8(layout.depth);
	state.write_usize(layout.fields.len());
	for field in &layout.fields {
		state.write(field.name.as_bytes());
		state.write_u8(field.level);
		state.write_usize(field.offset);
		state.write_usize(field.size);
	}
}

impl PartialEq for GroupItem {
	fn eq(&self, other: &Self) -> bool {
		if self.layout != other.layout || self.len != other.len {
			return false;
		}
		let stride = self.layout.lane_stride();
		let (a, b) = (self.frames(), other.frames());
		// SAFETY: both sides hold `len` lanes of the shared layout.
		(0..self.len).all(|lane| unsafe { record_content_eq(&self.layout, a.add(lane * stride), b.add(lane * stride)) })
	}
}

impl graphene_hash::CacheHash for GroupItem {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		layout_shape_hash(&self.layout, state);
		state.write_usize(self.len);
		let stride = self.layout.lane_stride();
		let frames = self.frames();
		for lane in 0..self.len {
			// SAFETY: `adopt` filled `len` lanes of `layout`.
			unsafe { record_content_hash(&self.layout, frames.add(lane * stride), state) };
		}
	}
}

impl PartialEq for Group {
	fn eq(&self, other: &Self) -> bool {
		self.row == other.row && self.content == other.content
	}
}

impl graphene_hash::CacheHash for Group {
	fn cache_hash<H: core::hash::Hasher>(&self, state: &mut H) {
		state.write_u8(self.row.is_some() as u8);
		if let Some(row) = &self.row {
			row.cache_hash(state);
		}
		self.content.cache_hash(state);
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
			content_hash: None,
			content_eq: None,
		}
	}

	fn f64_field(name: &'static str) -> FieldWrite {
		sized_field(name, 8, 8)
	}

	#[test]
	fn a_run_and_a_legacy_list_serve_the_same_values() {
		use crate::attribute::{Attribute, Opacity, Transform};
		use crate::lane::LaneSource;
		use glam::DAffine2;

		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[FieldWrite::of::<Transform>(0), FieldWrite::of::<Opacity>(0)]);
		let stride = layout.lane_stride();
		let transform = DAffine2::from_translation((5., 6.).into());
		let mut bytes = vec![0u8; stride * 2];
		for lane in 0..2 {
			// SAFETY: `bytes` is `stride` per lane, and the offsets come from `layout`.
			unsafe {
				let base = bytes.as_mut_ptr().add(lane * stride);
				base.cast::<f64>().write(lane as f64);
				base.add(layout.offset_of(Transform::NAME, 0).unwrap()).cast::<DAffine2>().write(transform);
				base.add(layout.offset_of(Opacity::NAME, 0).unwrap()).cast::<f64>().write(0.25);
			}
		}
		// SAFETY: `bytes` holds two lanes of `layout` at its stride.
		let item = unsafe { GroupItem::from_resident(crate::node::RecordBatch::new(bytes.as_ptr(), 2, &layout)) };
		let run = RunView::<f64>::new(&item).expect("the run holds f64 elements");

		let mut list = crate::list::List::new_from_element(0f64);
		list.push(crate::list::Item::new_from_element(1f64));
		for lane in 0..2 {
			list.set_attribute(Transform::NAME, lane, transform);
			list.set_attribute(Opacity::NAME, lane, 0.25);
		}

		assert_eq!(run.lane_count(), list.lane_count());
		for lane in 0..2 {
			assert_eq!(run.attr::<Transform>(lane), list.attr::<Transform>(lane));
			assert_eq!(run.attr::<Opacity>(lane), list.attr::<Opacity>(lane));
			assert_eq!(run.element(lane), list.element(lane));
		}
	}

	#[test]
	fn a_run_missing_a_column_serves_the_census_default() {
		use crate::attribute::Opacity;
		use crate::lane::LaneSource;

		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[]);
		let bytes = vec![0u8; layout.lane_stride()];
		// SAFETY: `bytes` holds one lane of `layout` at its stride.
		let item = unsafe { GroupItem::from_resident(crate::node::RecordBatch::new(bytes.as_ptr(), 1, &layout)) };
		let run = RunView::<f64>::new(&item).expect("the run holds f64 elements");

		assert_eq!(run.attr::<Opacity>(0), 1.);
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
	fn adopted_lanes_round_trip_through_the_group_item() {
		let arena = crate::arena::Arena::new(1024).unwrap();
		let layout = Layout::default().with_writes(1, element_write::<f64>(), &[f64_field("opacity")]);
		let stride = layout.lane_stride();
		let offset = layout.offset_of("opacity", 0).unwrap();

		let mut buffer = vec![0u64; (2 * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		for lane in 0..2usize {
			unsafe {
				base.add(lane * stride).cast::<f64>().write(10. + lane as f64);
				base.add(lane * stride + offset).cast::<f64>().write(0.5 + lane as f64);
			}
		}
		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 2, &layout) };

		let item = GroupItem::adopt(batch, &arena).unwrap();
		// The source buffer dies before the reads: the adopted copy must not
		// alias it.
		buffer.fill(0);

		assert_eq!(item.len(), 2);
		assert_eq!(item.layout(), &layout);
		let lanes = item.lanes();
		for lane in 0..2usize {
			assert_eq!(unsafe { lanes.get(lane).rec().element::<f64>() }, 10. + lane as f64, "lane {lane}");
			assert_eq!(unsafe { lanes.get(lane).rec().read::<f64>(offset) }, 0.5 + lane as f64, "lane {lane}");
		}
	}

	#[test]
	fn the_element_write_pick_selects_the_content_glue_by_type() {
		use super::{ElementWritePickHashed as _, ElementWritePickPlain as _};

		#[derive(Clone)]
		struct Opaque;

		let hashed = (&ElementWritePick::<String>(std::marker::PhantomData)).element_write();
		assert!(hashed.content_hash.is_some() && hashed.content_eq.is_some());
		let plain = (&ElementWritePick::<Opaque>(std::marker::PhantomData)).element_write();
		assert!(plain.content_hash.is_none() && plain.content_eq.is_none());
	}

	#[test]
	fn group_content_equality_and_hashing_read_through_the_park() {
		use graphene_hash::CacheHash;

		let arena = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(1, element_write_hashed::<String>(), &[]);
		let stride = layout.lane_stride();

		let build = |labels: &[&str]| {
			let mut buffer = vec![0u64; (labels.len() * stride).div_ceil(8)];
			let base = buffer.as_mut_ptr().cast::<u8>();
			for (lane, label) in labels.iter().enumerate() {
				unsafe { write_element(base.add(lane * stride), label.to_string(), &arena) }.unwrap();
			}
			let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), labels.len(), &layout) };
			GroupItem::adopt(batch, &arena).unwrap()
		};
		let digest = |item: &GroupItem| {
			let mut state = std::collections::hash_map::DefaultHasher::new();
			item.cache_hash(&mut state);
			std::hash::Hasher::finish(&state)
		};

		// Each build parks its strings at fresh arena addresses, so equality
		// and hashing must read the parked content, not the pointer bytes.
		let a = build(&["x", "y"]);
		let b = build(&["x", "y"]);
		let c = build(&["x", "z"]);
		assert_eq!(a, b);
		assert_eq!(digest(&a), digest(&b));
		assert_ne!(a, c);
		assert_ne!(digest(&a), digest(&c));
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
	fn owned_items_replay_re_parked_payloads_after_the_source_dies() {
		let layout = Layout::default().with_writes(0, element_write_hashed::<String>(), &[FieldWrite::of::<crate::attribute::Name>(0)]);
		let stride = layout.lane_stride();
		let mut bytes = vec![0u8; stride * 2];

		let owned = {
			let arena = crate::arena::Arena::new(1024).unwrap();
			for lane in 0..2 {
				let base = unsafe { bytes.as_mut_ptr().add(lane * stride) };
				unsafe { write_element(base, format!("element {lane}"), &arena) }.unwrap();
				let (name, _) = arena.alloc(format!("field {lane}")).unwrap();
				unsafe { write_field::<&str>(base, layout.offset_of("name", 0).unwrap(), name.as_str()) };
			}
			let item = unsafe { GroupItem::from_resident(crate::node::RecordBatch::new(bytes.as_ptr(), 2, &layout)) };
			item.copy_out()
		};
		bytes.fill(u8::MAX);

		let arena = crate::arena::Arena::new(1024).unwrap();
		let replayed = owned.replay(&arena).unwrap();
		let lanes = replayed.lanes();
		for lane in 0..2 {
			let rec = lanes.get(lane).rec();
			assert_eq!(unsafe { read_element::<String>(rec) }, format!("element {lane}"));
			assert_eq!(unsafe { rec.read::<&str>(layout.offset_of("name", 0).unwrap()) }, format!("field {lane}"));
		}
	}

	#[test]
	#[should_panic(expected = "an owned item replays")]
	fn an_owned_item_refuses_reads() {
		let layout = Layout::default().with_writes(0, element_write_hashed::<String>(), &[]);
		let mut bytes = vec![0u8; layout.lane_stride()];
		let arena = crate::arena::Arena::new(1024).unwrap();
		unsafe { write_element(bytes.as_mut_ptr(), String::from("parked"), &arena) }.unwrap();
		let item = unsafe { GroupItem::from_resident(crate::node::RecordBatch::new(bytes.as_ptr(), 1, &layout)) };
		item.copy_out().lanes();
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
