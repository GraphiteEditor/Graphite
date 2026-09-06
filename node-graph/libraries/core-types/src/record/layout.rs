//! Wiring-time shape facts: the layout a record takes and the writes it folds from.

use super::access::{Rec, RecordValue, borrow_element, erase_static, read_element, write_element_keyed};
use super::owned::deep_element_glue;
use super::promote::{Promotion, retained_measure};
use crate::attribute;

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

/// Declarative record-io metadata for a node type, emitted by the macro into
/// its registry entry so the compiler can fold each input's layout without
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
	/// consumes the whole subject input, so only the node's own levels remain.
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
			// A fold consumes the whole subject input (a deeper input folds its
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
			Some(&source) if self.level_delta >= 0 => {
				let from = inputs[source as usize].expect("layout resolve source input has no layout");
				let carry_element = matches!(self.element, ElementSpec::Carried);
				let removes: Vec<(&str, u8)> = self.removes.clone();
				copy_plan(from, &layout, carry_element, &removes)
			}
			// A reducer collapses its carrier's levels, so it writes a fresh record rather than copying fields down.
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

/// Whether elements of `T` move once into the arena and ride as references:
/// records byte-copy their contents and never run drop glue, so a type is
/// byte-carried exactly when it has none.
pub const fn element_parked<T>() -> bool {
	std::mem::needs_drop::<T>()
}

/// The element (size, align) a record input of `T` carries.
pub fn element_dims<T>() -> (usize, usize) {
	match element_parked::<T>() {
		true => (size_of::<*const u8>(), align_of::<*const u8>()),
		false => (size_of::<T>(), align_of::<T>()),
	}
}

/// The element slot a record input of `T` carries, its erased glue bound at
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
		// SAFETY: the caller's contract, keyed as this element's own parks are.
		unsafe { write_element_keyed(dst, value.clone(), arena, retained, std::any::TypeId::of::<T::Static>()) }
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
		unsafe { promotion.move_park::<T>(parked, retained) }.map(<*const T::Static>::cast)
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::record::test_support::{f64_field, sized_field};

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
	fn elements_ride_as_bytes_exactly_without_drop_glue() {
		assert!(!element_parked::<f64>());
		assert!(!element_parked::<[f64; 4]>());
		assert!(element_parked::<String>());
		assert!(element_parked::<std::sync::Arc<str>>());
		assert_eq!(element_dims::<[f64; 4]>(), (32, 8));
		assert_eq!(element_dims::<String>(), (8, 8));
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
}
