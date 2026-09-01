//! The packed-record tier at rank 0. A record is the element at offset 0
//! plus one field per written attribute; its [`Layout`] is computed at
//! wiring from the upstream write set and never serialized. Records of
//! inline layouts live in the [`RecordValue`] itself; larger ones live as
//! per-lane views on the evaluation's [`Frames`], which the root owns and
//! every node claims its own frame out of. Kernels route them as opaque
//! [`RecordValue`]s that carry
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
	pub type_id: std::any::TypeId,
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
		A::Value<'static>: graphene_hash::CacheHash + PartialEq + 'static,
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
			type_id: std::any::TypeId::of::<A::Value<'static>>(),
			read_erased: A::read_erased,
			repark: A::REPARK,
			content_hash: Some(content_hash::<A::Value<'static>>),
			content_eq: Some(content_eq::<A::Value<'static>>),
		}
	}
}

/// One field of a [`Layout`]: a (name, level) key resolved to an offset.
/// Levels are numbered innermost-out; only level 0 exists at rank 0.
/// Equality is structural over the field's identity, its value type
/// included; only the glue pointers are excluded, since fn-pointer identity
/// is not guaranteed across codegen units and layout equality drives
/// identity forwarding.
#[derive(Clone, Debug)]
pub struct FieldDesc {
	pub name: &'static str,
	pub level: u8,
	pub offset: usize,
	pub size: usize,
	pub align: usize,
	pub type_id: std::any::TypeId,
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
		(self.name, self.level, self.offset, self.size, self.align, self.type_id) == (other.name, other.level, other.offset, other.size, other.align, other.type_id)
	}
}

impl FieldDesc {
	/// The write this field re-declares when a layout's fields fold into
	/// another layout.
	pub fn as_write(&self) -> FieldWrite {
		FieldWrite {
			name: self.name,
			level: self.level,
			size: self.size,
			align: self.align,
			type_id: self.type_id,
			read_erased: self.read_erased,
			repark: self.repark,
			content_hash: self.content_hash,
			content_eq: self.content_eq,
		}
	}
}

impl Eq for FieldDesc {}

