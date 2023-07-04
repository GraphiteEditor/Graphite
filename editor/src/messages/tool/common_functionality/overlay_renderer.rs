use super::shape_editor::SelectedShapeState;
use crate::application::generate_uuid;
use crate::consts::VIEWPORT_GRID_ROUNDING_BIAS;
use crate::consts::{COLOR_ACCENT, HIDE_HANDLE_DISTANCE, MANIPULATOR_GROUP_MARKER_SIZE, PATH_OUTLINE_WEIGHT};
use crate::messages::prelude::*;

use bezier_rs::ManipulatorGroup;
use document_legacy::document::Document;
use document_legacy::layers::style::{self, Fill, Stroke};
use document_legacy::{LayerId, Operation};
use graphene_core::raster::color::Color;
use graphene_core::uuid::ManipulatorGroupId;
use graphene_core::vector::{ManipulatorPointId, SelectedType};

use glam::{DAffine2, DVec2};

/// [ManipulatorGroupOverlay]s is the collection of overlays that make up an [ManipulatorGroup] visible in the editor.
#[derive(Clone, Debug, Default)]
struct ManipulatorGroupOverlays {
	pub anchor: Option<Vec<LayerId>>,
	pub in_handle: Option<Vec<LayerId>>,
	pub in_line: Option<Vec<LayerId>>,
	pub out_handle: Option<Vec<LayerId>>,
	pub out_line: Option<Vec<LayerId>>,
}
impl ManipulatorGroupOverlays {
	pub fn iter(&self) -> impl Iterator<Item = &'_ Option<Vec<LayerId>>> {
		[&self.anchor, &self.in_handle, &self.in_line, &self.out_handle, &self.out_line].into_iter()
	}
}

type GraphiteManipulatorGroup = ManipulatorGroup<ManipulatorGroupId>;

const POINT_STROKE_WEIGHT: f64 = 2.;

#[derive(Clone, Debug, Default)]
pub struct OverlayRenderer {
	shape_overlay_cache: HashMap<LayerId, Vec<LayerId>>,
	manipulator_group_overlay_cache: HashMap<LayerId, HashMap<ManipulatorGroupId, ManipulatorGroupOverlays>>,
}

