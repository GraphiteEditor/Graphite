use super::Graphic;
use core_types::Color;
use core_types::list::{Item, List};
use raster_types::{CPU, Raster};
use vector_types::Vector;

/// The graphic with every `Group` deep-copied to its owned form, which
/// survives the arena generation but cannot be read until
/// [`map_groups_to_resident`] re-parks it into a serving arena.
pub fn map_groups_to_owned<'out>(graphic: &Graphic<'_>) -> Graphic<'out> {
	match graphic {
		Graphic::Group(group) => Graphic::Group(group.copy_out()),
		Graphic::Graphic(children) => {
			let mut out = List::new();
			for item in children.clone().into_iter() {
				let (element, attributes) = item.into_parts();
				out.push(Item::from_parts(map_groups_to_owned(&element), attributes));
			}
			Graphic::Graphic(out)
		}
		Graphic::Vector(vector) => Graphic::Vector(vector.clone()),
		Graphic::RasterCPU(raster) => Graphic::RasterCPU(raster.clone()),
		Graphic::RasterGPU(raster) => Graphic::RasterGPU(raster.clone()),
		Graphic::Color(color) => Graphic::Color(*color),
		Graphic::Gradient(gradient) => Graphic::Gradient(gradient.clone()),
		Graphic::Text(text) => Graphic::Text(text.clone()),
	}
}

/// The graphic with every owned `Group` re-parked into `arena`; `None`
/// reports arena exhaustion.
pub fn map_groups_to_resident<'a>(graphic: &Graphic<'a>, arena: &'a core_types::arena::Arena) -> Option<Graphic<'a>> {
	match graphic {
		Graphic::Group(group) => group.replay(arena).map(Graphic::Group),
		Graphic::Graphic(children) => {
			let mut children = children.clone();
			for child in children.iter_element_values_mut() {
				*child = map_groups_to_resident(child, arena)?;
			}
			Some(Graphic::Graphic(children))
		}
		other => Some(other.clone()),
	}
}

/// The deep copy-out for `Graphic` elements: a plain clone of a group
/// interior would carry frame pointers into the evaluation's arena, so memo
/// and capture seams copy out the owned-group form.
///
/// # Safety
/// `ptr` must point at a live parked `Graphic` element field.
unsafe fn deep_clone_graphic(ptr: *const u8) -> Box<dyn std::any::Any + Send + Sync> {
	let graphic = unsafe { core_types::record::borrow_element::<Graphic>(core_types::record::Rec::new(ptr)) };
	Box::new(map_groups_to_owned(graphic))
}

/// The deep re-park for `Graphic` elements: owned groups replay into the
/// serving arena before the graphic parks.
///
/// # Safety
/// `value` must hold a `Graphic` and `dst` must be a live `Graphic` element
/// field.
unsafe fn deep_repark_graphic(value: &(dyn std::any::Any + Send + Sync), dst: *mut u8, arena: &core_types::arena::Arena) -> Option<()> {
	let graphic = value.downcast_ref::<Graphic>().expect("an element replays at its own type");
	let resident = map_groups_to_resident(graphic, arena)?;
	let retained = graphic_retained_heap(&resident);
	unsafe { core_types::record::write_element_sized(dst, resident, arena, retained) }
}

/// The graphic with every `Group` promoted into the persistent region: an
/// interior already living there is shared rather than copied, so the cost is
/// what this level newly produced. `None` reports arena exhaustion.
pub fn map_groups_to_persistent<'p>(graphic: &Graphic<'_>, promotion: &core_types::record::Promotion<'p>) -> Option<Graphic<'p>> {
	match graphic {
		Graphic::Group(group) => group.to_persistent(promotion).map(Graphic::Group),
		Graphic::Graphic(children) => {
			let mut out = List::new();
			for item in children.clone().into_iter() {
				let (element, attributes) = item.into_parts();
				out.push(Item::from_parts(map_groups_to_persistent(&element, promotion)?, attributes));
			}
			Some(Graphic::Graphic(out))
		}
		Graphic::Vector(vector) => Some(Graphic::Vector(vector.clone())),
		Graphic::RasterCPU(raster) => Some(Graphic::RasterCPU(raster.clone())),
		Graphic::RasterGPU(raster) => Some(Graphic::RasterGPU(raster.clone())),
		Graphic::Color(color) => Some(Graphic::Color(*color)),
		Graphic::Gradient(gradient) => Some(Graphic::Gradient(gradient.clone())),
		Graphic::Text(text) => Some(Graphic::Text(text.clone())),
	}
}