/// The element slot of a layout: its dimensions plus erased glue bound where
/// the element type is statically known, so generic consumers read or
/// deep-copy the element without it. Equality is structural over the
/// element's identity, its type included; glue pointers are excluded for the
/// same reason as [`FieldDesc`]'s.
#[derive(Clone, Copy, Debug)]
pub struct ElementWrite {
	pub size: usize,
	pub align: usize,
	pub parked: bool,
	pub type_id: std::any::TypeId,
	pub clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	pub repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
	/// Moves a parked payload's header into the persistent region rather than
	/// cloning the heap it owns, returning the new header. `None` declines,
	/// which leaves the caller its clone path.
	pub park_move: unsafe fn(*const u8, &Promotion<'_>) -> Option<*const u8>,
	/// Hashes the element's content. `None` means the stored bytes are the
	/// content, which holds for every unparked element.
	pub content_hash: Option<unsafe fn(*const u8, &mut dyn core::hash::Hasher)>,
	/// Compares two elements' content. `None` as for `content_hash`.
	pub content_eq: Option<unsafe fn(*const u8, *const u8) -> bool>,
}

impl PartialEq for ElementWrite {
	fn eq(&self, other: &Self) -> bool {
		(self.size, self.align, self.parked, self.type_id) == (other.size, other.align, other.parked, other.type_id)
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
		unsafe fn park_move(_parked: *const u8, _promotion: &Promotion<'_>) -> Option<*const u8> {
			None
		}
		Self {
			size: 0,
			align: 0,
			parked: false,
			type_id: std::any::TypeId::of::<()>(),
			clone_out,
			repark,
			park_move,
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
	pub fn rec<'v>(&self, value: &'v RecordValue<'_>) -> Rec<'v> {
		match self.size == 0 {
			true => Rec((&raw const *value).cast(), std::marker::PhantomData),
			false => Rec(value.ptr, std::marker::PhantomData),
		}
	}

	/// The union of this layout's fields and `writes` over `element` at
	/// `depth`, in canonical order. A (name, level) written at a different
	/// size is a type conflict and panics; the census keeps declared names to
	/// one type, so this only fires on wiring bugs.
	pub fn with_writes(&self, depth: u8, element: ElementWrite, writes: &[FieldWrite]) -> Layout {
		let mut merged: Vec<FieldWrite> = self.fields.iter().map(FieldDesc::as_write).collect();
		for &write in writes {
			match merged.iter().find(|field| field.name == write.name && field.level == write.level) {
				Some(existing) => assert_eq!(existing.type_id, write.type_id, "attribute `{}` written at two different types", write.name),
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
					type_id: write.type_id,
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
		let retained: Vec<FieldWrite> = self.fields.iter().filter(|field| !removes.contains(&(field.name, field.level))).map(FieldDesc::as_write).collect();
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
			let writes: Vec<FieldWrite> = layout.fields.iter().map(FieldDesc::as_write).collect();
			union = union.with_writes(union.depth, union.element, &writes);
		}
		union
	}
}

/// A field handle minted once against a layout: the marker's (name, level)
/// resolved to a field index and offset, with the marker-to-type proof taken
/// there. Resolving it against a layout re-checks that one index instead of
/// scanning names, so a token paired with any other layout resolves to
/// nothing rather than to the wrong bytes.
pub struct FieldOffset<A: attribute::Attribute> {
	index: usize,
	offset: usize,
	level: u8,
	marker: std::marker::PhantomData<fn() -> A>,
}

impl<A: attribute::Attribute> Clone for FieldOffset<A> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<A: attribute::Attribute> Copy for FieldOffset<A> {}

impl<A: attribute::Attribute> std::fmt::Debug for FieldOffset<A> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("FieldOffset").field("name", &A::NAME).field("offset", &self.offset).field("level", &self.level).finish()
	}
}

impl<A: attribute::Attribute> FieldOffset<A> {
	/// The marker's field at `level`, `None` where the layout does not carry
	/// it. Panics where the layout declares the name at another value type,
	/// which the census forbids.
	pub fn of(layout: &Layout, level: u8) -> Option<Self> {
		let (index, field) = layout.fields.iter().enumerate().find(|(_, field)| field.name == A::NAME && field.level == level)?;
		assert_eq!(field.type_id, std::any::TypeId::of::<A::Value<'static>>(), "attribute `{}` is declared at another value type", A::NAME);
		Some(Self {
			index,
			offset: field.offset,
			level,
			marker: std::marker::PhantomData,
		})
	}

	pub fn offset(self) -> usize {
		self.offset
	}

	/// The offset this token names in `layout`, `None` unless `layout` is the
	/// one it was minted against. The field's value type is re-checked, so a
	/// resolved offset carries the marker-to-type proof into `layout`.
	pub fn resolve(self, layout: &Layout) -> Option<usize> {
		let field = layout.fields.get(self.index)?;
		let same = field.offset == self.offset && field.level == self.level && field.name == A::NAME && field.type_id == std::any::TypeId::of::<A::Value<'static>>();
		same.then_some(self.offset)
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

/// A view of one record: a pointer whose layout is proven at wiring, borrowing
/// the storage it points into for `'r`.
#[derive(Clone, Copy, Debug)]
pub struct Rec<'r>(*const u8, std::marker::PhantomData<&'r u8>);

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
	fn rebind<'a>(self) -> RecordValue<'a> {
		RecordValue {
			ptr: self.ptr,
			_lifetime: std::marker::PhantomData,
		}
	}
}

/// A record edge evaluable at a derived context, yielding the record at that
/// context's lifetime. The lifetime is a trait parameter because a bound like
/// `for<'d> Node<Derived<'d, C>>` cannot also say the derived context's arena
/// is at `'d`: the equality binding `ExtractArena<ArenaRef = &'d Arena>` is an
/// unconstrained position under a higher rank.
pub trait DerivedRecordEdge<'derived, C> {
	fn eval_derived(&self, cell: &crate::node::StatusCell, input_index: usize, ctx: &C, frames: &Frames<'derived>) -> Result<RecordValue<'derived>, crate::gpoll::Interrupt>;
	fn extent_at_derived(&self, ctx: &C, level: u8, frames: &Frames<'derived>) -> GPoll<crate::gpoll::Extent>;
}

impl<'derived, C, N> DerivedRecordEdge<'derived, C> for N
where
	N: Node<C>,
	C: crate::context::ExtractArena<ArenaRef = &'derived crate::arena::Arena>,
{
	fn eval_derived(&self, cell: &crate::node::StatusCell, input_index: usize, ctx: &C, frames: &Frames<'derived>) -> Result<RecordValue<'derived>, crate::gpoll::Interrupt> {
		cell.eval_input(input_index, self, ctx, frames)
	}

	fn extent_at_derived(&self, ctx: &C, level: u8, frames: &Frames<'derived>) -> GPoll<crate::gpoll::Extent> {
		self.extent_at(ctx, level, frames)
	}
}

/// Fills caller scratch with one frame per lane of `range`: the edge serves
/// into the lane's own region of the slab, and the lane's own frame space is
/// free again at the next lane, so the frame peak stays at one lane's need and
/// every lane's bytes are distinct.
pub fn fill_frames<'a, 'e, C, N>(node: &'a N, input: &C, range: std::ops::Range<u64>, scratch: Option<&'a mut [std::mem::MaybeUninit<u64>]>, frames: &Frames<'e>) -> crate::node::BatchStatus<'a>
where
	C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	N: Node<C>,
{
	use crate::node::BatchStatus;
	let Some(scratch) = scratch else {
		return BatchStatus::NeedBuffer;
	};
	let Some(len) = range.end.checked_sub(range.start).and_then(|len| usize::try_from(len).ok()) else {
		return BatchStatus::InvalidRange;
	};
	let Some(mut run) = frames.run(scratch, len, node.layout()) else {
		return BatchStatus::InvalidRange;
	};
	let mut local = *input;
	let mut finality = crate::gpoll::Finality::AllFinal;
	let mut hint = crate::gpoll::Extent::AtLeast(range.end as usize);
	for lane in 0..len {
		local.set_index(range.start + lane as u64);
		let lane_frames = frames.scope();
		let slot = run.slot(lane, &lane_frames);
		let served = match node.serve(&local, slot) {
			GPoll::Final(served) => served,
			GPoll::Partial(served) => {
				finality = crate::gpoll::Finality::Partial;
				served
			}
			GPoll::Pending => return BatchStatus::Pending,
			GPoll::Fallback(boxed) => return BatchStatus::Error(boxed.1),
			// A lane past a lower-bound level ends the data: the fill comes
			// back short and the hint turns exact.
			GPoll::Error(error) if error.kind == crate::gpoll::ErrorKind::PastEnd => {
				hint = crate::gpoll::Extent::Exactly(range.start as usize + lane);
				break;
			}
			GPoll::Error(error) => return BatchStatus::Error(*error),
		};
		run.served(lane, &served);
	}
	BatchStatus::Filled(run.finish(), finality, hint)
}

/// The driver a consumer runs on a record edge: a resident batch returns with
/// no allocation, a node's own batch impl gets `n * frame_bytes` of arena
/// scratch, and an unbatched edge falls back to the [`fill_frames`] loop.
pub fn materialize_batch<'a, 'e, C, N>(node: &'a N, input: &'a C, range: std::ops::Range<u64>, arena: &'a crate::arena::Arena, frames: &Frames<'e>) -> crate::node::BatchStatus<'a>
where
	C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	N: Node<C>,
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
	match node.eval_batch(input, range.clone(), None, frames) {
		BatchStatus::Unbatched => match arena.alloc_scratch::<u64>(words) {
			Some(scratch) => fill_frames(node, input, range, Some(scratch), frames),
			None => exhausted(),
		},
		BatchStatus::NeedBuffer => match arena.alloc_scratch::<u64>(words) {
			Some(scratch) => node.eval_batch(input, range, Some(scratch), frames),
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
pub fn materialize_level<'a, 'e, C, N>(node: &'a N, input: &'a C, arena: &'a crate::arena::Arena, frames: &Frames<'e>) -> LevelStatus<'a>
where
	C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	N: Node<C>,
{
	use crate::gpoll::{Extent, GraphError, Level};
	use crate::node::BatchStatus;
	let sized = match node.extent(input, Level::Total, frames) {
		GPoll::Final(Extent::Exactly(count)) => Ok(count),
		GPoll::Final(Extent::AtLeast(bound)) => Err(bound),
		GPoll::Pending => return LevelStatus::Pending,
		_ => return LevelStatus::Error(GraphError::new("materialize over a non-exact extent")),
	};
	match sized {
		Ok(count) => match materialize_batch(node, input, 0..count as u64, arena, frames) {
			BatchStatus::Lent(batch, finality, _) => LevelStatus::Batch(batch, finality),
			BatchStatus::Filled(batch, finality, _) => LevelStatus::Batch(batch.into_shared(), finality),
			BatchStatus::Pending => LevelStatus::Pending,
			BatchStatus::Error(error) => LevelStatus::Error(error),
			_ => LevelStatus::Error(GraphError::new("materialize batch failed")),
		},
		Err(bound) => {
			let mut guess = bound.max(16);
			loop {
				let (batch, finality, hint) = match materialize_batch(node, input, 0..guess as u64, arena, frames) {
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

/// The raw lazy record edge handed to a record-opaque kernel: the wire plus
/// its wiring-proven layout, the pairing the kernel's unsafe record
/// operations rely on. The kernel must only pair the layout with values this
/// edge produced.
pub struct RecordEdgeInput<'a, 'e, N> {
	node: &'a N,
	layout: &'a Layout,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, N> RecordEdgeInput<'a, 'e, N> {
	pub fn new(node: &'a N, layout: &'a Layout, frames: &'a Frames<'e>) -> Self {
		Self { node, layout, frames }
	}

	pub fn layout(&self) -> &Layout {
		self.layout
	}

	/// Serves the edge through the kernel's own claim: the kernel's output
	/// layout is the edge's, so the claim it was handed is the edge's frame.
	pub fn serve<'l, C>(&self, ctx: &C, slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
	where
		N: Node<C>,
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		self.node.serve(ctx, slot)
	}

	/// [`materialize_level`] over the edge: the wire's whole flat span as one
	/// batch.
	pub fn materialize_level<'b, C>(&'b self, ctx: &'b C, arena: &'b crate::arena::Arena) -> LevelStatus<'b>
	where
		N: Node<C>,
		C: crate::context::InjectIndex + Copy + crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		materialize_level(self.node, ctx, arena, self.frames)
	}
}

/// The raw lazy edge handed to a poll kernel whose wire rides records while
/// the kernel consumes the plain element.
/// # Safety
/// `rec` must be a record of the layout the offsets were resolved against
/// and `El` its element type; both are proven at wiring.
unsafe fn element_only<El: Clone>(rec: Rec<'_>, _reads: &[Option<usize>]) -> El {
	unsafe { read_element::<El>(rec) }
}

pub struct ElementEdge<'a, 'e, Out, N> {
	node: &'a N,
	layout: &'a Layout,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, El: Clone, N> ElementEdge<'a, 'e, El, N> {
	pub fn new(node: &'a N, layout: &'a Layout, frames: &'a Frames<'e>) -> Self {
		Self {
			node,
			layout,
			reads: &[],
			read: element_only::<El>,
			frames,
		}
	}
}

impl<'a, 'e, Out, N> ElementEdge<'a, 'e, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn with_reads(node: &'a N, layout: &'a Layout, reads: &'a [Option<usize>], read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out, frames: &'a Frames<'e>) -> Self {
		Self { node, layout, reads, read, frames }
	}

	/// The edge's element at `ctx`, read out of a record claimed beyond the
	/// kernel's own frame; the claim dies with the call, so the record is free
	/// again at the next one.
	pub fn eval<'d, C>(&self, ctx: &C) -> GPoll<Out>
	where
		N: DerivedRecordEdge<'d, C>,
		'e: 'd,
	{
		let cell = crate::node::StatusCell::new();
		let scope = self.frames.scope();
		match self.node.eval_derived(&cell, 0, ctx, &scope) {
			// SAFETY: the read copies out by value against the edge's own layout.
			Ok(value) => cell.finish(unsafe { (self.read)(self.layout.rec(&value), self.reads) }),
			Err(interrupt) => interrupt.into(),
		}
	}
}

/// The lazy input handed to a kernel whose edge rides a record wire while
/// the kernel consumes the plain element, or the element beside its declared
/// attribute reads.
#[derive(Clone, Copy)]
pub struct ElementLazyInput<'a, 'e, Out, N> {
	node: &'a N,
	cell: &'a crate::node::StatusCell,
	input_index: usize,
	layout: &'a Layout,
	reads: &'a [Option<usize>],
	read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, El: Clone, N> ElementLazyInput<'a, 'e, El, N> {
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, layout: &'a Layout, frames: &'a Frames<'e>) -> Self {
		Self {
			node,
			cell,
			input_index,
			layout,
			reads: &[],
			read: element_only::<El>,
			frames,
		}
	}
}

impl<'a, 'e, Out, N> ElementLazyInput<'a, 'e, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn with_reads(
		node: &'a N,
		cell: &'a crate::node::StatusCell,
		input_index: usize,
		layout: &'a Layout,
		reads: &'a [Option<usize>],
		read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
		frames: &'a Frames<'e>,
	) -> Self {
		Self {
			node,
			cell,
			input_index,
			layout,
			reads,
			read,
			frames,
		}
	}

	/// The read copies the element and declared attributes out by value, so
	/// the record's claim dies with the call.
	pub fn eval<'d, C>(&self, ctx: &C) -> Result<Out, crate::gpoll::Interrupt>
	where
		N: DerivedRecordEdge<'d, C>,
		'e: 'd,
	{
		let scope = self.frames.scope();
		let value = self.node.eval_derived(self.cell, self.input_index, ctx, &scope)?;
		// SAFETY: the reads are the edge's own layout's, resolved at wiring.
		Ok(unsafe { (self.read)(self.layout.rec(&value), self.reads) })
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
	frames: &'a Frames<'e>,
}

impl<'a, 'e, N> RecordLazyInput<'a, 'e, N> {
	pub fn new(node: &'a N, cell: &'a crate::node::StatusCell, input_index: usize, inner_levels: u8, frames: &'a Frames<'e>) -> Self {
		Self {
			node,
			cell,
			input_index,
			inner_levels,
			frames,
		}
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<RecordValue<'e>, crate::gpoll::Interrupt>
	where
		N: DerivedRecordEdge<'d, C>,
		'e: 'd,
	{
		Ok(self.node.eval_derived(self.cell, self.input_index, ctx, self.frames)?.rebind())
	}

	/// The flat lane count of one copy: the product of the edge's inner-level
	/// extents, queried uniform across copies (at copy 0). The dividend of a
	/// structure node's decompose-and-promote.
	pub fn inner_extent<B>(&self, ctx: &B) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, 0, self.inner_levels, self.input_index, self.frames)
	}

	/// The flat lane count of the copy at `copy`, for edges whose inner
	/// extents vary per copy.
	pub fn inner_extent_at<B>(&self, ctx: &B, copy: u64) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, copy, self.inner_levels, self.input_index, self.frames)
	}
}

/// See [`RecordLazyInput::inner_extent`].
fn inner_extent_of<B, N>(node: &N, ctx: &B, copy: u64, levels: u8, input_index: usize, frames: &Frames<'_>) -> Result<u64, crate::gpoll::Interrupt>
where
	B: crate::context::DeriveCtx,
	N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
{
	let mut frame = crate::context::IndexLink { index: 0, outer: None };
	let derived = ctx.push_level(&mut frame, copy, 0);
	let mut inner: u64 = 1;
	for level in 0..levels {
		match node.extent_at_derived(&derived, level, frames) {
			GPoll::Final(crate::gpoll::Extent::Exactly(count)) => inner *= count as u64,
			GPoll::Final(crate::gpoll::Extent::AtLeast(_)) => return probed_inner(node, ctx, copy, input_index, frames),
			GPoll::Pending => return Err(crate::gpoll::Interrupt::Pending),
			_ => return Err(crate::gpoll::GraphError::new("structure decomposition over a non-exact extent").into()),
		}
	}
	Ok(inner)
}

/// The flat lane count of one copy of a lower-bound edge, probed by
/// evaluating lanes to the past-end signal. The probed records are
/// discarded, and their statuses land in a scratch cell.
fn probed_inner<B, N>(node: &N, ctx: &B, copy: u64, input_index: usize, frames: &Frames<'_>) -> Result<u64, crate::gpoll::Interrupt>
where
	B: crate::context::DeriveCtx,
	N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
{
	let cell = crate::node::StatusCell::new();
	let mut count: u64 = 0;
	loop {
		// The probed record is discarded, so the probe's claim is free again
		// at the next iteration.
		let probe_frames = frames.scope();
		let mut frame = crate::context::IndexLink { index: 0, outer: None };
		let probe = ctx.push_level(&mut frame, copy, count);
		let result = node.eval_derived(&cell, input_index, &probe, &probe_frames);
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
	read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
	frames: &'a Frames<'e>,
}

impl<'a, 'e, Out, N> DerivedLazyInput<'a, 'e, Out, N> {
	/// `read` must be sound against the layout the offsets in `reads` were
	/// resolved from; the macro proves both at wiring.
	pub fn new(
		node: &'a N,
		cell: &'a crate::node::StatusCell,
		input_index: usize,
		inner_levels: u8,
		reads: &'a [Option<usize>],
		read: unsafe fn(Rec<'_>, &[Option<usize>]) -> Out,
		frames: &'a Frames<'e>,
	) -> Self {
		Self {
			node,
			cell,
			input_index,
			inner_levels,
			reads,
			read,
			frames,
		}
	}

	/// The flat lane count of one copy; see [`RecordLazyInput::inner_extent`].
	pub fn inner_extent<B>(&self, ctx: &B) -> Result<u64, crate::gpoll::Interrupt>
	where
		B: crate::context::DeriveCtx,
		N: for<'d> DerivedRecordEdge<'d, crate::context::Derived<'d, B>>,
	{
		inner_extent_of(self.node, ctx, 0, self.inner_levels, self.input_index, self.frames)
	}

	pub fn eval<'d, C>(&self, ctx: &C) -> Result<Out, crate::gpoll::Interrupt>
	where
		N: DerivedRecordEdge<'d, C>,
		'e: 'd,
	{
		let value: RecordValue<'e> = self.node.eval_derived(self.cell, self.input_index, ctx, self.frames)?.rebind();
		// SAFETY: declared reads imply a non-empty layout, so the record is
		// spilled and its pointer is the frame the offsets index into.
		Ok(unsafe { (self.read)(Rec::new(value.ptr), self.reads) })
	}
}

/// The read-less [`DerivedLazyInput`] glue: the token alone.
///
/// # Safety
/// `rec` must be a spilled record's frame.
pub unsafe fn token_only<'e>(rec: Rec<'_>, _reads: &[Option<usize>]) -> RecordValue<'e> {
	RecordValue::spilled(rec)
}

/// The evaluation's record frame space: a grow-only buffer the executor owns
/// and lends by `&mut`, sized by the wiring-derived frame need, so exhaustion
/// is an accounting failure the debug assertion catches rather than a hot-path
/// branch. Frame bytes carry no drop glue, so growth and reuse are plain
/// buffer operations.
#[derive(Debug, Default)]
pub struct FrameArena {
	buf: Vec<u64>,
}

impl FrameArena {
	pub fn new() -> Self {
		Self { buf: Vec::new() }
	}

	/// Grows the buffer to hold `bytes`, the root's wiring-derived frame need.
	/// Grow-only, so repeated evaluations reuse one allocation.
	pub fn reserve(&mut self, bytes: usize) {
		let words = bytes.div_ceil(8).max(1);
		if self.buf.len() < words {
			self.buf.resize(words, 0);
		}
	}

	/// The whole buffer as free space, for one evaluation.
	pub fn frames(&mut self) -> Frames<'_> {
		let base = self.buf.as_mut_ptr().cast::<u8>();
		Frames {
			base: std::cell::Cell::new(base),
			words: std::cell::Cell::new(self.buf.len()),
			bounds: (base as usize, self.buf.len() * 8),
			_lifetime: std::marker::PhantomData,
		}
	}
}

/// The free frame space at one point of an evaluation. [`Self::claim`] splits
/// a node's own frame off the front and the claim carries the remainder, so a
/// node's inputs claim beyond its frame, one after another, and the space they
/// used is free again once the claim dies: the release is the claim's
/// lifetime, not a rewind contract. The cursor is shared through `&self` so
/// the lazy edges a kernel holds claim beyond each other rather than over each
/// other. Covariant in `'e`, so a claim minted at the evaluation shortens onto
/// a derived context's arena lifetime.
pub struct Frames<'e> {
	base: std::cell::Cell<*mut u8>,
	words: std::cell::Cell<usize>,
	/// The whole buffer as (address, bytes), which stays fixed while claims
	/// advance the cursor, so a promote can range-check a reference against it.
	bounds: (usize, usize),
	_lifetime: std::marker::PhantomData<&'e ()>,
}

impl std::fmt::Debug for Frames<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Frames").field("free_words", &self.words.get()).finish()
	}
}

impl<'e> Frames<'e> {
	/// An independent cursor over the same free space: claims made through it
	/// are free again when it dies, so a repeated evaluation reuses one region.
	/// Two live cursors hand out the same region, so only one may be claimed
	/// from at a time.
	pub fn reborrow(&self) -> Frames<'e> {
		Frames {
			base: std::cell::Cell::new(self.base.get()),
			words: std::cell::Cell::new(self.words.get()),
			bounds: self.bounds,
			_lifetime: std::marker::PhantomData,
		}
	}

	/// The whole frame buffer as (address, bytes), the range a promote treats
	/// as evaluation-lived.
	pub fn bounds(&self) -> (usize, usize) {
		self.bounds
	}

	/// Runs claims against this space and gives it back on the guard's drop, so
	/// a loop that evaluates a subtree per iteration reuses the same region.
	pub fn scope(&self) -> FrameScope<'_, 'e> {
		FrameScope {
			frames: self,
			base: self.base.get(),
			words: self.words.get(),
		}
	}

	/// The words still free, the observable the frame accounting asserts on.
	pub fn free_words(&self) -> usize {
		self.words.get()
	}

	/// Claims `layout`'s frame at the front of the free space; the claim
	/// carries the remainder, and an inline layout's record builds in the
	/// claim itself.
	pub fn claim<'l>(&self, layout: &'l Layout) -> FrameClaim<'e, 'l> {
		let frame = match layout.frame_bytes() {
			0 => None,
			bytes => Some(self.split(bytes)),
		};
		FrameClaim {
			layout,
			inline: RecordValue::zeroed(),
			frame,
			free: self.reborrow(),
		}
	}