impl OverlayRenderer {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn render_subpath_overlays(&mut self, selected_shape_state: &SelectedShapeState, document: &Document, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		let transform = document.generate_transform_relative_to_viewport(&layer_path).ok().unwrap();
		if let Ok(layer) = document.layer(&layer_path) {
			let layer_id = layer_path.last().unwrap();
			self.layer_overlay_visibility(document, layer_path.clone(), true, responses);

			if let Some(vector_data) = layer.as_vector_data() {
				let outline_cache = self.shape_overlay_cache.get(layer_id);
				trace!("Overlay: Outline cache {:?}", &outline_cache);

				// Create an outline if we do not have a cached one
				if outline_cache.is_none() {
					let outline_path = self.create_shape_outline_overlay(graphene_core::vector::Subpath::from_bezier_rs(&vector_data.subpaths), responses);
					self.shape_overlay_cache.insert(*layer_id, outline_path.clone());
					Self::place_outline_overlays(outline_path.clone(), &transform, responses);
					trace!("Overlay: Creating new outline {:?}", &outline_path);
				} else if let Some(outline_path) = outline_cache {
					trace!("Overlay: Updating overlays for {:?} owning layer: {:?}", outline_path, layer_id);
					Self::modify_outline_overlays(outline_path.clone(), graphene_core::vector::Subpath::from_bezier_rs(&vector_data.subpaths), responses);
					Self::place_outline_overlays(outline_path.clone(), &transform, responses);
				}

				// Create, place, and style the manipulator overlays
				for manipulator_group in vector_data.manipulator_groups() {
					let manipulator_group_cache = self.manipulator_group_overlay_cache.entry(*layer_id).or_default().entry(manipulator_group.id).or_default();

					// Only view in and out handles if they are not on top of the anchor
					let [in_handle, out_handle] = {
						let anchor = manipulator_group.anchor;

						let anchor_position = transform.transform_point2(anchor);
						let not_under_anchor = |&position: &DVec2| transform.transform_point2(position).distance_squared(anchor_position) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
						let filter_handle = |manipulator: Option<DVec2>| manipulator.filter(not_under_anchor);
						[filter_handle(manipulator_group.in_handle), filter_handle(manipulator_group.out_handle)]
					};

					// Create anchor
					manipulator_group_cache.anchor = manipulator_group_cache.anchor.take().or_else(|| Some(Self::create_anchor_overlay(responses)));
					// Create or delete in handle
					if in_handle.is_none() {
						Self::remove_overlay(manipulator_group_cache.in_handle.take(), responses);
						Self::remove_overlay(manipulator_group_cache.in_line.take(), responses);
					} else {
						manipulator_group_cache.in_handle = manipulator_group_cache.in_handle.take().or_else(|| Self::create_handle_overlay_if_exists(in_handle, responses));
						manipulator_group_cache.in_line = manipulator_group_cache.in_line.take().or_else(|| Self::create_handle_line_overlay_if_exists(in_handle, responses));
					}
					// Create or delete out handle
					if out_handle.is_none() {
						Self::remove_overlay(manipulator_group_cache.out_handle.take(), responses);
						Self::remove_overlay(manipulator_group_cache.out_line.take(), responses);
					} else {
						manipulator_group_cache.out_handle = manipulator_group_cache.out_handle.take().or_else(|| Self::create_handle_overlay_if_exists(out_handle, responses));
						manipulator_group_cache.out_line = manipulator_group_cache.out_line.take().or_else(|| Self::create_handle_line_overlay_if_exists(out_handle, responses));
					}

					// Update placement and style
					Self::place_manipulator_group_overlays(manipulator_group, manipulator_group_cache, &transform, responses);
					Self::style_overlays(selected_shape_state, &layer_path, manipulator_group, manipulator_group_cache, responses);
				}

				if let Some(layer_overlays) = self.manipulator_group_overlay_cache.get_mut(layer_id) {
					if layer_overlays.len() > vector_data.manipulator_groups().count() {
						layer_overlays.retain(|manipulator, manipulator_group_overlays| {
							if vector_data.manipulator_groups().any(|current_manipulator| current_manipulator.id == *manipulator) {
								true
							} else {
								Self::remove_manipulator_group_overlays(manipulator_group_overlays, responses);
								false
							}
						});
					}
				}
				// TODO Handle removing shapes from cache so we don't memory leak
				// Eventually will get replaced with am immediate mode renderer for overlays
			}
		}
		responses.add(OverlaysMessage::Rerender);
	}

	pub fn clear_subpath_overlays(&mut self, _document: &Document, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		let layer_id = layer_path.last().unwrap();

		// Remove the shape outline overlays
		if let Some(overlay_path) = self.shape_overlay_cache.get(layer_id) {
			Self::remove_outline_overlays(overlay_path.clone(), responses)
		}
		self.shape_overlay_cache.remove(layer_id);

		// Remove the ManipulatorGroup overlays
		let Some(layer_cache) = self.manipulator_group_overlay_cache.remove(layer_id) else { return };

		for manipulator_group_overlays in layer_cache.values() {
			Self::remove_manipulator_group_overlays(manipulator_group_overlays, responses);
		}
	}

	pub fn layer_overlay_visibility(&mut self, document: &Document, layer_path: Vec<LayerId>, visibility: bool, responses: &mut VecDeque<Message>) {
		let layer_id = layer_path.last().unwrap();

		// Hide the shape outline overlays
		if let Some(overlay_path) = self.shape_overlay_cache.get(layer_id) {
			Self::set_outline_overlay_visibility(overlay_path.clone(), visibility, responses);
		}

		// Hide the manipulator group overlays
		let Some(manipulator_groups) = self.manipulator_group_overlay_cache.get(layer_id) else { return };
		if visibility {
			let Ok(layer) = document.layer(&layer_path) else { return };
			let Some(vector_data) = layer.as_vector_data()  else { return };
			for manipulator_group in vector_data.manipulator_groups() {
				let id = manipulator_group.id;
				if let Some(manipulator_group_overlays) = manipulator_groups.get(&id) {
					Self::set_manipulator_group_overlay_visibility(manipulator_group_overlays, visibility, responses);
				}
			}
		} else {
			for manipulator_group_overlays in manipulator_groups.values() {
				Self::set_manipulator_group_overlay_visibility(manipulator_group_overlays, visibility, responses);
			}
		}
	}

