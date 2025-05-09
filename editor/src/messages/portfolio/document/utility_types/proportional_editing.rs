use glam::DVec2;
use graphene_std::vector::PointId;
use std::collections::{HashMap, HashSet};

use super::document_metadata::LayerNodeIdentifier;
use crate::messages::prelude::*;
use crate::messages::tool::{
	common_functionality::shape_editor::ShapeState,
	tool_messages::{
		path_tool::{PathOptionsUpdate, PathToolData, PathToolOptions},
		tool_prelude::{DropdownInput, LayoutGroup, MenuListEntry, NumberInput, Separator, SeparatorType, TextLabel},
	},
};

#[derive(PartialEq, Eq, Hash, Copy, Clone, Debug, Default, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ProportionalFalloffType {
	#[default]
	Smooth = 0,
	Sphere = 1,
	Root = 2,
	InverseSquare = 3,
	Sharp = 4,
	Linear = 5,
	Constant = 6,
	Random = 7,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ProportionalEditingData {
	pub center: DVec2,
	pub affected_points: HashMap<LayerNodeIdentifier, Vec<(PointId, f64)>>,
	pub falloff_type: ProportionalFalloffType,
	pub radius: u32,
}

pub fn proportional_editing_options(options: &PathToolOptions) -> Vec<LayoutGroup> {
	let mut widgets = Vec::new();

	// Header row with title
	widgets.push(LayoutGroup::Row {
		widgets: vec![TextLabel::new("Proportional Editing").bold(true).widget_holder()],
	});

	let callback = |message| Message::Batched(Box::new([PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalEditingEnabled(true)).into(), message]));

	// Falloff type row
	widgets.push(LayoutGroup::Row {
		widgets: vec![
			TextLabel::new("Falloff").table_align(true).min_width(80).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			DropdownInput::new(vec![vec![
				MenuListEntry::new("Smooth")
					.label("Smooth")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::Smooth)).into())),
				MenuListEntry::new("Sphere")
					.label("Sphere")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::Sphere)).into())),
				MenuListEntry::new("Root")
					.label("Root")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::Root)).into())),
				MenuListEntry::new("Inverse Square")
					.label("Inverse Square")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::InverseSquare)).into())),
				MenuListEntry::new("Sharp")
					.label("Sharp")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::Sharp)).into())),
				MenuListEntry::new("Linear")
					.label("Linear")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::Linear)).into())),
				MenuListEntry::new("Constant")
					.label("Constant")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::Constant)).into())),
				MenuListEntry::new("Random")
					.label("Random")
					.on_commit(move |_| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalFalloffType(ProportionalFalloffType::Random)).into())),
			]])
			.min_width(120)
			.selected_index(Some(options.proportional_falloff_type as u32))
			.widget_holder(),
		],
	});

	// Radius row
	widgets.push(LayoutGroup::Row {
		widgets: vec![
			TextLabel::new("Radius").table_align(true).min_width(80).widget_holder(),
			Separator::new(SeparatorType::Unrelated).widget_holder(),
			NumberInput::new(Some(options.proportional_radius as f64))
				.unit(" px")
				.min(1.)
				.int()
				.min_width(120)
				.on_update(move |number_input| callback(PathToolMessage::UpdateOptions(PathOptionsUpdate::ProportionalRadius(number_input.value.unwrap_or(1.) as u32)).into()))
				.widget_holder(),
		],
	});

	widgets
}

pub fn calculate_proportional_affected_points(
	path_tool_data: &mut PathToolData,
	document: &DocumentMessageHandler,
	shape_editor: &ShapeState,
	radius: u32,
	proportional_falloff_type: ProportionalFalloffType,
) {
	path_tool_data.proportional_affected_points.clear();

	let radius = radius as f64;

	// If initial positions haven't been stored yet, do it now
	if path_tool_data.initial_point_positions.is_empty() {
		store_initial_point_positions(path_tool_data, document);
	}

	// Collect all selected points with their initial world positions
	let mut selected_points_world_pos = Vec::new();
	let selected_point_ids: HashSet<_> = shape_editor.selected_points().filter_map(|point| point.as_anchor()).collect();

	// Extract initial positions of selected points
	for (_layer, points_map) in &path_tool_data.initial_point_positions {
		for &point_id in &selected_point_ids {
			if let Some(&world_pos) = points_map.get(&point_id) {
				selected_points_world_pos.push(world_pos);
			}
		}
	}

	// Find all affected points using initial positions
	for (layer, points_map) in &path_tool_data.initial_point_positions {
		let selected_points: HashSet<_> = shape_editor.selected_points().filter_map(|point| point.as_anchor()).collect();

		let mut layer_affected_points = Vec::new();

		// Check each point in the layer
		for (&point_id, &initial_position) in points_map {
			if !selected_points.contains(&point_id) {
				// Find the smallest distance to any selected point using initial positions
				let min_distance = selected_points_world_pos
					.iter()
					.map(|&selected_pos| initial_position.distance(selected_pos))
					.filter(|&distance| distance <= radius)
					.min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

				if let Some(distance) = min_distance {
					let factor = path_tool_data.calculate_falloff_factor(distance, radius, proportional_falloff_type);
					layer_affected_points.push((point_id, factor));
				}
			}
		}

		if !layer_affected_points.is_empty() {
			path_tool_data.proportional_affected_points.insert(*layer, layer_affected_points);
		}
	}

	// Find all affected points using initial positions
	// NOTE: This works based on initial affected point location -> original selected point location for falloff calculation
	for (layer, points_map) in &path_tool_data.initial_point_positions {
		let selected_points: HashSet<_> = shape_editor.selected_points().filter_map(|point| point.as_anchor()).collect();

		let mut layer_affected_points = Vec::new();

		// Check each point in the layer
		for (&point_id, &initial_position) in points_map {
			if !selected_points.contains(&point_id) {
				// Find the smallest distance to any selected point using initial positions
				let min_distance = selected_points_world_pos
					.iter()
					.map(|&selected_pos| initial_position.distance(selected_pos))
					.filter(|&distance| distance <= radius)
					.min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

				if let Some(distance) = min_distance {
					let factor = path_tool_data.calculate_falloff_factor(distance, radius, proportional_falloff_type);
					layer_affected_points.push((point_id, factor));
				}
			}
		}

		if !layer_affected_points.is_empty() {
			path_tool_data.proportional_affected_points.insert(*layer, layer_affected_points);
		}
	}
}