	/// Splits `bytes` (rounded to word alignment) off the front.
	fn split(&self, bytes: usize) -> *mut u8 {
		let words = bytes.div_ceil(8);
		debug_assert!(words <= self.words.get(), "record frame space exhausted: the root buffer must cover the graph's frame need");
		let frame = self.base.get();
		// SAFETY: the buffer covers the wiring-derived need, so the advanced
		// cursor stays within it.
		self.base.set(unsafe { frame.add(words * 8) });
		self.words.set(self.words.get() - words);
		frame
	}

	/// A run of same-layout slots over caller scratch: lanes serve in place,
	/// each backed by its own region of the slab, and the collected proofs
	/// certify the filled prefix. `None` where the scratch cannot hold `len`
	/// lanes.
	pub fn run<'a>(&self, scratch: &'a mut [std::mem::MaybeUninit<u64>], len: usize, layout: &'a Layout) -> Option<SlotRun<'a>> {
		SlotRun::new(scratch, len, layout)
	}
}

/// Law-test scaffolding: a frame space of `bytes`, leaked so a fixture holds
/// it for the whole test without threading the buffer's own borrow. Production
/// roots own their buffer and lend it by `&mut`.
#[doc(hidden)]
pub fn test_frames(bytes: usize) -> Frames<'static> {
	let arena: &'static mut FrameArena = Box::leak(Box::new(FrameArena::new()));
	arena.reserve(bytes);
	arena.frames()
}

/// See [`Frames::scope`]. A forgotten guard leaks its region until the frame
/// space itself dies, rather than releasing it.
pub struct FrameScope<'s, 'e> {
	frames: &'s Frames<'e>,
	base: *mut u8,
	words: usize,
}

impl<'e> std::ops::Deref for FrameScope<'_, 'e> {
	type Target = Frames<'e>;

	fn deref(&self) -> &Frames<'e> {
		self.frames
	}
}

impl Drop for FrameScope<'_, '_> {
	fn drop(&mut self) {
		self.frames.base.set(self.base);
		self.frames.words.set(self.words);
	}
}

/// A claimed run of same-layout slots over the caller's scratch: [`Self::slot`]
/// backs an ordinary claim's own region by the lane's region of the slab, so a
/// lane serves in place with no staging copy, and [`Self::served`] records the
/// proof. The filled prefix is the only readable part, which is what makes
/// [`Self::finish`] safe.
pub struct SlotRun<'a> {
	scratch: &'a mut [std::mem::MaybeUninit<u64>],
	layout: &'a Layout,
	len: usize,
	filled: usize,
}

impl<'a> SlotRun<'a> {
	fn new(scratch: &'a mut [std::mem::MaybeUninit<u64>], len: usize, layout: &'a Layout) -> Option<SlotRun<'a>> {
		(scratch.len() * 8 >= len * layout.lane_stride()).then_some(SlotRun { scratch, layout, len, filled: 0 })
	}

	pub fn layout(&self) -> &'a Layout {
		self.layout
	}

	/// Lane `lane`'s claim: its own frame is the lane's region of the slab and
	/// its free space is `frames`, so the lane's inputs claim beyond it.
	pub fn slot<'e>(&mut self, lane: usize, frames: &Frames<'e>) -> FrameClaim<'e, 'a> {
		assert!(lane < self.len, "lane {lane} out of bounds for a run of {}", self.len);
		let stride = self.layout.lane_stride();
		// SAFETY: in-bounds by the assert against the capacity check `new` made.
		let frame = unsafe { self.scratch.as_mut_ptr().cast::<u8>().add(lane * stride) };

		FrameClaim {
			layout: self.layout,
			inline: RecordValue::zeroed(),
			frame: (self.layout.frame_bytes() != 0).then_some(frame),
			free: frames.reborrow(),
		}
	}

	/// Records lane `lane` as served; the proof came from that lane's slot. An
	/// inline record rides the value's own storage, so it takes the one copy
	/// the run makes.
	pub fn served(&mut self, lane: usize, proof: &Served<'_>) {
		if self.layout.size == 0 {
			let stride = self.layout.lane_stride();
			// SAFETY: in-bounds by the capacity check `new` made, and the lane
			// takes the whole inline record.
			unsafe { std::ptr::copy_nonoverlapping(self.layout.rec(proof.record()).ptr(), self.scratch.as_mut_ptr().cast::<u8>().add(lane * stride), stride) };
		}
		self.filled = self.filled.max(lane + 1);
	}

	/// The served lanes as the caller's exclusive batch.
	pub fn finish(self) -> crate::node::RecordBatchMut<'a> {
		crate::node::RecordBatchMut::new(self.scratch, self.filled, self.layout)
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
/// already carried into `dst`, and inline frames copy out of the scratch
/// bytes. Arena exhaustion of a parked element reports as an error poll.
///
/// # Safety
/// `dst` must be the claimed frame (or inline scratch when `frame_bytes` is
/// 0) of a record whose element is `T` and whose frame size is `frame_bytes`,
/// with every carried field already written.
pub(crate) unsafe fn lift_poll_into<'e, T: Send + Sync>(poll: GPoll<T>, dst: *mut u8, frame_bytes: usize, arena: &'e crate::arena::Arena) -> GPoll<RecordValue<'e>> {
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
pub fn register_deep_element_clone<T: dyn_any::StaticTypeSized>(
	clone_out: unsafe fn(*const u8) -> Box<dyn std::any::Any + Send + Sync>,
	repark: unsafe fn(&(dyn std::any::Any + Send + Sync), *mut u8, &crate::arena::Arena) -> Option<()>,
) {
	DEEP_ELEMENT_CLONES.lock().unwrap().insert(std::any::TypeId::of::<T::Static>(), DeepElementGlue { clone_out, repark });
}

