//! The legacy bridge: record-backed groups rebuilt as owned legacy lists.

use super::walk::push_lane_paint_into_interiors;
use super::{Graphic, detable_items};
use crate::markers::{ATTR_FILL, ATTR_STROKE};
use core_types::Color;
use core_types::list::{AttributeValueDyn, Item, List};
use raster_types::{CPU, GPU, Raster};
use vector_types::{GradientStops, Vector};

/// One typed run as an owned list, elements cloned and every attribute copied
/// through its erased read. Content keeps its native form; the legacy
/// conversions layer their mapping on top.
pub fn run_to_list<T: Clone + Send + Sync + dyn_any::StaticTypeSized>(item: &core_types::record::GroupItem) -> Option<List<T>> {
	let lanes = item.typed_lanes::<T>()?;
	let mut list = List::new();
	for lane in 0..lanes.len() {
		list.push(Item::new_from_element(lanes.element_ref(lane).clone()));
	}
	for field in &item.layout().fields {
		for lane in 0..lanes.len() {
			// SAFETY: the offset comes from the item's own layout.
			let value = unsafe { (field.read_erased)(item.lanes().get(lane).rec().ptr().add(field.offset)) };
			list.set_attribute_value_dyn(field.name, lane, AttributeValueDyn(value));
		}
	}
	Some(list)
}

/// Converts the group content of the list's paint attribute values to legacy
/// form, so a legacy product owns everything its attributes reach.
pub fn map_paint_attrs_to_legacy<T>(list: &mut List<T>) {
	for key in [ATTR_FILL, ATTR_STROKE, crate::markers::ATTR_EDITOR_MERGED_LAYERS] {
		let Some(values) = list.iter_attribute_values_mut::<Option<List<Graphic>>>(key) else { continue };
		for value in values.flatten() {
			for element in value.iter_element_values_mut() {
				*element = map_groups_to_legacy(element);
			}
		}
	}
}

/// One typed run as a legacy list: [`run_to_list`] with the paint attribute
/// contents converted to their legacy form.
pub(crate) fn run_to_legacy_list<T: Clone + Send + Sync + dyn_any::StaticTypeSized>(item: &core_types::record::GroupItem) -> Option<List<T>> {
	let mut list = run_to_list::<T>(item)?;
	map_paint_attrs_to_legacy(&mut list);
	Some(list)
}

/// The graphic with every `Group` converted to its legacy form.
pub fn map_groups_to_legacy<'out>(graphic: &Graphic<'_>) -> Graphic<'out> {
	match graphic {
		Graphic::Group(group) => group_to_legacy_graphic(group),
		Graphic::Graphic(children) => {
			let mut out = List::new();
			for item in children.clone().into_iter() {
				let (element, attributes) = item.into_parts();
				out.push(Item::from_parts(map_groups_to_legacy(&element), attributes));
			}
			map_paint_attrs_to_legacy(&mut out);
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

/// The group as one legacy graphic. A bare (row-less) wrap of a single typed
/// run keeps the run's typed variant, matching the `Into<Graphic>` the
/// pre-flip wrap applied; everything else becomes the legacy group list.
pub fn group_to_legacy_graphic(group: &core_types::record::Group) -> Graphic<'static> {
	if group.row.is_none() {
		let item = &group.content;
		let typed = None
			.or_else(|| run_to_legacy_list::<Vector>(item).map(|list| detable_items(list, Graphic::Vector)))
			.or_else(|| run_to_legacy_list::<Raster<CPU>>(item).map(|list| detable_items(list, Graphic::RasterCPU)))
			.or_else(|| run_to_legacy_list::<Raster<GPU>>(item).map(|list| detable_items(list, Graphic::RasterGPU)))
			.or_else(|| run_to_legacy_list::<Color>(item).map(|list| detable_items(list, Graphic::Color)))
			.or_else(|| run_to_legacy_list::<GradientStops>(item).map(|list| detable_items(list, Graphic::Gradient)))
			.or_else(|| run_to_legacy_list::<String>(item).map(|list| detable_items(list, Graphic::Text)));
		if let Some(typed) = typed {
			return Graphic::Graphic(typed);
		}
	}
	Graphic::Graphic(group_to_legacy_list(group))
}

/// The group as a legacy `List<Graphic>`: a `Graphic` run becomes the items,
/// another typed run becomes one de-tabled leaf item per lane.
pub fn group_to_legacy_list(group: &core_types::record::Group) -> List<Graphic<'static>> {
	let item = &group.content;
	if let Some(mut list) = run_to_legacy_list::<Graphic>(item) {
		for element in list.iter_element_values_mut() {
			*element = map_groups_to_legacy(element);
		}
		push_lane_paint_into_interiors(&mut list);
		return list;
	}
	None.or_else(|| run_to_legacy_list::<Vector>(item).map(|list| detable_items(list, Graphic::Vector)))
		.or_else(|| run_to_legacy_list::<Raster<CPU>>(item).map(|list| detable_items(list, Graphic::RasterCPU)))
		.or_else(|| run_to_legacy_list::<Raster<GPU>>(item).map(|list| detable_items(list, Graphic::RasterGPU)))
		.or_else(|| run_to_legacy_list::<Color>(item).map(|list| detable_items(list, Graphic::Color)))
		.or_else(|| run_to_legacy_list::<GradientStops>(item).map(|list| detable_items(list, Graphic::Gradient)))
		.or_else(|| run_to_legacy_list::<String>(item).map(|list| detable_items(list, Graphic::Text)))
		.unwrap_or_default()
}

#[cfg(test)]
mod run_tests {
	use super::*;
	use crate::graphic::test_support::{native_group_paint, unit_square_at};
	use crate::markers::Fill;
	use core_types::attribute::Attribute;
	use core_types::record::{FieldWrite, RunBuilder, element_write_hashed};
	use glam::DVec2;

	#[test]
	fn a_legacy_list_owns_its_paint_attr_content() {
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
		let legacy = run_to_legacy_list::<Vector>(&item).expect("the run lowers to a legacy vector list");
		let expected = map_groups_to_legacy(paint.element(0).unwrap());
		drop(item);
		drop(paint);
		drop(source);

		let served = legacy.attribute::<Option<List<Graphic>>>(Fill::NAME, 0).expect("the fill attribute rides the list");
		let served = served.as_ref().expect("the fill is present");
		assert_eq!(served.element(0).unwrap(), &expected);
	}

	#[test]
	fn a_run_list_keeps_native_group_elements() {
		let inner_vector = unit_square_at(DVec2::ZERO);
		let arena = core_types::arena::Arena::new(1 << 16).unwrap();
		let content = native_group_paint(&inner_vector, &arena);
		let element = content.element(0).unwrap();

		let mut builder = RunBuilder::new(&arena, element_write_hashed::<Graphic>(), &[], 1).unwrap();
		builder.push(element.clone()).unwrap();
		let item = builder.finish();
		let list = run_to_list::<Graphic>(&item).expect("the run holds graphic lanes");
		assert!(matches!(list.element(0), Some(Graphic::Group(_))), "the list keeps the native group form");
	}
}
