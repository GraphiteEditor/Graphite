//! The packed-record tier at rank 0. A record is the element at offset 0
//! plus one field per written attribute; its [`Layout`] is computed at
//! wiring from the upstream write set and never serialized. Records live as
//! per-lane views in a per-worker [`Frame`] whose slots are assigned at
//! wiring, and kernels route them as opaque [`RecordValue`]s that carry
//! their provenance. Only generated or wiring code touches offsets, so a
//! safe kernel cannot misalign a field.

use crate::attribute;
use crate::gpoll::GPoll;
use crate::node::Node;
use std::cell::UnsafeCell;

/// One field of a [`Layout`]: a (name, level) key resolved to an offset.
/// Levels are numbered innermost-out; only level 0 exists at rank 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldDesc {
	pub name: &'static str,
	pub level: u8,
	pub offset: usize,
	pub size: usize,
	pub align: usize,
}

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
	pub fn with_writes(&self, depth: u8, element: (usize, usize), writes: &[(&'static str, u8, usize, usize)]) -> Layout {
		let mut merged: Vec<(&'static str, u8, usize, usize)> = self.fields.iter().map(|field| (field.name, field.level, field.size, field.align)).collect();
		for &(name, level, size, align) in writes {
			match merged.iter().find(|(n, l, ..)| *n == name && *l == level) {
				Some(&(.., existing_size, _)) => assert_eq!(existing_size, size, "attribute `{name}` written at two different sizes"),
				None => merged.push((name, level, size, align)),
			}
		}
		merged.sort_by(|a, b| b.3.cmp(&a.3).then(b.2.cmp(&a.2)).then(a.0.cmp(b.0)).then(a.1.cmp(&b.1)));
		let (element_size, element_align) = element;
		let mut offset = element_size;
		let mut align = element_align.max(1);
		let fields = merged
			.into_iter()
			.map(|(name, level, size, field_align)| {
				offset = offset.next_multiple_of(field_align.max(1));
				align = align.max(field_align);
				let desc = FieldDesc {
					name,
					level,
					offset,
					size,
					align: field_align,
				};
				offset += size;
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
			let writes: Vec<(&'static str, u8, usize, usize)> = layout.fields.iter().map(|field| (field.name, field.level, field.size, field.align)).collect();
			union = union.with_writes(union.depth, (union.element_size, union.element_align), &writes);
		}
		union
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

/// Assigns frame slots at wiring: a bump allocator over slot sizes, aligned
/// to at most 8 (record layouts never exceed word alignment).
#[derive(Debug, Default)]
pub struct FrameLayout {
	size: usize,
}

impl FrameLayout {
	pub fn slot(&mut self, layout: &Layout) -> usize {
		assert!(layout.align <= 8, "record layouts align to at most 8");
		self.size = self.size.next_multiple_of(layout.align.max(1));
		let offset = self.size;
		self.size += layout.size;
		offset
	}

	pub fn size(&self) -> usize {
		self.size
	}
}

/// The per-worker record frame: every record-producing slot lives at a
/// wiring-assigned offset. Slots are overwritten per lane and never touch
/// the arena.
pub struct Frame {
	words: Box<[UnsafeCell<u64>]>,
}

// SAFETY: a frame belongs to one worker; slot writes happen only inside that
// worker's evaluation, and wiring assigns disjoint offsets per slot.
unsafe impl Send for Frame {}
unsafe impl Sync for Frame {}

impl Frame {
	pub fn new(size: usize) -> Self {
		Self {
			words: (0..size.div_ceil(8).max(1)).map(|_| UnsafeCell::new(0)).collect(),
		}
	}

	/// # Safety
	/// `offset` must be a slot offset assigned by [`FrameLayout`] for this
	/// frame, and the caller must be the slot's owning node evaluation.
	pub unsafe fn slot(&self, offset: usize) -> *mut u8 {
		debug_assert!(offset <= self.words.len() * 8);
		unsafe { self.words.as_ptr().cast::<u8>().cast_mut().add(offset) }
	}
}

impl std::fmt::Debug for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Frame").field("bytes", &(self.words.len() * 8)).finish()
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
		&& info.packable
		&& info.size == size
	{
		(info.write_default_bytes)(&mut bytes);
	}
	bytes
}

/// A routing source's wiring-resolved translation: field moves into the
/// consumer's frame buffer plus census default fill for union fields the
/// source lacks. Absent when the source's layout already equals the union,
/// in which case the record pointer forwards untouched.
#[derive(Debug)]
pub struct SourcePlan {
	moves: Vec<(usize, usize, usize)>,
	fills: Vec<(usize, Box<[u8]>)>,
	slot: usize,
}

impl SourcePlan {
	pub fn new(source: &Layout, union: &Layout, slot: usize) -> Option<SourcePlan> {
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
		Some(SourcePlan { moves, fills, slot })
	}

	/// # Safety
	/// `src` must be a record of this plan's source layout and `frame` the
	/// frame whose slot was assigned to this plan at wiring.
	pub unsafe fn translate(&self, src: Rec, frame: &Frame) -> Rec {
		unsafe {
			let dst = frame.slot(self.slot);
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
pub struct RecordSource<N> {
	edge: N,
	plan: Option<SourcePlan>,
}

impl<N> RecordSource<N> {
	pub fn wire(edge: N, source: &Layout, union: &Layout, slot: usize) -> Self {
		Self {
			edge,
			plan: SourcePlan::new(source, union, slot),
		}
	}
}

impl<'e, C, N> Node<C> for RecordSource<N>
where
	C: crate::context::ExtractFrame<'e>,
	N: Node<C, Output = RecordValue<'e>>,
{
	type Output = RecordValue<'e>;

	fn eval(&self, input: &C) -> GPoll<RecordValue<'e>> {
		let value = self.edge.eval(input);
		match &self.plan {
			None => value,
			Some(plan) => {
				let Some(frame) = crate::context::ExtractFrame::frame(input) else {
					return GPoll::error("record frame missing");
				};
				value.map(|value| RecordValue::from_rec(unsafe { plan.translate(value.rec(), frame) }))
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn f64_field(name: &'static str) -> (&'static str, u8, usize, usize) {
		(name, 0, 8, 8)
	}

	#[test]
	fn canonical_order_and_offsets() {
		let layout = Layout::default().with_writes(0, (8, 8), &[("tint", 0, 4, 4), f64_field("opacity"), ("flag", 0, 1, 1)]);
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
		layout.with_writes(0, (8, 8), &[("opacity", 0, 4, 4)]);
	}

	#[test]
	fn union_is_order_independent() {
		let a = Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")]);
		let b = Layout::default().with_writes(0, (8, 8), &[f64_field("length")]);
		assert_eq!(Layout::union(&[&a, &b]), Layout::union(&[&b, &a]));
		assert!(Layout::union(&[&a, &b]).offset_of("length", 0).is_some());
	}

	#[test]
	fn frame_slots_bump_aligned() {
		let flag = Layout::default().with_writes(0, (1, 1), &[]);
		let wide = Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")]);
		let mut frame = FrameLayout::default();
		assert_eq!(frame.slot(&flag), 0);
		assert_eq!(frame.slot(&wide), 8);
		assert_eq!(frame.size(), 24);
	}

	#[test]
	fn translation_moves_fields_and_fills_census_defaults() {
		let source = Layout::default().with_writes(0, (8, 8), &[f64_field("length")]);
		let union = Layout::union(&[&source, &Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")])]);
		let mut frame_layout = FrameLayout::default();
		let slot = frame_layout.slot(&union);
		let frame = Frame::new(frame_layout.size());

		let plan = SourcePlan::new(&source, &union, slot).unwrap();
		let record = [5f64, 7f64];
		let translated = unsafe { plan.translate(Rec::new(record.as_ptr().cast()), &frame) };
		assert_eq!(unsafe { translated.element::<f64>() }, 5.);
		assert_eq!(unsafe { translated.read::<f64>(union.offset_of("length", 0).unwrap()) }, 7.);
		assert_eq!(unsafe { translated.read::<f64>(union.offset_of("opacity", 0).unwrap()) }, 1.);
	}

	#[test]
	fn identity_layouts_forward() {
		let layout = Layout::default().with_writes(0, (8, 8), &[f64_field("opacity")]);
		assert!(SourcePlan::new(&layout, &layout.clone(), 0).is_none());
	}
}