fn deep_element_glue(type_id: std::any::TypeId) -> Option<DeepElementGlue> {
	DEEP_ELEMENT_CLONES.lock().unwrap().get(&type_id).copied()
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
pub fn element_write<T: Clone + Send + Sync + dyn_any::StaticTypeSized>() -> ElementWrite
where
	T::Static: Clone + Send + Sync,
{
	unsafe fn clone_out<T: Clone + Send + Sync + dyn_any::StaticTypeSized>(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync>
	where
		T::Static: Clone + Send + Sync,
	{
		if let Some(deep) = deep_element_glue(std::any::TypeId::of::<T::Static>()) {
			return unsafe { (deep.clone_out)(ptr) };
		}
		// SAFETY: a lifetime-carrying element type registers deep glue, so
		// this shallow path only erases borrow-free values.
		Box::new(unsafe { erase_static(read_element::<T>(Rec::new(ptr))) })
	}
	unsafe fn repark<T: Clone + Send + Sync + dyn_any::StaticTypeSized>(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &crate::arena::Arena) -> Option<()>
	where
		T::Static: Clone + Send + Sync,
	{
		if let Some(deep) = deep_element_glue(std::any::TypeId::of::<T::Static>()) {
			return unsafe { (deep.repark)(value, dst, arena) };
		}
		let retained = retained_measure(std::any::TypeId::of::<T::Static>()).map_or(0, |measure| measure(value));
		let value = value.downcast_ref::<T::Static>().expect("an element replays at its own type");
		unsafe { write_element_sized(dst, value.clone(), arena, retained) }
	}
	/// Declines for a type carrying deep glue, whose value may hold interiors
	/// the evaluation's arena owns and which a moved header may not reference.
	///
	/// # Safety
	/// `parked` must address a live payload of `T`.
	unsafe fn park_move<T: Clone + Send + Sync + dyn_any::StaticTypeSized>(parked: *const u8, promotion: &Promotion<'_>) -> Option<*const u8>
	where
		T::Static: Clone + Send + Sync,
	{
		deep_element_glue(std::any::TypeId::of::<T::Static>()).is_none().then_some(())?;
		// SAFETY: the caller's contract, at the type the park was written with.
		let value: &(dyn std::any::Any + Send + Sync) = unsafe { &*parked.cast::<T::Static>() };
		let retained = retained_measure(std::any::TypeId::of::<T::Static>()).map_or(0, |measure| measure(value));
		// SAFETY: as above, and the decline above establishes that the payload
		// owns all of its content.
		unsafe { promotion.move_park::<T::Static>(parked, retained) }.map(<*const T::Static>::cast)
	}
	let (size, align) = element_dims::<T>();
	ElementWrite {
		size,
		align,
		parked: element_parked::<T>(),
		type_id: std::any::TypeId::of::<T::Static>(),
		clone_out: clone_out::<T>,
		repark: repark::<T>,
		park_move: park_move::<T>,
		content_hash: None,
		content_eq: None,
	}
}

/// [`element_write`] plus the content hashing and equality glue, for element
/// types that support them.
pub fn element_write_hashed<T: Clone + Send + Sync + graphene_hash::CacheHash + PartialEq + dyn_any::StaticTypeSized>() -> ElementWrite
where
	T::Static: Clone + Send + Sync,
{
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

impl<T: Clone + Send + Sync + graphene_hash::CacheHash + PartialEq + dyn_any::StaticTypeSized> ElementWritePickHashed for ElementWritePick<T>
where
	T::Static: Clone + Send + Sync,
{
	fn element_write(&self) -> ElementWrite {
		element_write_hashed::<T>()
	}
}

pub trait ElementWritePickPlain {
	fn element_write(&self) -> ElementWrite;
}

impl<T: Clone + Send + Sync + dyn_any::StaticTypeSized> ElementWritePickPlain for &ElementWritePick<T>
where
	T::Static: Clone + Send + Sync,
{
	fn element_write(&self) -> ElementWrite {
		element_write::<T>()
	}
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
pub unsafe fn write_element<T: Send + Sync>(dst: *mut u8, value: T, arena: &crate::arena::Arena) -> Option<()> {
	unsafe { write_element_sized(dst, value, arena, 0) }
}

/// [`write_element`] with the park glue's estimate of the heap `value` owns.
///
/// # Safety
/// As [`write_element`].
pub unsafe fn write_element_sized<T: Send + Sync>(dst: *mut u8, value: T, arena: &crate::arena::Arena, retained: usize) -> Option<()> {
	match element_parked::<T>() {
		true => {
			let (parked, _) = arena.alloc_sized(value, retained)?;
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
pub unsafe fn apply_plan(src: Rec<'_>, dst: *mut u8, plan: &[(usize, usize, usize)]) {
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
		Some(SourcePlan { moves, fills, source: source.clone() })
	}

	/// # Safety
	/// `src` must be a record of this plan's source layout and `dst` a
	/// buffer of the plan's union layout. The returned view borrows `dst`, so
	/// `'d` must not outlive it.
	pub unsafe fn translate<'d>(&self, src: Rec<'_>, dst: *mut u8) -> Rec<'d> {
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
/// A translation lands in the claim the caller minted, so the value survives
/// sibling evaluations and its region is free again with that claim, which
/// bounds claims at one per source evaluation the kernel performs.
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

/// A node's own output frame, minted by its caller from the caller's frame
/// space: the one closing surface for every exit. Writes land through it,
/// [`Self::lift`] and [`Self::finish`] serve the record, and it carries the
/// free space beyond the frame, so a node's inputs claim past it and their
/// space is free again when the claim dies, on value, error, and pending exits
/// alike with no per-exit ritual.
pub struct FrameClaim<'e, 'l> {
	layout: &'l Layout,
	inline: RecordValue<'static>,
	frame: Option<*mut u8>,
	free: Frames<'e>,
}

impl<'e, 'l> FrameClaim<'e, 'l> {
	/// The free space beyond this claim's frame, which the node's own inputs
	/// claim from.
	pub fn frames(&mut self) -> &Frames<'e> {
		&mut self.free
	}

	fn dst(&mut self) -> *mut u8 {
		match self.frame {
			Some(frame) => frame,
			None => (&raw mut self.inline).cast(),
		}
	}

	/// Asserts the served element matches the wired layout, so a node whose
	/// layout never resolved (or resolved at another type) panics here
	/// instead of writing past its frame.
	fn check_element<T: dyn_any::StaticTypeSized>(&self) {
		let (size, _) = element_dims::<T>();
		assert!(
			self.layout.element.size == size && self.layout.element.parked == element_parked::<T>() && self.layout.element.type_id == std::any::TypeId::of::<T::Static>(),
			"the served element `{}` ({size} bytes) must match the wired layout ({} bytes)",
			std::any::type_name::<T>(),
			self.layout.element.size,
		);
	}

	/// Carries the plan's fields from a source record into the frame.
	///
	/// # Safety
	/// `src` must be a live record of the plan's source layout, and the plan
	/// must be the wiring-resolved plan of this frame's layout.
	pub unsafe fn carry(&mut self, src: Rec<'_>, plan: &[(usize, usize, usize)]) {
		unsafe { apply_plan(src, self.dst(), plan) };
	}

	/// Writes a field at its wiring-resolved offset.
	///
	/// # Safety
	/// `offset` must be this layout's resolved offset for a field of `T`.
	pub unsafe fn attr_at<T>(&mut self, offset: usize, value: T) {
		unsafe { write_field(self.dst(), offset, value) };
	}

	/// Writes the element; `None` reports arena exhaustion for a parked
	/// element. Panics where the element does not match the wired layout.
	pub fn element<T: Send + Sync + dyn_any::StaticTypeSized>(&mut self, value: T, arena: &crate::arena::Arena) -> Option<()> {
		self.check_element::<T>();
		// SAFETY: the frame is this layout's fresh claim and the element
		// check pinned `T` to the layout's element slot.
		unsafe { write_element(self.dst(), value, arena) }
	}

	/// Lifts a kernel's poll into the frame and closes it: the element
	/// writes on value polls, every poll keeps the frame claimed, and arena
	/// exhaustion of a parked element reports as an error poll. Panics where
	/// the element does not match the wired layout.
	pub fn lift<T: Send + Sync + dyn_any::StaticTypeSized>(mut self, poll: GPoll<T>, arena: &'e crate::arena::Arena) -> GPoll<RecordValue<'e>> {
		self.check_element::<T>();
		let frame_bytes = self.layout.frame_bytes();
		let dst = self.dst();
		// SAFETY: the frame is this layout's fresh claim, the element check
		// pinned `T`, and the drop keeps the frame contract on every poll.
		unsafe { lift_poll_into(poll, dst, frame_bytes, arena) }
	}

	/// Copies a complete record of this layout into the frame, for serving
	/// cached or published bytes.
	///
	/// # Safety
	/// `src` must point at a live record of this layout whose parked
	/// references outlive the serving evaluation.
	pub unsafe fn fill_copy(&mut self, src: *const u8) {
		unsafe { std::ptr::copy_nonoverlapping(src, self.dst(), self.layout.size) };
	}

	/// The served record. The frame stays claimed for the consumer; the drop
	/// releases only what was claimed above it.
	///
	/// # Safety
	/// The frame must hold a complete record of the layout, written through
	/// the carry, element, and field writes.
	pub unsafe fn finish(mut self) -> RecordValue<'e> {
		match self.frame {
			Some(frame) => RecordValue::spilled(unsafe { Rec::new(frame.cast_const()) }),
			// SAFETY: the inline record is the value's own bytes.
			None => unsafe { (&raw mut self.inline).cast::<RecordValue<'e>>().read() },
		}
	}

	/// [`Self::lift`] with the proof-bearing return for [`Node::serve`].
	pub fn lift_served<T: Send + Sync + dyn_any::StaticTypeSized>(self, poll: GPoll<T>, arena: &'e crate::arena::Arena) -> GPoll<Served<'e>> {
		self.lift(poll, arena).map(|value| Served { value })
	}

	/// [`Self::finish`] with the proof-bearing return for [`Node::serve`].
	///
	/// # Safety
	/// As [`Self::finish`].
	pub unsafe fn finish_served(self) -> Served<'e> {
		Served { value: unsafe { self.finish() } }
	}

	/// Fills the frame from a record a forwarded wire already served, and
	/// closes it: the source's frame sits above this claim and dies with its
	/// drop, so the served record is this claim's own.
	///
	/// # Safety
	/// `value` must be a live record of this frame's layout.
	pub unsafe fn forward(mut self, value: &RecordValue<'_>) -> Served<'e> {
		let src = self.layout.rec(value).ptr();
		unsafe {
			self.fill_copy(src);
			self.finish_served()
		}
	}

	/// Translates a source record into the frame through a wiring-resolved plan.
	///
	/// # Safety
	/// `src` must be a live record of `plan`'s source layout, and `plan` must
	/// translate into this frame's layout.
	pub unsafe fn translate(&mut self, src: Rec<'_>, plan: &SourcePlan) {
		unsafe { plan.translate(src, self.dst()) };
	}
}

/// The regions a promote dispatches on. A payload already living in the
/// persistent region outlives every entry promoted into it and is shared; a
/// payload in the transient arena or the frame buffer dies at the next reset
/// and is cloned.
///
/// The dispatch is decidable only where the reference addresses the payload
/// itself, which holds for parked elements and for a group interior's frames.
/// A reference-valued attribute instead names heap its arena-parked owner
/// holds, at an address in no arena's range, so those always clone.
///
/// THE SHARING LAW: sharing a payload between persistent entries is sound
/// because persistent invalidation is epochal, so every entry dies at one
/// flush and no entry can outlive a payload another still names. Per-entry
/// eviction would have to refcount the shared payloads before it could
/// reclaim one entry's storage.
#[derive(Clone, Copy)]
pub struct Promotion<'a> {
	transient: &'a crate::arena::Arena,
	frames: (usize, usize),
	persistent: &'a crate::arena::Arena,
}

impl<'a> Promotion<'a> {
	/// `frames` is the whole frame buffer as (address, bytes), from
	/// [`Frames::bounds`].
	pub fn new(transient: &'a crate::arena::Arena, frames: (usize, usize), persistent: &'a crate::arena::Arena) -> Self {
		Promotion { transient, frames, persistent }
	}

	pub fn persistent(&self) -> &'a crate::arena::Arena {
		self.persistent
	}

	/// Whether the reference dies with the evaluation, which is the promote's
	/// clone-or-share question. A null or byte-carried slot reads as neither
	/// region's and shares.
	pub fn evaluation_lived(&self, ptr: *const u8) -> bool {
		self.transient.contains(ptr) || (ptr as usize).wrapping_sub(self.frames.0) < self.frames.1
	}

	/// Moves a transient payload's header into the persistent region instead of
	/// cloning the heap it owns: the heap travels with the drop obligation and
	/// is freed at the persistent flush, never at the transient reset. `None`
	/// where the header is not the transient arena's own park or the region
	/// refused it, which leaves the caller its clone path.
	///
	/// The forwarding is the evaluation's, so a payload two records share moves
	/// once and both reach the one persistent header. The source header stays
	/// readable to the evaluation's remaining sharers, which are the only reads
	/// the move keeps sound: no read of it may outlive the persistent flush.
	///
	/// # Safety
	/// `parked` must address a live `T` the transient arena parked, and `T`
	/// must own all of its content, a persistent header being allowed to
	/// reference no transient storage.
	pub unsafe fn move_park<T: Send + Sync>(&self, parked: *const u8, retained: usize) -> Option<*const T> {
		// SAFETY: the caller's contract, forwarded to the parking arena.
		unsafe { self.transient.move_park::<T>(parked, self.persistent, retained) }
	}
}

/// Rewrites one promoted record's parked references in place: the lane bytes
/// are already the persistent region's, and each reference whose payload dies
/// with the evaluation is replaced by a clone parked there.
///
/// A parked element rides the arena slot its payload was written into, so the
/// region holding it decides share against clone. A parked field's reference
/// instead points into heap its arena-parked owner holds, which lies in no
/// arena range, so provenance is undecidable there and the field always
/// clones.
///
/// # Safety
/// `dst` must be a persistent image of a live record of `layout`.
unsafe fn promote_record(layout: &Layout, dst: *mut u8, promotion: &Promotion<'_>) -> Option<()> {
	if layout.element.parked {
		// SAFETY: a parked element slot holds one reference at offset 0.
		let parked = unsafe { dst.cast::<*const u8>().read() };
		if !promotion.persistent.contains(parked) {
			match element_promote_glue(layout.element.type_id) {
				// SAFETY: the slot images a parked element of this type.
				Some(promote) => unsafe { promote(dst.cast_const(), dst, promotion) }?,
				// SAFETY: as above; the header is the payload's own.
				None => match unsafe { (layout.element.park_move)(parked, promotion) } {
					// SAFETY: a parked element slot holds one reference at offset 0.
					Some(moved) => unsafe { dst.cast::<*const u8>().write(moved) },
					None => {
						// SAFETY: as above, and the clone owns its content.
						let owned = unsafe { (layout.element.clone_out)(dst.cast_const()) };
						unsafe { (layout.element.repark)(&*owned, dst, promotion.persistent) }?;
					}
				},
			}
		}
	}
	for field in &layout.fields {
		let Some(repark) = field.repark else { continue };
		// SAFETY: a parked field slot holds one reference at its offset.
		let slot = unsafe { dst.add(field.offset) };
		// SAFETY: the slot images a parked field of this descriptor.
		let value = deepen_field_value(unsafe { (field.read_erased)(slot.cast_const()) });
		match deep_field_glue(value.as_any().type_id()) {
			Some(glue) => match (glue.replay)(&*value, promotion.persistent)? {
				// SAFETY: the replay produced this field's own value type.
				Some(resident) => unsafe { repark(&*resident, slot, promotion.persistent) }?,
				None => unsafe { repark(&*value, slot, promotion.persistent) }?,
			},
			None => unsafe { repark(&*value, slot, promotion.persistent) }?,
		}
	}
	Some(())
}

/// Re-walks a promoted record and asserts no reference into the evaluation's
/// storage survived, which is the postcondition every later hit and every
/// shared interior relies on. The element's payload must sit in the persistent
/// region itself; a field's payload is heap its owner holds, so the weaker
/// range check is all that is decidable there.
///
/// # Safety
/// `ptr` must be a live record of `layout`.
pub unsafe fn assert_promoted(layout: &Layout, ptr: *const u8, promotion: &Promotion<'_>) {
	if layout.element.parked {
		// SAFETY: a parked element slot holds one reference at offset 0.
		let parked = unsafe { ptr.cast::<*const u8>().read() };
		assert!(promotion.persistent.contains(parked), "a promoted element kept a reference outside the persistent region");
	}
	for field in &layout.fields {
		if field.repark.is_none() {
			continue;
		}
		// SAFETY: a parked field slot holds one reference at its offset.
		let parked = unsafe { ptr.add(field.offset).cast::<*const u8>().read() };
		assert!(!promotion.evaluation_lived(parked), "a promoted field kept a reference into the evaluation");
	}
}

/// The promote override for element types whose payload holds arena-resident
/// interiors: the generic path clones through an owned intermediate, while a
/// registered promote shares the interiors already living in the persistent
/// region and copies only the rest.
type ElementPromote = unsafe fn(*const u8, *mut u8, &Promotion<'_>) -> Option<()>;

static ELEMENT_PROMOTES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, ElementPromote>>> = std::sync::LazyLock::new(Default::default);

/// Registers the promote for elements of `T`. Called at startup from the crate
/// that owns the type. The promote must leave no reference the promotion calls
/// evaluation-lived.
pub fn register_element_promote<T: dyn_any::StaticTypeSized>(promote: ElementPromote) {
	ELEMENT_PROMOTES.lock().unwrap().insert(std::any::TypeId::of::<T::Static>(), promote);
}

fn element_promote_glue(type_id: std::any::TypeId) -> Option<ElementPromote> {
	ELEMENT_PROMOTES.lock().unwrap().get(&type_id).copied()
}

/// The park glue's heap estimate for values of a type, keyed as the deep glue
/// is. Consulted where a payload parks, so a region's retained heap is known
/// without walking it.
type RetainedMeasure = fn(&(dyn std::any::Any + Send + Sync)) -> usize;

static RETAINED_MEASURES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::any::TypeId, RetainedMeasure>>> = std::sync::LazyLock::new(Default::default);

/// Registers the retained-heap estimate for values of `T`. Called at startup
/// from the crate that owns the type. The estimate is a hint: an unregistered
/// type contributes 0, so a region's counter is a lower bound.
pub fn register_retained_heap<T: dyn_any::StaticTypeSized>(measure: RetainedMeasure) {
	RETAINED_MEASURES.lock().unwrap().insert(std::any::TypeId::of::<T::Static>(), measure);
}

fn retained_measure(type_id: std::any::TypeId) -> Option<RetainedMeasure> {
	RETAINED_MEASURES.lock().unwrap().get(&type_id).copied()
}

/// A materialized run of lanes as the arena region its frames live in: the
/// handle keeps the provenance the region was allocated with and carries the
/// generation, so resolving it re-checks liveness where an address would have
/// been trusted. The layout stays with the holder, which proved it at wiring.
#[derive(Clone, Copy)]
pub struct MaterializedSpan {
	base: crate::arena::ArenaWeak<u8>,
	len: usize,
}

impl std::fmt::Debug for MaterializedSpan {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("MaterializedSpan").field("len", &self.len).finish()
	}
}

impl MaterializedSpan {
	/// `None` where the batch's frames are not this arena's, which is the
	/// caller's cue to re-materialize rather than cache.
	pub fn of(batch: &crate::node::RecordBatch<'_>, arena: &crate::arena::Arena) -> Option<MaterializedSpan> {
		match batch.len() {
			0 => Some(MaterializedSpan {
				base: crate::arena::ArenaWeak::NULL,
				len: 0,
			}),
			len => Some(MaterializedSpan {
				base: arena.handle_at(batch.get(0).rec().ptr())?,
				len,
			}),
		}
	}

	/// Copies the batch into the persistent region as a copy-on-write over
	/// provenance: the frame bytes memcpy, and each parked reference is cloned
	/// only where it dies with the evaluation, so a layout carrying no parked
	/// slot reduces to the memcpy and a level whose payloads an upstream memo
	/// already published costs nothing beyond it. `None` where the region
	/// could not hold the copy, which leaves the caller with nothing to cache.
	///
	/// # Safety
	/// The batch's lanes must be live records of its layout.
	pub unsafe fn to_persistent(batch: &crate::node::RecordBatch<'_>, promotion: &Promotion<'_>) -> Option<MaterializedSpan> {
		let layout = batch.layout();
		let stride = layout.lane_stride();
		let len = batch.len();
		if len == 0 {
			return Some(MaterializedSpan {
				base: crate::arena::ArenaWeak::NULL,
				len: 0,
			});
		}
		let persistent = promotion.persistent();
		let slab = persistent.alloc_scratch::<u64>((len * stride).div_ceil(8))?;
		let base: *mut u8 = slab.as_mut_ptr().cast();
		for lane in 0..len {
			// SAFETY: the caller's contract on the lane, into the lane's own
			// region of the freshly reserved slab.
			let dst = unsafe { base.add(lane * stride) };
			unsafe { std::ptr::copy_nonoverlapping(batch.get(lane).rec().ptr(), dst, layout.size) };
			// SAFETY: the copy images a record of this layout.
			unsafe { promote_record(layout, dst, promotion) }?;
		}
		#[cfg(debug_assertions)]
		for lane in 0..len {
			// SAFETY: every lane was imaged and promoted above.
			unsafe { assert_promoted(layout, base.add(lane * stride).cast_const(), promotion) };
		}
		Some(MaterializedSpan {
			base: persistent.handle_at(base.cast_const())?,
			len,
		})
	}

	/// The span's lanes at `layout`, or `None` once the generation moved on.
	pub fn batch<'a>(&self, arena: &'a crate::arena::Arena, layout: &'a Layout) -> Option<crate::node::RecordBatch<'a>> {
		let base: *const u8 = match self.len {
			0 => std::ptr::NonNull::<u8>::dangling().as_ptr(),
			_ => self.base.upgrade(arena)?,
		};
		// SAFETY: the handle resolved in generation, so the region still holds
		// the lanes it was published with, packed at the layout's stride; an
		// empty span reads no lane.
		Some(unsafe { crate::node::RecordBatch::new(base, self.len, layout) })
	}

	/// Lane `lane`'s record, or `None` past the span or once the generation
	/// moved on.
	pub fn lane(&self, arena: &crate::arena::Arena, lane: usize, layout: &Layout) -> Option<*const u8> {
		(lane < self.len).then_some(())?;
		let base: *const u8 = self.base.upgrade(arena)?;
		// SAFETY: in-bounds by the length check, at the layout the span was
		// published under.
		Some(unsafe { base.add(lane * layout.lane_stride()) })
	}
}

/// The proof a record was served through a frame claim: mintable only by the
/// claim's closing methods, so holding one means the record is of the
/// claimed layout.
pub struct Served<'e> {
	value: RecordValue<'e>,
}

impl<'e> Served<'e> {
	pub fn value(self) -> RecordValue<'e> {
		self.value
	}

	/// The served record in place, for producers that read it before passing
	/// the proof on.
	pub fn record(&self) -> &RecordValue<'e> {
		&self.value
	}
}

/// Claims `node`'s own frame from `frames` and serves through it: the
/// caller-side half of [`Node::serve`], for drivers that want the record
/// rather than the proof.
pub fn serve_edge<'e, C, N>(node: &N, input: &C, frames: &Frames<'e>) -> GPoll<RecordValue<'e>>
where
	N: Node<C> + ?Sized,
	C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
{
	let slot = frames.claim(node.layout());
	node.serve(input, slot).map(Served::value)
}

/// A claim shortens onto a derived context's arena lifetime.
#[cfg(test)]
fn claim_shortens<'long: 'short, 'short, 'l>(claim: FrameClaim<'long, 'l>) -> FrameClaim<'short, 'l> {
	claim
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
	serve_edge(node, ctx, &scope).map(|value| ServedRecord {
		// SAFETY: the poll served `value` at the node's declared layout and
		// nothing has claimed frames since.
		record: unsafe { OwnedRecord::copy_out(&layout, layout.rec(&value)) },
		layout: layout.clone(),
	})
}

/// Law-test scaffolding: a kernel closure served onto an element-only record
/// wire (the element lands at offset 0, parked when it carries drop glue). No
/// production path constructs one; value edges are
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

impl<El: Clone + 'static, N> RecordExtract<El, N> {
	/// The edge's element, copied out of its record.
	pub fn eval<'e, C>(&self, input: &C, frames: &Frames<'e>) -> GPoll<El>
	where
		N: Node<C>,
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		// The element copies out by value, so the edge's claim dies with
		// the scope.
		let scope = frames.scope();
		serve_edge(&self.edge, input, &scope).map(|value| unsafe { read_element::<El>(self.layout.rec(&value)) })
	}
}