pub fn store_initial_point_positions(path_tool_data: &mut PathToolData, document: &DocumentMessageHandler) {
	path_tool_data.initial_point_positions.clear();

	// Store positions of all points in selected layers
	for layer in document.network_interface.selected_nodes().selected_layers(document.metadata()) {
		if let Some(vector_data) = document.network_interface.compute_modified_vector(layer) {
			let transform = document.metadata().transform_to_document(layer);
			let mut layer_points = HashMap::new();

			// Store all point positions in document space
			for (i, &point_id) in vector_data.point_domain.ids().iter().enumerate() {
				let position = vector_data.point_domain.positions()[i];
				let world_pos = transform.transform_point2(position);
				layer_points.insert(point_id, world_pos);
			}

			if !layer_points.is_empty() {
				path_tool_data.initial_point_positions.insert(layer, layer_points);
			}
		}
	}
}

pub fn update_proportional_positions(path_tool_data: &mut PathToolData, document: &DocumentMessageHandler, shape_editor: &mut ShapeState, responses: &mut VecDeque<Message>) {
	// Get a set of all selected point IDs across all layers
	let selected_points: HashSet<PointId> = shape_editor.selected_points().filter_map(|point| point.as_anchor()).collect();

	for (layer, affected_points) in &path_tool_data.proportional_affected_points {
		if let Some(vector_data) = document.network_interface.compute_modified_vector(*layer) {
			let transform = document.metadata().transform_to_document(*layer);
			let inverse_transform = transform.inverse();

			for (point_id, factor) in affected_points {
				// Skip this point if it's in the selected_points set
				if selected_points.contains(point_id) {
					continue;
				}

				if let Some(initial_doc_pos) = path_tool_data.initial_point_positions.get(layer).and_then(|pts| pts.get(point_id)) {
					// Calculate displacement from initial position to target position
					let displacement_document_space = path_tool_data.total_delta * (*factor);
					let target_document_space_position = *initial_doc_pos + displacement_document_space;
					let target_layer_space_position = inverse_transform.transform_point2(target_document_space_position);

					// Get current position and calculate delta
					if let Some(current_layer_space_position) = vector_data.point_domain.position_from_id(*point_id) {
						let delta = target_layer_space_position - current_layer_space_position;
						shape_editor.move_anchor(*point_id, &vector_data, delta, *layer, None, responses);
					}
				}
			}
		}
	}
}

pub fn reset_removed_points(
	path_tool_data: &mut PathToolData,
	previous: &HashMap<LayerNodeIdentifier, Vec<(PointId, f64)>>,
	document: &DocumentMessageHandler,
	shape_editor: &mut ShapeState,
	responses: &mut VecDeque<Message>,
) {
	for (layer, prev_points) in previous {
		let current_points = path_tool_data
			.proportional_affected_points
			.get(layer)
			.map(|v| v.iter().map(|(id, _)| *id).collect::<HashSet<_>>())
			.unwrap_or_default();

		for (point_id, _) in prev_points {
			if !current_points.contains(point_id) {
				if let Some(initial_doc_pos) = path_tool_data.initial_point_positions.get(layer).and_then(|pts| pts.get(point_id)) {
					let inverse_transform = document.metadata().transform_to_document(*layer).inverse();
					let target_layer_pos = inverse_transform.transform_point2(*initial_doc_pos);

					if let Some(vector_data) = document.network_interface.compute_modified_vector(*layer) {
						if let Some(current_layer_pos) = vector_data.point_domain.position_from_id(*point_id) {
							let delta = target_layer_pos - current_layer_pos;
							shape_editor.move_anchor(*point_id, &vector_data, delta, *layer, None, responses);
						}
					}
				}
			}
		}
	}
}
