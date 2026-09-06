//! Runs and groups: many records of one layout, resident or owned.

use super::access::write_element;
use super::layout::{ElementWrite, FieldOffset, FieldWrite, Layout, element_write_hashed};
use super::owned::deep_field_glue;
use super::promote::{Promotion, promote_record};

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

// SAFETY: as for `RecordValue`.
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
		if !batch.is_empty() && arena.contains(batch.frames_ptr()) {
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
	/// form; `None` reports arena exhaustion. A resident item re-adopts into
	/// `arena`, sharing only where its lanes already live there.
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
/// it `None`, because that lane's record carries the attributes.
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
			// SAFETY: the constructors store `len` lanes of `layout` at the layout's stride.
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
	use crate::record::access::{read_element, write_field};
	use crate::record::layout::element_write;
	use crate::record::test_support::f64_field;

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
}