impl<C, N> Node<C> for RecordSource<N>
where
	N: Node<C>,
{
	fn serve<'e, 'l>(&self, input: &C, mut slot: FrameClaim<'e, 'l>) -> GPoll<Served<'e>>
	where
		C: crate::context::ExtractArena<ArenaRef = &'e crate::arena::Arena>,
	{
		// The source's frame is claimed beyond this one and dies with the
		// claim; the translated union record stays.
		let Some(plan) = &self.plan else {
			return self.edge.serve(input, slot);
		};
		match serve_edge(&self.edge, input, &mut slot.frames().reborrow()) {
			GPoll::Final(value) => {
				// SAFETY: the value came from this edge, so it carries the
				// plan's source layout.
				unsafe { slot.translate(plan.source.rec(&value), plan) };
				// SAFETY: the translation completes the union record.
				GPoll::Final(unsafe { slot.finish_served() })
			}
			GPoll::Partial(value) => {
				// SAFETY: as for the final arm.
				unsafe { slot.translate(plan.source.rec(&value), plan) };
				// SAFETY: as for the final arm.
				GPoll::Partial(unsafe { slot.finish_served() })
			}
			GPoll::Fallback(boxed) => {
				let (value, error) = *boxed;
				// SAFETY: as for the final arm.
				unsafe { slot.translate(plan.source.rec(&value), plan) };
				// SAFETY: as for the final arm.
				GPoll::Fallback(Box::new((unsafe { slot.finish_served() }, error)))
			}
			GPoll::Pending => GPoll::Pending,
			GPoll::Error(error) => GPoll::Error(error),
		}
	}

	fn extent_at<'x>(&self, input: &C, level: u8, frames: &Frames<'x>) -> GPoll<crate::gpoll::Extent>
	where
		C: crate::context::ExtractArena<ArenaRef = &'x crate::arena::Arena>,
	{
		self.edge.extent_at(input, level, frames)
	}

	fn layout(&self) -> &Layout {
		&self.union
	}
}