/// The promote for `Graphic` elements: the generic path would deep-copy every
/// interior through an owned intermediate, while this shares the interiors the
/// persistent region already holds. A group-free graphic references nothing the
/// evaluation owns, so its header moves and its heap is never copied.
///
/// # Safety
/// `src` must point at a live parked `Graphic` element field, and `dst` at the
/// element field the promoted reference is written to.
unsafe fn promote_graphic(src: *const u8, dst: *mut u8, promotion: &core_types::record::Promotion<'_>) -> Option<()> {
	let graphic = unsafe { core_types::record::borrow_element::<Graphic>(core_types::record::Rec::new(src)) };
	if !graphic_contains_groups(graphic) {
		// SAFETY: a parked element slot holds one reference at offset 0, and a
		// group-free graphic owns all of its content.
		let header = unsafe { src.cast::<*const u8>().read() };
		if let Some(moved) = unsafe { promotion.move_park::<Graphic<'static>>(header, graphic_retained_heap(graphic)) } {
			// SAFETY: as above, into the promoted image's own element slot.
			unsafe { dst.cast::<*const Graphic<'static>>().write(moved) };
			return Some(());
		}
	}
	let promoted = map_groups_to_persistent(graphic, promotion)?;
	let retained = graphic_retained_heap(&promoted);
	unsafe { core_types::record::write_element_sized(dst, promoted, promotion.persistent(), retained) }
}

/// The heap a graphic's own payload owns. Group interiors are excluded: their
/// lanes park through this same glue and are counted as they land.
fn graphic_retained_heap(graphic: &Graphic<'_>) -> usize {
	match graphic {
		Graphic::Vector(vector) => vector_retained_heap(vector),
		Graphic::RasterCPU(raster) => raster.data.len() * size_of::<Color>(),
		Graphic::Text(text) => text.len(),
		Graphic::Gradient(gradient) => gradient.len() * size_of::<(f64, Color)>(),
		Graphic::Graphic(children) => (0..children.len()).filter_map(|index| children.element(index)).map(graphic_retained_heap).sum(),
		Graphic::Group(_) | Graphic::RasterGPU(_) | Graphic::Color(_) => 0,
	}
}

/// The heap a vector's domain columns own, summed over the columns it
/// exposes, so the segment domain's private parallel columns are undercounted.
fn vector_retained_heap(vector: &Vector) -> usize {
	size_of_val(vector.point_domain.ids())
		+ size_of_val(vector.point_domain.positions())
		+ size_of_val(vector.segment_domain.ids())
		+ size_of_val(vector.region_domain.ids())
		+ size_of_val(vector.colinear_manipulators.as_slice())
}

fn graphic_contains_groups(graphic: &Graphic) -> bool {
	match graphic {
		Graphic::Group(_) => true,
		Graphic::Graphic(children) => list_contains_groups(children),
		_ => false,
	}
}

pub(crate) fn list_contains_groups(list: &List<Graphic>) -> bool {
	(0..list.len()).any(|index| list.element(index).is_some_and(graphic_contains_groups))
}

/// The heap a graphic list's elements own, group interiors excluded as
/// [`graphic_retained_heap`] excludes them.
fn list_retained_heap(list: &List<Graphic>) -> usize {
	(0..list.len()).filter_map(|index| list.element(index)).map(graphic_retained_heap).sum()
}

/// The deep copy-out for graphic-list field values (the paint markers' owned
/// form): content groups leave in their owned form. Declines (`None`) for
/// group-free content, which already owns everything.
fn deep_clone_graphic_list(value: &dyn core_types::list::AnyAttributeValue) -> Option<Box<dyn core_types::list::AnyAttributeValue>> {
	let list = value.as_any().downcast_ref::<Option<List<Graphic>>>().expect("a graphic list field deep-copies at its own type");
	let list = list.as_ref().filter(|list| list_contains_groups(list))?;
	let mut list = list.clone();
	for element in list.iter_element_values_mut() {
		*element = map_groups_to_owned(element);
	}
	Some(Box::new(Some(list)))
}

/// The deep replay for graphic-list field values: owned content groups replay
/// into the serving arena before the field re-parks. `Some(None)` declines
/// for group-free content; `None` reports arena exhaustion.
fn deep_repark_graphic_list(value: &dyn core_types::list::AnyAttributeValue, arena: &core_types::arena::Arena) -> Option<Option<Box<dyn core_types::list::AnyAttributeValue>>> {
	let list = value.as_any().downcast_ref::<Option<List<Graphic>>>().expect("a graphic list field replays at its own type");
	let Some(list) = list.as_ref().filter(|list| list_contains_groups(list)) else {
		return Some(None);
	};
	let mut list = list.clone();
	for element in list.iter_element_values_mut() {
		*element = map_groups_to_resident(element, arena)?;
	}
	let list = unsafe { core_types::record::erase_static(list) };
	Some(Some(Box::new(Some(list))))
}

/// The promote for graphic-list fields, the deep field glue's third half: the
/// owned halves copy content groups out to the owned form and replay them back,
/// while this maps the content transient-to-persistent in one pass.
///
/// THE TWO-LEVEL SHARING LAW: the field's own header is not provenance-shared.
/// It moves where the content is group-free, since the payload then owns all of
/// its content and the transient arena confirms the reference is its own park,
/// and otherwise it clones into a fresh persistent park. One level inside, a
/// content group's interior is arena-resident and its provenance is decidable,
/// so it takes [`map_groups_to_persistent`]'s Cow dispatch: an interior the
/// persistent region already holds is shared pointer for pointer.
///
/// # Safety
/// `src` must point at a live parked graphic-list field, and `dst` at the field
/// slot the promoted reference is written to.
unsafe fn promote_graphic_list(src: *const u8, dst: *mut u8, promotion: &core_types::record::Promotion<'_>) -> Option<()> {
	// SAFETY: the caller's contract; the slot holds one optional reference.
	let Some(list) = (unsafe { src.cast::<Option<&List<Graphic<'static>>>>().read() }) else {
		// SAFETY: as above, into the promoted image's own field slot.
		unsafe { dst.cast::<Option<&List<Graphic<'static>>>>().write(None) };
		return Some(());
	};
	let retained = list_retained_heap(list);
	if !list_contains_groups(list) {
		// SAFETY: a group-free list owns all of its content, and the arena
		// declines a reference that is not a park at the list's own address
		// and size.
		if let Some(moved) = unsafe { promotion.move_park::<List<Graphic<'static>>>(std::ptr::from_ref(list).cast(), retained) } {
			// SAFETY: the move published a live list in the persistent region.
			unsafe { dst.cast::<Option<&List<Graphic<'static>>>>().write(Some(&*moved)) };
			return Some(());
		}
	}
	let mut promoted = list.clone();
	for element in promoted.iter_element_values_mut() {
		*element = map_groups_to_persistent(element, promotion)?;
	}
	// SAFETY: every borrow the clone carried was replaced by persistent content
	// above, so the erased form outlives the evaluation.
	let promoted = unsafe { core_types::record::erase_static(promoted) };
	let (parked, _) = promotion.persistent().alloc_sized(promoted, retained)?;
	// SAFETY: the slot holds one optional reference.
	unsafe { dst.cast::<Option<&List<Graphic<'static>>>>().write(Some(parked)) };
	Some(())
}

const _: () = {
	fn register_all() {
		core_types::record::register_deep_element_clone::<Graphic>(deep_clone_graphic, deep_repark_graphic);
		core_types::record::register_deep_field_value::<Option<List<Graphic>>>(deep_clone_graphic_list, deep_repark_graphic_list);
		core_types::record::register_field_promote::<Option<&'static List<Graphic<'static>>>>(promote_graphic_list);
		core_types::record::register_element_promote::<Graphic>(promote_graphic);
		core_types::record::register_retained_heap::<Graphic>(|value| value.downcast_ref::<Graphic>().map_or(0, graphic_retained_heap));
		core_types::record::register_retained_heap::<Vector>(|value| value.downcast_ref::<Vector>().map_or(0, vector_retained_heap));
		core_types::record::register_retained_heap::<Raster<CPU>>(|value| value.downcast_ref::<Raster<CPU>>().map_or(0, |raster| raster.data.len() * size_of::<Color>()));
		core_types::record::register_retained_heap::<String>(|value| value.downcast_ref::<String>().map_or(0, String::len));
	}

	#[cfg(not(target_family = "wasm"))]
	#[core_types::ctor::ctor]
	fn register() {
		register_all();
	}

	#[cfg(target_family = "wasm")]
	#[unsafe(export_name = "__node_registry_deep_element_graphic")]
	extern "C" fn register() {
		register_all();
	}
};

#[cfg(test)]
mod run_tests {
	use super::*;
	use crate::graphic::test_support::{native_group_paint, unit_square_at};
	use crate::graphic::{group_to_legacy_list, map_groups_to_legacy};
	use crate::markers::Fill;
	use core_types::attribute::Attribute;
	use core_types::lane::LaneSource;
	use core_types::record::{FieldWrite, RunBuilder, RunView, element_write_hashed};
	use glam::DVec2;

	#[test]
	fn an_owned_group_replays_content_equal_after_the_source_dies() {
		let paint = List::new_from_element(Graphic::Color(Color::BLACK));
		let vector = unit_square_at(DVec2::ZERO);

		let source = core_types::arena::Arena::new(1 << 16).unwrap();
		let mut builder = RunBuilder::new(&source, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)], 1).unwrap();
		let lane = builder.push(vector.clone()).unwrap();
		builder.attr::<Fill>(lane, Some(&paint));
		let group = core_types::record::Group { row: None, content: builder.finish() };
		let expected = group_to_legacy_list(&group);
		let owned = map_groups_to_owned(&Graphic::Group(group));
		drop(source);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let resident = map_groups_to_resident(&owned, &arena).expect("the arena holds the replay");
		let Graphic::Group(group) = &resident else { panic!("the replay keeps the group form") };
		assert_eq!(group_to_legacy_list(group), expected);
	}

	#[test]
	fn an_owned_group_replays_nested_groups_through_the_element_glue() {
		let vector = unit_square_at(DVec2::ZERO);
		let source = core_types::arena::Arena::new(1 << 16).unwrap();
		let mut builder = RunBuilder::new(&source, element_write_hashed::<Vector>(), &[], 1).unwrap();
		builder.push(vector.clone()).unwrap();
		let nested = Graphic::Group(core_types::record::Group { row: None, content: builder.finish() });

		let mut builder = RunBuilder::new(&source, element_write_hashed::<Graphic>(), &[], 1).unwrap();
		builder.push(nested).unwrap();
		let group = core_types::record::Group { row: None, content: builder.finish() };
		let expected = group_to_legacy_list(&group);
		let owned = map_groups_to_owned(&Graphic::Group(group));
		drop(source);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let resident = map_groups_to_resident(&owned, &arena).expect("the arena holds the replay");
		let Graphic::Group(group) = &resident else { panic!("the replay keeps the group form") };
		assert_eq!(group_to_legacy_list(group), expected);
	}

	#[test]
	fn an_owned_run_deep_copies_graphic_list_fields() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let source = core_types::arena::Arena::new(1 << 16).unwrap();
		// SAFETY: the erased native list serves only while `source` is live; the
		// deep glue under test replaces its borrows at the copy-out seam.
		let paint = unsafe { core_types::record::erase_static(native_group_paint(&inner_vector, &source)) };

		let vector = unit_square_at(DVec2::new(4., 4.));
		let mut builder = RunBuilder::new(&source, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)], 1).unwrap();
		let lane = builder.push(vector.clone()).unwrap();
		builder.attr::<Fill>(lane, Some(&paint));
		let item = builder.finish();
		let owned = item.copy_out();
		let expected = map_groups_to_legacy(paint.element(0).unwrap());
		drop(item);
		drop(paint);
		drop(source);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let replayed = owned.replay(&arena).expect("the arena holds the replay");
		let run = RunView::<Vector>::new(&replayed).expect("the run holds vector elements");
		let served = run.attr::<Fill>(0).expect("the fill replays present");
		assert_eq!(map_groups_to_legacy(served.element(0).unwrap()), expected);
	}

	#[test]
	fn an_owned_record_deep_copies_graphic_list_fields() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let source = core_types::arena::Arena::new(1 << 16).unwrap();
		// SAFETY: the erased native list serves only while `source` is live; the
		// deep glue under test replaces its borrows at the copy-out seam.
		let paint = unsafe { core_types::record::erase_static(native_group_paint(&inner_vector, &source)) };

		let vector = unit_square_at(DVec2::new(4., 4.));
		let mut builder = RunBuilder::new(&source, element_write_hashed::<Vector>(), &[FieldWrite::of::<Fill>(0)], 1).unwrap();
		let lane = builder.push(vector.clone()).unwrap();
		builder.attr::<Fill>(lane, Some(&paint));
		let item = builder.finish();
		let layout = item.layout().clone();
		let offset = layout.offset_of(Fill::NAME, 0).unwrap();
		// SAFETY: the item's lane is a live record of `layout`.
		let owned = unsafe { core_types::record::OwnedRecord::copy_out(&layout, item.lanes().get(0).rec()) };
		let expected = map_groups_to_legacy(paint.element(0).unwrap());
		drop(item);
		drop(paint);
		drop(source);

		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let frames = core_types::record::test_frames(layout.frame_bytes());
		let mut slot = frames.claim(&layout);
		owned.replay_into(&mut slot, &arena).expect("the arena holds the replay");
		// SAFETY: the replay completes the record in the claimed frame.
		let value = unsafe { slot.finish() };
		// SAFETY: the replay wrote a record of `layout`.
		let served = unsafe { layout.rec(&value).read::<Option<&List<Graphic>>>(offset) }.expect("the fill replays present");
		assert_eq!(map_groups_to_legacy(served.element(0).unwrap()), expected);
	}

	/// A `lanes`-long frame promoted out of `transient` into `persistent`, with
	/// `fill` written into the paint field of every lane. The frame buffer comes
	/// back so it outlives the promote's reads.
	fn promote_paint_field(
		fill: Option<&List<Graphic<'static>>>,
		lanes: usize,
		transient: &core_types::arena::Arena,
		persistent: &core_types::arena::Arena,
	) -> (core_types::record::Layout, core_types::record::MaterializedSpan, Vec<u64>) {
		use core_types::record::{Layout, MaterializedSpan, Promotion, element_write, write_field};

		let layout = Layout::default().with_writes(0, element_write::<f64>(), &[FieldWrite::of::<Fill>(0)]);
		let offset = layout.offset_of(Fill::NAME, 0).unwrap();
		let stride = layout.lane_stride();
		let mut buffer = vec![0u64; (lanes * stride).div_ceil(8)];
		let base = buffer.as_mut_ptr().cast::<u8>();
		let bounds = (base as usize, buffer.len() * 8);
		for lane in 0..lanes {
			// SAFETY: the frame is this layout's, written at the element slot and
			// at the paint field's own offset.
			unsafe {
				base.add(lane * stride).cast::<f64>().write(lane as f64);
				write_field::<Option<&List<Graphic<'static>>>>(base.add(lane * stride), offset, fill);
			}
		}
		// SAFETY: the frames hold `lanes` live records of `layout`.
		let batch = unsafe { core_types::node::RecordBatch::new(base.cast_const(), lanes, &layout) };
		let promotion = Promotion::new(transient, bounds, persistent);
		// SAFETY: as above.
		let span = unsafe { MaterializedSpan::to_persistent(&batch, &promotion) }.expect("the region holds the promote");
		(layout, span, buffer)
	}

	/// The promoted paint of one lane, at the layout the promote published.
	fn promoted_paint<'p>(
		span: &core_types::record::MaterializedSpan,
		layout: &core_types::record::Layout,
		lane: usize,
		persistent: &'p core_types::arena::Arena,
	) -> &'p List<Graphic<'p>> {
		let offset = layout.offset_of(Fill::NAME, 0).unwrap();
		let batch = span.batch(persistent, layout).expect("the span resolves in its own region");
		// SAFETY: the promote wrote a record of `layout` into every lane.
		unsafe { batch.get(lane).rec().read::<Option<&List<Graphic>>>(offset) }.expect("the paint promotes present")
	}

	#[test]
	fn a_promoted_paint_field_shares_persistent_interiors() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let transient = core_types::arena::Arena::new(1 << 16).unwrap();
		let persistent = core_types::arena::Arena::new(1 << 16).unwrap();

		// The interior an upstream promote already published, named by a paint
		// list the evaluation parked.
		let published = native_group_paint(&inner_vector, &persistent);
		let interior = {
			let Some(Graphic::Group(group)) = published.element(0) else { panic!("the paint carries a native group") };
			group.content.lanes().get(0).rec().ptr()
		};
		// SAFETY: the list serves only while `persistent` is live, and the
		// promote under test replaces every borrow it carries.
		let (paint, _) = transient.alloc_sized_keyed(unsafe { core_types::record::erase_static(published) }, 0).unwrap();

		let occupied = persistent.occupancy();
		let (layout, span, _frames) = promote_paint_field(Some(paint), 1, &transient, &persistent);
		let served = promoted_paint(&span, &layout, 0, &persistent);

		let Some(Graphic::Group(group)) = served.element(0) else { panic!("the promote keeps the group form") };
		assert_eq!(group.content.lanes().get(0).rec().ptr(), interior, "a persistent interior is shared pointer for pointer");
		assert!(
			persistent.occupancy() - occupied <= layout.frame_bytes() + size_of::<List<Graphic>>() + align_of::<List<Graphic>>(),
			"the promote allocated the lane slab and the field's own header, never the owned form of the shared interior"
		);
	}

	#[test]
	fn a_group_free_paint_field_moves_its_parked_header() {
		let mut transient = core_types::arena::Arena::new(1 << 16).unwrap();
		let persistent = core_types::arena::Arena::new(1 << 16).unwrap();

		let paint = List::new_from_element(Graphic::Vector(unit_square_at(DVec2::ZERO)));
		let heap = {
			let Some(Graphic::Vector(vector)) = paint.element(0) else { panic!("the paint carries a vector") };
			vector.point_domain.positions().as_ptr()
		};
		let (paint, _) = transient.alloc_sized_keyed(paint, 0).unwrap();

		let (layout, span, _frames) = promote_paint_field(Some(paint), 2, &transient, &persistent);
		let served = promoted_paint(&span, &layout, 0, &persistent);
		let Some(Graphic::Vector(vector)) = served.element(0) else { panic!("the promote keeps the vector") };
		assert_eq!(vector.point_domain.positions().as_ptr(), heap, "the promote moved the header, so the served paint names the pre-promote heap");
		assert!(std::ptr::eq(served, promoted_paint(&span, &layout, 1, &persistent)), "a paint two lanes share moves once");

		transient.reset();
		let served = promoted_paint(&span, &layout, 0, &persistent);
		assert!(matches!(served.element(0), Some(Graphic::Vector(_))), "the moved paint survives the transient reset");
	}
}