	/// Create the kurbo shape that matches the selected viewport shape.
	fn create_shape_outline_overlay(&self, subpath: graphene_core::vector::Subpath, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddShape {
			path: layer_path.clone(),
			subpath,
			style: style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), PATH_OUTLINE_WEIGHT)), Fill::None),
			insert_index: -1,
			transform: DAffine2::IDENTITY.to_cols_array(),
		};
		responses.add(DocumentMessage::Overlays(operation.into()));

		layer_path
	}

	/// Create a single anchor overlay and return its layer ID.
	fn create_anchor_overlay(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddRect {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), 2.0)), Fill::solid(Color::WHITE)),
			insert_index: -1,
		};
		responses.add(DocumentMessage::Overlays(operation.into()));
		layer_path
	}

	/// Create a single handle overlay and return its layer ID.
	fn create_handle_overlay(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddEllipse {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), 2.0)), Fill::solid(Color::WHITE)),
			insert_index: -1,
		};
		responses.add(DocumentMessage::Overlays(operation.into()));
		layer_path
	}

	/// Create a single handle overlay and return its layer id if it exists.
	fn create_handle_overlay_if_exists(handle: Option<DVec2>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		handle.map(|_| Self::create_handle_overlay(responses))
	}

	/// Remove an overlay at the specified path
	fn remove_overlay(path: Option<Vec<LayerId>>, responses: &mut VecDeque<Message>) {
		if let Some(path) = path {
			responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()));
		}
	}

	/// Create the shape outline overlay and return its layer ID.
	fn create_handle_line_overlay(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddLine {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), 1.0)), Fill::None),
			insert_index: -1,
		};
		responses.add_front(DocumentMessage::Overlays(operation.into()));
		layer_path
	}

	/// Create the shape outline overlay and return its layer ID.
	fn create_handle_line_overlay_if_exists(handle: Option<DVec2>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		handle.as_ref().map(|_| Self::create_handle_line_overlay(responses))
	}

	fn place_outline_overlays(outline_path: Vec<LayerId>, parent_transform: &DAffine2, responses: &mut VecDeque<Message>) {
		let transform_message = Self::overlay_transform_message(outline_path, parent_transform.to_cols_array());
		responses.add(transform_message);
	}

	fn modify_outline_overlays(outline_path: Vec<LayerId>, subpath: graphene_core::vector::Subpath, responses: &mut VecDeque<Message>) {
		let outline_modify_message = Self::overlay_modify_message(outline_path, subpath);
		responses.add(outline_modify_message);
	}

	/// Updates the position of the overlays based on the [Subpath] points.
	fn place_manipulator_group_overlays(manipulator_group: &GraphiteManipulatorGroup, overlays: &mut ManipulatorGroupOverlays, parent_transform: &DAffine2, responses: &mut VecDeque<Message>) {
		let anchor = manipulator_group.anchor;

		let mut place_handle_and_line = |handle_position: DVec2, line_overlay: &[LayerId], marker_source: &mut Option<Vec<LayerId>>| {
			let line_vector = parent_transform.transform_point2(anchor) - parent_transform.transform_point2(handle_position);
			let scale = DVec2::splat(line_vector.length());
			let angle = -line_vector.angle_between(DVec2::X);

			let translation = (parent_transform.transform_point2(handle_position) + VIEWPORT_GRID_ROUNDING_BIAS).round() + DVec2::splat(0.5);
			let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
			responses.add(Self::overlay_transform_message(line_overlay.to_vec(), transform));

			let marker_overlay = marker_source.take().unwrap_or_else(|| Self::create_handle_overlay(responses));

			let scale = DVec2::splat(MANIPULATOR_GROUP_MARKER_SIZE);
			let angle = 0.;
			let translation = (parent_transform.transform_point2(handle_position) - (scale / 2.) + VIEWPORT_GRID_ROUNDING_BIAS).round();
			let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

			responses.add(Self::overlay_transform_message(marker_overlay.clone(), transform));

			*marker_source = Some(marker_overlay);
		};

		// Place the handle overlays
		if let (Some(handle_position), Some(line_overlay)) = (manipulator_group.in_handle, overlays.in_line.as_mut()) {
			place_handle_and_line(handle_position, line_overlay, &mut overlays.in_handle);
		}
		if let (Some(handle_position), Some(line_overlay)) = (manipulator_group.out_handle, overlays.out_line.as_ref()) {
			place_handle_and_line(handle_position, line_overlay, &mut overlays.out_handle);
		}

		// Place the anchor point overlay
		if let Some(anchor_overlay) = &overlays.anchor {
			let scale = DVec2::splat(MANIPULATOR_GROUP_MARKER_SIZE);
			let angle = 0.;
			let translation = (parent_transform.transform_point2(anchor) - (scale / 2.) + VIEWPORT_GRID_ROUNDING_BIAS).round();
			let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

			let message = Self::overlay_transform_message(anchor_overlay.clone(), transform);
			responses.add(message);
		}
	}

	/// Removes the manipulator overlays from the overlay document.
	fn remove_manipulator_group_overlays(overlay_paths: &ManipulatorGroupOverlays, responses: &mut VecDeque<Message>) {
		overlay_paths.iter().flatten().for_each(|layer_id| {
			trace!("Overlay: Sending delete message for: {:?}", layer_id);
			responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer_id.clone() }.into()));
		});
	}

	fn remove_outline_overlays(overlay_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		responses.add(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path }.into()));
	}

	/// Sets the visibility of the handles overlay.
	fn set_manipulator_group_overlay_visibility(manipulator_group_overlays: &ManipulatorGroupOverlays, visibility: bool, responses: &mut VecDeque<Message>) {
		manipulator_group_overlays.iter().flatten().for_each(|layer_id| {
			responses.add(Self::overlay_visibility_message(layer_id.clone(), visibility));
		});
	}

	fn set_outline_overlay_visibility(overlay_path: Vec<LayerId>, visibility: bool, responses: &mut VecDeque<Message>) {
		responses.add(Self::overlay_visibility_message(overlay_path, visibility));
	}

	/// Create a visibility message for an overlay.
	fn overlay_visibility_message(layer_path: Vec<LayerId>, visibility: bool) -> Message {
		DocumentMessage::Overlays(
			Operation::SetLayerVisibility {
				path: layer_path,
				visible: visibility,
			}
			.into(),
		)
		.into()
	}

	/// Create a transform message for an overlay.
	fn overlay_transform_message(layer_path: Vec<LayerId>, transform: [f64; 6]) -> Message {
		DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: layer_path, transform }.into()).into()
	}

	/// Create an update message for an overlay.
	fn overlay_modify_message(layer_path: Vec<LayerId>, subpath: graphene_core::vector::Subpath) -> Message {
		DocumentMessage::Overlays(Operation::SetShapePath { path: layer_path, subpath }.into()).into()
	}

	/// Sets the overlay style for this point.
	fn style_overlays(state: &SelectedShapeState, layer_path: &[LayerId], manipulator_group: &GraphiteManipulatorGroup, overlays: &ManipulatorGroupOverlays, responses: &mut VecDeque<Message>) {
		// TODO Move the style definitions out of the Subpath, should be looked up from a stylesheet or similar
		let selected_style = style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), POINT_STROKE_WEIGHT + 1.0)), Fill::solid(COLOR_ACCENT));
		let deselected_style = style::PathStyle::new(Some(Stroke::new(Some(COLOR_ACCENT), POINT_STROKE_WEIGHT)), Fill::solid(Color::WHITE));
		let selected_shape_state = state.get(layer_path);
		// Update if the manipulator points are shown as selected
		// Here the index is important, even though overlays[..] has five elements we only care about the first three
		for (index, overlay) in [&overlays.in_handle, &overlays.out_handle, &overlays.anchor].into_iter().enumerate() {
			let selected_type = [SelectedType::InHandle, SelectedType::OutHandle, SelectedType::Anchor][index];
			if let Some(overlay_path) = overlay {
				let selected = selected_shape_state
					.filter(|state| state.is_selected(ManipulatorPointId::new(manipulator_group.id, selected_type)))
					.is_some();

				let style = if selected { selected_style.clone() } else { deselected_style.clone() };
				responses.add(DocumentMessage::Overlays(Operation::SetLayerStyle { path: overlay_path.clone(), style }.into()));
			}
		}
	}
}