/// Builds a resident run lane by lane: fresh frames in the arena at a layout
/// derived from the element glue and field writes, elements pushed in order
/// and attributes written onto pushed lanes. The finished item's frames are
/// arena-resident, valid for the evaluation like every parked payload.
pub struct RunBuilder<'e> {
	arena: &'e crate::arena::Arena,
	layout: Layout,
	frames: *mut u8,
	len: usize,
	pushed: usize,
}

impl<'e> RunBuilder<'e> {
	/// Fresh frames for `len` lanes of a layout over `element` and `fields`.
	/// `None` reports arena exhaustion.
	pub fn new(arena: &'e crate::arena::Arena, element: ElementWrite, fields: &[FieldWrite], len: usize) -> Option<Self> {
		let layout = Layout::default().with_writes(0, element, fields);
		assert!(!layout.element.parked || layout.element.content_hash.is_some(), "a parked element adopts only with content glue");
		for field in &layout.fields {
			assert!(field.repark.is_none() || field.content_hash.is_some(), "a parked field adopts only with content glue");
		}
		let stride = layout.lane_stride();
		let scratch = arena.alloc_scratch::<u64>((len * stride).div_ceil(8))?;
		Some(Self {
			arena,
			layout,
			frames: scratch.as_mut_ptr().cast(),
			len,
			pushed: 0,
		})
	}

	/// Starts the next lane: moves its element in and default-fills its
	/// fields. Returns the lane index; `None` reports arena exhaustion.
	pub fn push<T: Send + Sync + dyn_any::StaticTypeSized>(&mut self, element: T) -> Option<usize> {
		assert_eq!(
			std::any::TypeId::of::<T::Static>(),
			self.layout.element.type_id,
			"the pushed element must match the layout's element type"
		);
		assert!(self.pushed < self.len, "the builder holds exactly its declared lane count");
		let lane = self.pushed;
		let stride = self.layout.lane_stride();
		// SAFETY: the frames hold `len` lanes at the layout's stride, and
		// `lane` is below `len`; the element slot and each field's region are
		// disjoint parts of this lane.
		let base = unsafe { self.frames.add(lane * stride) };
		unsafe { write_element(base, element, self.arena) }?;
		for field in &self.layout.fields {
			// SAFETY: as above; the field region is within the lane.
			let bytes = unsafe { std::slice::from_raw_parts_mut(base.add(field.offset), field.size) };
			bytes.fill(0);
			if let Some(info) = crate::attribute::info(field.name)
				&& info.size == field.size
			{
				(info.write_default_bytes)(bytes);
			}
		}
		self.pushed = lane + 1;
		Some(lane)
	}

	/// Writes the marker's value on an already pushed lane. The layout must
	/// carry the marker among its field writes.
	pub fn attr<A: crate::attribute::Attribute>(&mut self, lane: usize, value: A::Value<'e>)
	where
		A::Value<'static>: 'static,
	{
		assert!(lane < self.pushed, "attributes write onto pushed lanes");
		let field = self
			.layout
			.fields
			.iter()
			.find(|field| field.name == A::NAME && field.level == 0)
			.expect("the layout carries the written marker");
		let offset = field.offset;
		assert_eq!(field.type_id, std::any::TypeId::of::<A::Value<'static>>(), "the field was declared at the marker's value type");
		// SAFETY: the offset comes from the builder's own layout and the value
		// type matches the field's declared type.
		unsafe { self.frames.add(lane * self.layout.lane_stride() + offset).cast::<A::Value<'e>>().write(value) };
	}

	/// Writes a legacy stored value on an already pushed lane through the
	/// census glue, parking droppable payloads. A marker outside the layout's
	/// fields is dropped; a wrong-typed stored value leaves the field's
	/// default. `None` reports arena exhaustion.
	pub fn attr_stored(&mut self, lane: usize, info: &crate::attribute::AttributeInfo, value: &dyn crate::list::AnyAttributeValue) -> Option<()> {
		assert!(lane < self.pushed, "attributes write onto pushed lanes");
		let Some(offset) = self.layout.offset_of(info.name, 0) else { return Some(()) };
		// SAFETY: the offset comes from the builder's own layout, and the
		// census writer verifies the stored type before touching the field.
		unsafe { (info.write_stored)(value, self.frames.add(lane * self.layout.lane_stride() + offset), self.arena) }
	}

	/// The finished run. Panics unless every lane was pushed, since an
	/// unwritten parked element slot must never become readable.
	pub fn finish(self) -> GroupItem<'e> {
		assert_eq!(self.pushed, self.len, "every lane pushes before the run finishes");
		GroupItem {
			layout: self.layout,
			storage: ItemStorage::Resident(self.frames.cast_const()),
			len: self.len,
			_arena: std::marker::PhantomData,
		}
	}
}

/// `len` records stored in the arena at `layout`'s stride. The layout is
/// owned by the value and identifies the run's element type. A resident item
/// borrows the serving arena at `'e`, so it cannot outlive the evaluation. An
/// owned item ([`Self::copy_out`]) is the `'static` form: it survives the
/// generation, refuses lane reads, and re-enters a serving arena through
/// [`Self::replay`]. Safe code cannot mint a resident `GroupItem<'static>`,
/// so the lifetime instantiation is the resident/owned distinction.
#[derive(Clone, Debug)]
pub struct GroupItem<'e> {
	layout: Layout,
	storage: ItemStorage,
	len: usize,
	_arena: std::marker::PhantomData<&'e ()>,
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
unsafe impl Send for GroupItem<'_> {}
// SAFETY: as `Send`.
unsafe impl Sync for GroupItem<'_> {}

impl<'e> GroupItem<'e> {
	/// Copies the batch's lanes into the arena and clones its layout.
	/// Returns `None` when the arena is exhausted. Parked regions must carry
	/// the content glue, so equality and hashing never fall back to pointer
	/// bytes.
	pub fn adopt(batch: crate::node::RecordBatch<'_>, arena: &'e crate::arena::Arena) -> Option<Self> {
		let layout = batch.layout().clone();
		assert!(!layout.element.parked || layout.element.content_hash.is_some(), "a parked element adopts only with content glue");
		for field in &layout.fields {
			assert!(field.repark.is_none() || field.content_hash.is_some(), "a parked field adopts only with content glue");
		}
		let stride = layout.lane_stride();
		// A run already living in the target region needs no copy: the arena is
		// insert-only within a generation, so the lanes cannot move or change
		// while the adopting item borrows them.
		if batch.len() > 0 && arena.contains(batch.frames_ptr()) {
			return Some(Self {
				layout,
				storage: ItemStorage::Resident(batch.frames_ptr()),
				len: batch.len(),
				_arena: std::marker::PhantomData,
			});
		}
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
			_arena: std::marker::PhantomData,
		})
	}

