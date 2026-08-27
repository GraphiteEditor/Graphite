//! Boundary helpers between leveled wires and the legacy editor surface:
//! the renderer's flip form materializes a wire into a group, and captured
//! wires convert to the legacy values the editor's downcasts expect.

use crate::graphic::{Graphic, group_to_legacy_list, run_to_legacy_list};
use crate::raster_types::{CPU, GPU, Raster};
use crate::{Artboard, Vector};
use core_types::Color;
use core_types::arena::Arena;
use core_types::context::InjectIndex;
use core_types::gpoll::{Finality, GraphError};
use core_types::node::Node;
use core_types::record::{Group, GroupContent, GroupItem, LevelStatus, RecordValue, materialize_level};
use core_types::uuid::NodeId;
use glam::{DAffine2, DVec2};
use vector_types::GradientStops;

/// The outcome of materializing a leveled wire into a group.
pub enum LevelGroup {
	Group(Group, Finality),
	Pending,
	Error(GraphError),
}

/// The renderer's flip form: the wire's whole extent materialized into a
/// group over the level's records, ready for the group render bridge.
pub fn materialize_group<'e, C, N>(node: &N, input: &C, arena: &Arena) -> LevelGroup
where
	C: InjectIndex + Copy,
	N: Node<C, Output = RecordValue<'e>>,
{
	match materialize_level(node, input, arena) {
		LevelStatus::Batch(batch, finality) => {
			// SAFETY: a materialized batch's frames are arena-resident.
			let item = unsafe { GroupItem::from_resident(batch) };
			LevelGroup::Group(
				Group {
					row: None,
					content: GroupContent::Run(item),
				},
				finality,
			)
		}
		LevelStatus::Pending => LevelGroup::Pending,
		LevelStatus::Error(error) => LevelGroup::Error(error),
	}
}

/// The resident batch as the legacy value the editor's downcasts expect: a
/// rank-0 wire is its element, a leveled `Graphic` wire becomes its legacy
/// list through the group bridge, and another element type becomes a legacy
/// list of that element. `None` for an element type outside the legacy
/// vocabulary or an empty rank-0 batch.
pub fn batch_to_legacy(layout: &core_types::record::Layout, batch: core_types::node::RecordBatch<'_>, _arena: &Arena) -> Option<Box<dyn std::any::Any + Send + Sync>> {
	if layout.depth == 0 {
		// The group-carrying types legacy-convert while the batch is
		// resident; the deep clone-out would hand back the unreadable owned
		// form.
		if batch.is_empty() {
			return None;
		}
		let element = &layout.element;
		if element.type_id == std::any::TypeId::of::<Graphic>() {
			// SAFETY: the layout records the element type, and a parked
			// element stores its reference at offset 0.
			let graphic = unsafe { core_types::record::borrow_element::<Graphic>(batch.get(0).rec()) };
			return Some(Box::new(crate::graphic::map_groups_to_legacy(graphic)));
		}
		if element.type_id == std::any::TypeId::of::<Artboard>() {
			// SAFETY: as for the graphic arm.
			let artboard = unsafe { core_types::record::borrow_element::<Artboard>(batch.get(0).rec()) };
			return Some(Box::new(artboard.with_legacy_groups()));
		}
		// SAFETY: lane 0 is a live record of `layout`.
		return Some(unsafe { (element.clone_out)(batch.get(0).rec().ptr()) });
	}
	// SAFETY: the caller's batch is resident for the read.
	let item = unsafe { GroupItem::from_resident(batch) };
	fn typed<T: Clone + Send + Sync + 'static>(item: &GroupItem) -> Option<Box<dyn std::any::Any + Send + Sync>> {
		run_to_legacy_list::<T>(item).map(|list| Box::new(list) as Box<dyn std::any::Any + Send + Sync>)
	}
	if item.typed_lanes::<Graphic>().is_some() {
		let group = Group {
			row: None,
			content: GroupContent::Run(item),
		};
		return Some(Box::new(group_to_legacy_list(&group)));
	}
	fn typed_artboards(item: &GroupItem) -> Option<Box<dyn std::any::Any + Send + Sync>> {
		let mut list = run_to_legacy_list::<Artboard>(item)?;
		// The capture outlives the arena generation, so group content must
		// leave in its owned legacy form.
		for artboard in list.iter_element_values_mut() {
			*artboard = artboard.with_legacy_groups();
		}
		Some(Box::new(list))
	}
	None.or_else(|| typed_artboards(&item))
		.or_else(|| typed::<Vector>(&item))
		.or_else(|| typed::<Raster<CPU>>(&item))
		.or_else(|| typed::<Raster<GPU>>(&item))
		.or_else(|| typed::<Color>(&item))
		.or_else(|| typed::<GradientStops>(&item))
		.or_else(|| typed::<String>(&item))
		.or_else(|| typed::<f64>(&item))
		.or_else(|| typed::<u64>(&item))
		.or_else(|| typed::<u32>(&item))
		.or_else(|| typed::<bool>(&item))
		.or_else(|| typed::<NodeId>(&item))
		.or_else(|| typed::<DAffine2>(&item))
		.or_else(|| typed::<DVec2>(&item))
}