	/// A resident run built from a legacy list: one lane per item, the element
	/// moved in and every census-declared attribute written through its stored
	/// form. An undeclared key has no field form and is dropped; a wrong-typed
	/// stored value leaves its field's default, matching the legacy read.
	/// `None` reports arena exhaustion.
	pub fn from_list<T: Clone + Send + Sync + graphene_hash::CacheHash + PartialEq + dyn_any::StaticTypeSized>(list: crate::list::List<T>, arena: &'e crate::arena::Arena) -> Option<GroupItem<'e>>
	where
		T::Static: Clone + Send + Sync,
	{
		let declared: Vec<crate::attribute::AttributeInfo> = list.attribute_keys().filter_map(crate::attribute::info).collect();
		let writes: Vec<FieldWrite> = declared.iter().map(|info| (info.field_write_at)(0)).collect();
		let mut builder = RunBuilder::new(arena, element_write_hashed::<T>(), &writes, list.len())?;
		for item in list.into_iter() {
			let (element, attributes) = item.into_parts();
			let lane = builder.push(element)?;
			for (key, value) in attributes.iter() {
				let Some(info) = declared.iter().find(|info| info.name == key) else { continue };
				builder.attr_stored(lane, info, value)?;
			}
		}
		Some(builder.finish())
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
	pub unsafe fn from_resident(batch: crate::node::RecordBatch<'e>) -> Self {
		let layout = batch.layout().clone();
		assert!(!layout.element.parked || layout.element.content_hash.is_some(), "a parked element adopts only with content glue");
		for field in &layout.fields {
			assert!(field.repark.is_none() || field.content_hash.is_some(), "a parked field adopts only with content glue");
		}
		Self {
			storage: ItemStorage::Resident(batch.frames_ptr()),
			len: batch.len(),
			layout,
			_arena: std::marker::PhantomData,
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
	pub fn copy_out(&self) -> GroupItem<'static> {
		if let ItemStorage::Owned(_) = &self.storage {
			return GroupItem {
				layout: self.layout.clone(),
				storage: self.storage.clone(),
				len: self.len,
				_arena: std::marker::PhantomData,
			};
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
			_arena: std::marker::PhantomData,
		}
	}

	/// Re-parks an owned item's lanes into `arena`, restoring the resident
	/// form; `None` reports arena exhaustion. A resident item returns a plain
	/// clone.
	pub fn replay<'a>(&self, arena: &'a crate::arena::Arena) -> Option<GroupItem<'a>> {
		let ItemStorage::Owned(owned) = &self.storage else {
			// A resident item re-serves at the target arena's own lifetime,
			// so its lanes copy rather than relabeling the borrow.
			return GroupItem::adopt(self.lanes(), arena);
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
			_arena: std::marker::PhantomData,
		})
	}

	/// The run promoted into the persistent region: a run already living there
	/// is shared, since its own promote left it free of evaluation-lived
	/// references, and one that dies with the evaluation is copied lane by lane
	/// with the same dispatch applied to its parked references. This is what
	/// keeps a promote proportional to newly produced data.
	pub fn to_persistent<'p>(&self, promotion: &Promotion<'p>) -> Option<GroupItem<'p>> {
		let ItemStorage::Resident(frames) = self.storage else {
			return self.replay(promotion.persistent());
		};
		let persistent = promotion.persistent();
		if self.len == 0 || persistent.contains(frames) {
			return Some(GroupItem {
				layout: self.layout.clone(),
				storage: ItemStorage::Resident(frames),
				len: self.len,
				_arena: std::marker::PhantomData,
			});
		}
		let stride = self.layout.lane_stride();
		let scratch = persistent.alloc_scratch::<u64>((self.len * stride).div_ceil(8))?;
		let dst = scratch.as_mut_ptr().cast::<u8>();
		// SAFETY: the source holds `len` lanes of `layout` at its stride and the
		// scratch was reserved for exactly that.
		unsafe { std::ptr::copy_nonoverlapping(frames, dst, self.len * stride) };
		for lane in 0..self.len {
			// SAFETY: each lane images a record of this layout.
			unsafe { promote_record(&self.layout, dst.add(lane * stride), promotion) }?;
		}
		Some(GroupItem {
			layout: self.layout.clone(),
			storage: ItemStorage::Resident(dst.cast_const()),
			len: self.len,
			_arena: std::marker::PhantomData,
		})
	}

	/// A typed view over the stored records, checked against the layout's
	/// element type.
	pub fn typed_lanes<T: dyn_any::StaticTypeSized>(&self) -> Option<crate::node::List<'_, T>> {
		match self.layout.element.type_id == std::any::TypeId::of::<T::Static>() {
			// SAFETY: the layout records the element type the lanes hold.
			true => Some(unsafe { crate::node::List::new(self.lanes()) }),
			false => None,
		}
	}
}

/// A run read at its element type. Record fields hold each marker's value
/// verbatim, so lane reads are plain typed reads at a hoisted offset.
pub struct RunView<'a, T> {
	item: &'a GroupItem<'a>,
	lanes: crate::node::List<'a, T>,
}

impl<'a, T: dyn_any::StaticTypeSized> RunView<'a, T> {
	/// `None` where the run holds another element type.
	pub fn new(item: &'a GroupItem<'a>) -> Option<Self> {
		item.typed_lanes::<T>().map(|lanes| Self { item, lanes })
	}
}

/// A marker's field on a run, its offset resolved once.
pub struct RunColumn<'a, A: crate::attribute::Attribute> {
	item: &'a GroupItem<'a>,
	field: Option<FieldOffset<A>>,
}

impl<'a, A: crate::attribute::Attribute> RunColumn<'a, A> {
	/// The marker's column on an item, its offset resolved once.
	pub fn of(item: &'a GroupItem<'a>) -> Self {
		Self {
			item,
			field: FieldOffset::of(item.layout(), 0),
		}
	}
}

impl<'a, A: crate::attribute::Attribute> crate::lane::LaneColumn<'a, A> for RunColumn<'a, A> {
	fn try_get(&self, lane: usize) -> Option<A::Value<'a>> {
		self.item.lanes().get(lane).try_attr_at(self.field?)
	}
}

impl<'a, T: dyn_any::StaticTypeSized> crate::lane::LaneSource for RunView<'a, T> {
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
		RunColumn::of(self.item)
	}
}

impl<T: crate::render_complexity::RenderComplexity + dyn_any::StaticTypeSized> crate::render_complexity::RenderComplexity for RunView<'_, T> {
	fn render_complexity(&self) -> usize {
		use crate::lane::LaneSource;
		(0..self.lane_count())
			.filter_map(|lane| self.element(lane))
			.map(crate::render_complexity::RenderComplexity::render_complexity)
			.sum()
	}
}

impl<T: crate::bounds::BoundingBox + dyn_any::StaticTypeSized> crate::bounds::BoundingBox for RunView<'_, T> {
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
pub struct Group<'e> {
	pub row: Option<GroupItem<'e>>,
	pub content: GroupItem<'e>,
}

impl<'e> Group<'e> {
	/// The group deep-copied out of its evaluation, every run in owned form.
	pub fn copy_out(&self) -> Group<'static> {
		Group {
			row: self.row.as_ref().map(GroupItem::copy_out),
			content: self.content.copy_out(),
		}
	}

	/// The group promoted into the persistent region, each run shared or copied
	/// by [`GroupItem::to_persistent`].
	pub fn to_persistent<'p>(&self, promotion: &Promotion<'p>) -> Option<Group<'p>> {
		let row = match &self.row {
			Some(row) => Some(row.to_persistent(promotion)?),
			None => None,
		};
		Some(Group {
			row,
			content: self.content.to_persistent(promotion)?,
		})
	}

	/// Re-parks an owned group's runs into `arena`; `None` reports arena
	/// exhaustion.
	pub fn replay<'a>(&self, arena: &'a crate::arena::Arena) -> Option<Group<'a>> {
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

impl PartialEq for GroupItem<'_> {
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

impl graphene_hash::CacheHash for GroupItem<'_> {
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

impl PartialEq for Group<'_> {
	fn eq(&self, other: &Self) -> bool {
		self.row == other.row && self.content == other.content
	}
}

impl graphene_hash::CacheHash for Group<'_> {
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
			type_id: std::any::TypeId::of::<()>(),
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

		let arena = crate::arena::Arena::new(4096).unwrap();
		let transform = DAffine2::from_translation((5., 6.).into());
		let mut builder = RunBuilder::new(&arena, element_write::<f64>(), &[FieldWrite::of::<Transform>(0), FieldWrite::of::<Opacity>(0)], 2).unwrap();
		for lane in 0..2 {
			let lane = builder.push(lane as f64).unwrap();
			builder.attr::<Transform>(lane, transform);
			builder.attr::<Opacity>(lane, 0.25);
		}
		let item = builder.finish();
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

		let arena = crate::arena::Arena::new(1024).unwrap();
		let mut builder = RunBuilder::new(&arena, element_write::<f64>(), &[], 1).unwrap();
		builder.push(0f64).unwrap();
		let item = builder.finish();
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
	#[should_panic(expected = "two different types")]
	fn type_conflicts_panic() {
		let mut conflicting = sized_field("opacity", 8, 8);
		conflicting.type_id = std::any::TypeId::of::<u64>();
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		layout.with_writes(0, element_write::<f64>(), &[conflicting]);
	}

	#[test]
	fn a_token_resolves_only_in_the_layout_it_was_minted_against() {
		use crate::attribute::{Attribute, Opacity, Transform};

		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[FieldWrite::of::<Opacity>(0)]);
		let token = FieldOffset::<Opacity>::of(&layout, 0).expect("the layout carries the marker");
		assert_eq!(token.resolve(&layout), layout.offset_of(Opacity::NAME, 0));

		let shifted = Layout::default().with_writes(0, element_write::<f64>(), &[FieldWrite::of::<Transform>(0), FieldWrite::of::<Opacity>(0)]);
		assert_eq!(FieldOffset::<Opacity>::of(&shifted, 0).and_then(|token| token.resolve(&layout)), None);
		assert!(FieldOffset::<Opacity>::of(&Layout::default(), 0).is_none());
	}

	#[test]
	fn union_is_order_independent() {
		let a = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		let b = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("length")]);
		assert_eq!(Layout::union(&[&a, &b]), Layout::union(&[&b, &a]));
		assert!(Layout::union(&[&a, &b]).offset_of("length", 0).is_some());
	}

	#[test]
	fn a_pod_level_promotes_as_a_bare_memcpy() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(1, element_write::<f64>(), &[f64_field("opacity")]);
		let stride = layout.lane_stride();

		let mut buffer = vec![0u64; (4 * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		for lane in 0..4usize {
			unsafe { base.add(lane * stride).cast::<f64>().write(lane as f64) };
		}
		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 4, &layout) };

		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let slab = (4 * stride).div_ceil(8) * 8;
		assert_eq!(persistent.occupancy(), slab, "a layout with no parked slot allocates the slab and nothing else");
		assert_eq!(persistent.retained_heap(), 0, "no payload parked, so nothing is retained");
		let published = span.batch(&persistent, &layout).unwrap();
		for lane in 0..4usize {
			assert_eq!(unsafe { published.get(lane).rec().element::<f64>() }, lane as f64, "lane {lane}");
		}
	}

	#[test]
	fn a_promoted_element_lands_in_the_persistent_region() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		unsafe { write_element(base, String::from("parked in the evaluation"), &transient) }.unwrap();
		let source = unsafe { base.cast::<*const u8>().read() };
		assert!(transient.contains(source));

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 1, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let promoted = unsafe { published.get(0).rec().ptr().cast::<*const u8>().read() };
		assert!(persistent.contains(promoted), "the promote re-parked the payload into the persistent region");
		assert_ne!(source, promoted, "an evaluation-lived payload gets its own persistent header");
		assert_eq!(unsafe { borrow_element::<String>(published.get(0).rec()) }, "parked in the evaluation");
	}

	/// The measure production registers for `String`, without which a promote
	/// credits the region 0 and the counters cannot be observed to transfer.
	fn measure_strings() {
		register_retained_heap::<String>(|value| value.downcast_ref::<String>().map_or(0, String::len));
	}

	#[test]
	fn a_promoted_payload_moves_its_heap_rather_than_cloning_it() {
		measure_strings();
		let mut transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		let owned = String::from("the obligation travels with the header");
		let (heap, length) = (owned.as_ptr(), owned.len());
		unsafe { write_element_sized(base, owned, &transient, length) }.unwrap();
		assert_eq!(transient.retained_heap(), length);

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 1, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let served = unsafe { borrow_element::<String>(published.get(0).rec()) };
		assert_eq!(served.as_ptr(), heap, "the promote moved the header, so the served view names the pre-promote heap");
		assert_eq!(transient.retained_heap(), 0, "the transient counter gave the hint up");
		assert_eq!(persistent.retained_heap(), length, "and the persistent counter took it");

		transient.reset();
		let published = span.batch(&persistent, &layout).unwrap();
		assert_eq!(
			unsafe { borrow_element::<String>(published.get(0).rec()) },
			"the obligation travels with the header",
			"the moved payload survives the transient reset"
		);
	}

	#[test]
	fn a_payload_two_records_share_promotes_to_one_persistent_header() {
		measure_strings();
		let transient = crate::arena::Arena::new(4096).unwrap();
		let mut persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);
		let stride = layout.lane_stride();

		let mut buffer = vec![0u64; (2 * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		let length = "shared across lanes".len();
		unsafe { write_element_sized(base, String::from("shared across lanes"), &transient, length) }.unwrap();
		// A carried field byte-copies the reference, so both lanes name the one park.
		let shared = unsafe { base.cast::<*const u8>().read() };
		unsafe { base.add(stride).cast::<*const u8>().write(shared) };

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 2, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let first = unsafe { published.get(0).rec().ptr().cast::<*const u8>().read() };
		let second = unsafe { published.get(1).rec().ptr().cast::<*const u8>().read() };
		assert_eq!(first, second, "a payload two records share moves once");
		assert_eq!(persistent.retained_heap(), length, "and its hint transfers once");

		persistent.reset();
		assert_eq!(persistent.retained_heap(), 0, "the flush frees the one header exactly once");
	}

	#[test]
	fn a_persistent_element_is_shared_by_the_promote() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		// The payload an upstream memo already published: the promote must
		// name it rather than copy it.
		unsafe { write_element(base, String::from("published upstream"), &persistent) }.unwrap();
		let upstream = unsafe { base.cast::<*const u8>().read() };
		let occupied = persistent.occupancy();

		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 1, &layout) };
		let promotion = Promotion::new(&transient, bounds, &persistent);
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.unwrap();

		let published = span.batch(&persistent, &layout).unwrap();
		let promoted = unsafe { published.get(0).rec().ptr().cast::<*const u8>().read() };
		assert_eq!(upstream, promoted, "an already persistent payload is shared, pointer for pointer");
		assert_eq!(persistent.occupancy() - occupied, layout.frame_bytes(), "only the lane slab was allocated");
	}

	#[test]
	#[should_panic(expected = "a promoted element kept a reference outside the persistent region")]
	fn the_rewalk_catches_a_reference_the_promote_left_behind() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<String>(), &[]);

		let mut buffer = vec![0u64; layout.frame_bytes().div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		unsafe { write_element(base, String::from("never promoted"), &transient) }.unwrap();

		let promotion = Promotion::new(&transient, bounds, &persistent);
		unsafe { assert_promoted(&layout, base.cast_const(), &promotion) };
	}

	#[test]
	fn an_interior_run_already_persistent_is_shared() {
		let transient = crate::arena::Arena::new(4096).unwrap();
		let persistent = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(1, element_write::<f64>(), &[f64_field("opacity")]);
		let stride = layout.lane_stride();

		let mut buffer = vec![0u64; (2 * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		for lane in 0..2usize {
			unsafe { base.add(lane * stride).cast::<f64>().write(lane as f64) };
		}
		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 2, &layout) };

		let published = GroupItem::adopt(batch, &persistent).unwrap();
		let occupied = persistent.occupancy();
		let promotion = Promotion::new(&transient, (base as usize, buffer.len() * 8), &persistent);
		let shared = published.to_persistent(&promotion).unwrap();

		assert_eq!(persistent.occupancy(), occupied, "a run the region already holds costs no allocation");
		assert!(std::ptr::eq(published.lanes().frames_ptr(), shared.lanes().frames_ptr()), "the interior is shared, not copied");
	}

	#[test]
	fn adopting_within_one_region_shares_the_run() {
		let arena = crate::arena::Arena::new(4096).unwrap();
		let layout = Layout::default().with_writes(1, element_write::<f64>(), &[f64_field("opacity")]);
		let stride = layout.lane_stride();

		let mut buffer = vec![0u64; (2 * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		for lane in 0..2usize {
			unsafe { base.add(lane * stride).cast::<f64>().write(lane as f64) };
		}
		let batch = unsafe { crate::node::RecordBatch::new(base.cast_const(), 2, &layout) };

		let resident = GroupItem::adopt(batch, &arena).unwrap();
		let occupied = arena.occupancy();
		let again = GroupItem::adopt(resident.lanes(), &arena).unwrap();
		assert_eq!(arena.occupancy(), occupied, "re-adopting a run the arena already holds copies nothing");
		assert!(std::ptr::eq(resident.lanes().frames_ptr(), again.lanes().frames_ptr()));
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

		#[derive(Clone, dyn_any::DynAny)]
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
	fn a_run_builds_from_a_legacy_list_and_serves_its_rows() {
		use crate::lane::LaneSource;

		let mut list = crate::list::List::new_from_element(String::from("row 0"));
		list.push(crate::list::Item::new_from_element(String::from("row 1")));
		let transform = glam::DAffine2::from_translation(glam::DVec2::new(3., 4.));
		list.set_attribute(crate::ATTR_TRANSFORM, 0, transform);
		list.set_attribute("name", 0, String::from("first"));
		list.set_attribute(crate::ATTR_EDITOR_LAYER_PATH, 1, vec![crate::uuid::NodeId(7), crate::uuid::NodeId(9)]);
		list.set_attribute("max_width", 1, Some(12.5f64));

		let arena = crate::arena::Arena::new(1 << 16).unwrap();
		let item = GroupItem::from_list(list, &arena).unwrap();
		assert_eq!(item.len(), 2);
		let run = RunView::<String>::new(&item).expect("the run holds string elements");

		assert_eq!(run.element(0).map(String::as_str), Some("row 0"));
		assert_eq!(run.attr::<crate::attribute::Transform>(0), transform);
		assert_eq!(run.attr::<crate::attribute::Name>(0), "first");
		assert!(run.attr::<crate::attribute::EditorLayerPath>(0).is_empty());

		assert_eq!(run.element(1).map(String::as_str), Some("row 1"));
		// A lane without the value reads the census default, not garbage.
		assert_eq!(run.attr::<crate::attribute::Transform>(1), glam::DAffine2::IDENTITY);
		assert_eq!(run.attr::<crate::attribute::Name>(1), "");
		assert_eq!(run.attr::<crate::attribute::EditorLayerPath>(1), &[crate::uuid::NodeId(7), crate::uuid::NodeId(9)]);
		assert_eq!(run.attr::<crate::attribute::MaxWidth>(1), Some(12.5));
	}

	#[test]
	fn a_built_run_replays_after_the_source_dies() {
		use crate::lane::LaneSource;

		let owned = {
			let arena = crate::arena::Arena::new(1 << 16).unwrap();
			let mut list = crate::list::List::new_from_element(String::from("element"));
			list.set_attribute("name", 0, String::from("label"));
			list.set_attribute(crate::ATTR_EDITOR_LAYER_PATH, 0, vec![crate::uuid::NodeId(3)]);
			GroupItem::from_list(list, &arena).unwrap().copy_out()
		};

		let arena = crate::arena::Arena::new(1 << 16).unwrap();
		let replayed = owned.replay(&arena).unwrap();
		let run = RunView::<String>::new(&replayed).expect("the run holds string elements");
		assert_eq!(run.element(0).map(String::as_str), Some("element"));
		assert_eq!(run.attr::<crate::attribute::Name>(0), "label");
		assert_eq!(run.attr::<crate::attribute::EditorLayerPath>(0), &[crate::uuid::NodeId(3)]);
	}

	#[test]
	fn a_wrong_typed_or_undeclared_column_leaves_the_default() {
		use crate::lane::LaneSource;

		let mut list = crate::list::List::new_from_element(String::from("element"));
		list.set_attribute(crate::ATTR_OPACITY, 0, String::from("not an f64"));
		list.set_attribute("never_declared", 0, 5u32);

		let arena = crate::arena::Arena::new(1 << 16).unwrap();
		let item = GroupItem::from_list(list, &arena).unwrap();
		assert!(item.layout().offset_of("never_declared", 0).is_none(), "an undeclared key has no field form");
		let run = RunView::<String>::new(&item).expect("the run holds string elements");
		assert_eq!(run.attr::<crate::attribute::Opacity>(0), 1., "the wrong-typed value reads as absent");
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

	#[test]
	#[should_panic(expected = "must match the wired layout")]
	fn a_mistyped_element_is_rejected_at_the_write() {
		let arena = crate::arena::Arena::new(256).unwrap();
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[]);
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(1 << 10);
		let frames = frame_arena.frames();
		frames.claim(&layout).element(1u32, &arena);
	}

	#[test]
	fn a_scope_releases_and_reuses_its_frames() {
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[f64_field("opacity")]);
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(1 << 10);
		let frames = frame_arena.frames();
		let free = frames.free_words();
		let mut addresses = Vec::new();
		for _ in 0..3 {
			let scope = frames.scope();
			let mut claim = scope.claim(&layout);
			addresses.push(claim.dst() as usize);
			drop(claim);
			drop(scope);
			assert_eq!(frames.free_words(), free, "the scope returns its claims");
		}
		assert!(addresses.windows(2).all(|pair| pair[0] == pair[1]), "each claim reuses the same region");
	}

	#[test]
	fn a_claim_shortens_onto_a_derived_lifetime() {
		fn shorten<'long: 'short, 'short, 'l>(claim: FrameClaim<'long, 'l>) -> FrameClaim<'short, 'l> {
			claim_shortens(claim)
		}
		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[]);
		let mut frame_arena = FrameArena::new();
		frame_arena.reserve(64);
		let frames = frame_arena.frames();
		let claim = shorten(frames.claim(&layout));
		assert_eq!(claim.layout, &layout);
	}
}
