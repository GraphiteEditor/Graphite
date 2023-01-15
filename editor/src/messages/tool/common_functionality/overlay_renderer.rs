use crate::application::generate_uuid;
use crate::consts::VIEWPORT_GRID_ROUNDING_BIAS;
use crate::consts::{COLOR_ACCENT, HIDE_HANDLE_DISTANCE, MANIPULATOR_GROUP_MARKER_SIZE, PATH_OUTLINE_WEIGHT};
use crate::messages::prelude::*;

use document_legacy::color::Color;
use document_legacy::document::Document;
use document_legacy::layers::style::{self, Fill, Stroke};
use document_legacy::{LayerId, Operation};
use graphene_std::vector::consts::ManipulatorType;
use graphene_std::vector::manipulator_group::ManipulatorGroup;
use graphene_std::vector::manipulator_point::ManipulatorPoint;
use graphene_std::vector::subpath::Subpath;

use glam::{DAffine2, DVec2};

/// [ManipulatorGroupOverlay]s is the collection of overlays that make up an [ManipulatorGroup] visible in the editor.
type ManipulatorGroupOverlays = [Option<Vec<LayerId>>; 5];
type ManipulatorId = u64;

const POINT_STROKE_WEIGHT: f64 = 2.;

#[derive(Clone, Debug, Default)]
pub struct OverlayRenderer {
	shape_overlay_cache: HashMap<LayerId, Vec<LayerId>>,
	manipulator_group_overlay_cache: HashMap<(LayerId, ManipulatorId), ManipulatorGroupOverlays>,
}

impl OverlayRenderer {
	pub fn new() -> Self {
		OverlayRenderer {
			manipulator_group_overlay_cache: HashMap::new(),
			shape_overlay_cache: HashMap::new(),
		}
	}

	pub fn render_subpath_overlays(&mut self, document: &Document, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		let transform = document.generate_transform_relative_to_viewport(&layer_path).ok().unwrap();
		if let Ok(layer) = document.layer(&layer_path) {
			let layer_id = layer_path.last().unwrap();
			self.layer_overlay_visibility(document, layer_path.clone(), true, responses);

			if let Some(shape) = layer.as_subpath() {
				let outline_cache = self.shape_overlay_cache.get(layer_id);
				trace!("Overlay: Outline cache {:?}", &outline_cache);

				// Create an outline if we do not have a cached one
				if outline_cache.is_none() {
					let outline_path = self.create_shape_outline_overlay(shape.clone(), responses);
					self.shape_overlay_cache.insert(*layer_id, outline_path.clone());
					Self::place_outline_overlays(outline_path.clone(), &transform, responses);
					trace!("Overlay: Creating new outline {:?}", &outline_path);
				} else if let Some(outline_path) = outline_cache {
					trace!("Overlay: Updating overlays for {:?} owning layer: {:?}", outline_path, layer_id);
					Self::modify_outline_overlays(outline_path.clone(), shape.clone(), responses);
					Self::place_outline_overlays(outline_path.clone(), &transform, responses);
				}

				// Create, place, and style the manipulator overlays
				for (manipulator_group_id, manipulator_group) in shape.manipulator_groups().enumerate() {
					let manipulator_group_cache = self.manipulator_group_overlay_cache.entry((*layer_id, *manipulator_group_id)).or_insert(Default::default());

					// Only view in and out handles if they are not on top of the anchor
					let [in_handle, out_handle] = {
						let Some(anchor) = manipulator_group.points[ManipulatorType::Anchor].as_ref() else{
							continue;
						};

						let anchor_position = transform.transform_point2(anchor.position);
						let filter_position = |handle: &&ManipulatorPoint| transform.transform_point2(handle.position).distance_squared(anchor_position) >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
						let filter_manipulator_point = |manipulator_type| manipulator_group.points[manipulator_type as usize].as_ref().filter(filter_position);
						[filter_manipulator_point(ManipulatorType::InHandle), filter_manipulator_point(ManipulatorType::OutHandle)]
					};

					// Create anchor
					manipulator_group_cache[0] = manipulator_group_cache[0].take().or_else(|| Some(Self::create_anchor_overlay(responses)));
					// Create or delete in handle
					if in_handle.is_none() {
						Self::remove_overlay(manipulator_group_cache[1].take(), responses);
						Self::remove_overlay(manipulator_group_cache[3].take(), responses);
					} else {
						manipulator_group_cache[1] = manipulator_group_cache[1].take().or_else(|| Self::create_handle_overlay_if_exists(in_handle, responses));
						manipulator_group_cache[3] = manipulator_group_cache[3].take().or_else(|| Self::create_handle_line_overlay_if_exists(in_handle, responses));
					}
					// Create or delete out handle
					if out_handle.is_none() {
						Self::remove_overlay(manipulator_group_cache[2].take(), responses);
						Self::remove_overlay(manipulator_group_cache[4].take(), responses);
					} else {
						manipulator_group_cache[2] = manipulator_group_cache[2].take().or_else(|| Self::create_handle_overlay_if_exists(out_handle, responses));
						manipulator_group_cache[4] = manipulator_group_cache[4].take().or_else(|| Self::create_handle_line_overlay_if_exists(out_handle, responses));
					}

					// Update placement and style
					Self::place_manipulator_group_overlays(manipulator_group, manipulator_group_cache, &transform, responses);
					Self::style_overlays(manipulator_group, manipulator_group_cache, responses);
				}
				// TODO Handle removing shapes from cache so we don't memory leak
				// Eventually will get replaced with am immediate mode renderer for overlays
			}
		}
	}

	pub fn clear_subpath_overlays(&mut self, document: &Document, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		let layer_id = layer_path.last().unwrap();

		// Remove the shape outline overlays
		if let Some(overlay_path) = self.shape_overlay_cache.get(layer_id) {
			Self::remove_outline_overlays(overlay_path.clone(), responses)
		}
		self.shape_overlay_cache.remove(layer_id);

		// Remove the ManipulatorGroup overlays
		if let Ok(layer) = document.layer(&layer_path) {
			if let Some(shape) = layer.as_subpath() {
				for (id, _) in shape.manipulator_groups().enumerate() {
					if let Some(manipulator_group_overlays) = self.manipulator_group_overlay_cache.get(&(*layer_id, *id)) {
						Self::remove_manipulator_group_overlays(manipulator_group_overlays, responses);
						self.manipulator_group_overlay_cache.remove(&(*layer_id, *id));
					}
				}
			}
		}
	}

	pub fn layer_overlay_visibility(&mut self, document: &Document, layer_path: Vec<LayerId>, visibility: bool, responses: &mut VecDeque<Message>) {
		let layer_id = layer_path.last().unwrap();

		// Hide the shape outline overlays
		if let Some(overlay_path) = self.shape_overlay_cache.get(layer_id) {
			Self::set_outline_overlay_visibility(overlay_path.clone(), visibility, responses);
		}

		// Hide the manipulator group overlays
		if let Ok(layer) = document.layer(&layer_path) {
			if let Some(shape) = layer.as_subpath() {
				for (id, _) in shape.manipulator_groups().enumerate() {
					if let Some(manipulator_group_overlays) = self.manipulator_group_overlay_cache.get(&(*layer_id, *id)) {
						Self::set_manipulator_group_overlay_visibility(manipulator_group_overlays, visibility, responses);
					}
				}
			}
		}
	}

	/// Create the kurbo shape that matches the selected viewport shape.
	fn create_shape_outline_overlay(&self, subpath: Subpath, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddShape {
			path: layer_path.clone(),
			subpath,
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, PATH_OUTLINE_WEIGHT)), Fill::None),
			insert_index: -1,
			transform: DAffine2::IDENTITY.to_cols_array(),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		layer_path
	}

	/// Create a single anchor overlay and return its layer ID.
	fn create_anchor_overlay(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddRect {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Fill::solid(Color::WHITE)),
			insert_index: -1,
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	/// Create a single handle overlay and return its layer ID.
	fn create_handle_overlay(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddEllipse {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Fill::solid(Color::WHITE)),
			insert_index: -1,
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	/// Create a single handle overlay and return its layer id if it exists.
	fn create_handle_overlay_if_exists(handle: Option<&ManipulatorPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		handle.map(|_| Self::create_handle_overlay(responses))
	}

	/// Remove an overlay at the specified path
	fn remove_overlay(path: Option<Vec<LayerId>>, responses: &mut VecDeque<Message>) {
		if let Some(path) = path {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path }.into()).into());
		}
	}

	/// Create the shape outline overlay and return its layer ID.
	fn create_handle_line_overlay(responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddLine {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Fill::None),
			insert_index: -1,
		};
		responses.push_front(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	/// Create the shape outline overlay and return its layer ID.
	fn create_handle_line_overlay_if_exists(handle: Option<&ManipulatorPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		handle.as_ref().map(|_| Self::create_handle_line_overlay(responses))
	}

	fn place_outline_overlays(outline_path: Vec<LayerId>, parent_transform: &DAffine2, responses: &mut VecDeque<Message>) {
		let transform_message = Self::overlay_transform_message(outline_path, parent_transform.to_cols_array());
		responses.push_back(transform_message);
	}

	fn modify_outline_overlays(outline_path: Vec<LayerId>, subpath: Subpath, responses: &mut VecDeque<Message>) {
		let outline_modify_message = Self::overlay_modify_message(outline_path, subpath);
		responses.push_back(outline_modify_message);
	}

	/// Updates the position of the overlays based on the [Subpath] points.
	fn place_manipulator_group_overlays(manipulator_group: &ManipulatorGroup, overlays: &mut ManipulatorGroupOverlays, parent_transform: &DAffine2, responses: &mut VecDeque<Message>) {
		if let Some(manipulator_point) = &manipulator_group.points[ManipulatorType::Anchor] {
			// Helper function to keep things DRY (don't-repeat-yourself)
			let mut place_handle_and_line = |handle: &ManipulatorPoint, line_overlay: &mut Vec<LayerId>, marker_source: &mut Option<Vec<LayerId>>| {
				let line_vector = parent_transform.transform_point2(manipulator_point.position) - parent_transform.transform_point2(handle.position);
				let scale = DVec2::splat(line_vector.length());
				let angle = -line_vector.angle_between(DVec2::X);
				let translation = (parent_transform.transform_point2(handle.position) + VIEWPORT_GRID_ROUNDING_BIAS).round() + DVec2::splat(0.5);
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
				responses.push_back(Self::overlay_transform_message(line_overlay.clone(), transform));

				let marker_overlay = marker_source.take().unwrap_or_else(|| Self::create_handle_overlay(responses));
				let scale = DVec2::splat(MANIPULATOR_GROUP_MARKER_SIZE);
				let angle = 0.;
				let translation = (parent_transform.transform_point2(handle.position) - (scale / 2.) + VIEWPORT_GRID_ROUNDING_BIAS).round();
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
				responses.push_back(Self::overlay_transform_message(marker_overlay.clone(), transform));
				*marker_source = Some(marker_overlay);
			};

			// Place the handle overlays
			let [_, h1, h2] = &manipulator_group.points;
			let [a, b, c, line1, line2] = overlays;
			let markers = [a, b, c];
			if let (Some(handle), Some(line_source)) = (h1.as_ref(), line1.as_mut()) {
				place_handle_and_line(handle, line_source, markers[handle.manipulator_type as usize]);
			}
			if let (Some(handle), Some(line_source)) = (h2.as_ref(), line2.as_mut()) {
				place_handle_and_line(handle, line_source, markers[handle.manipulator_type as usize]);
			}

			// Place the anchor point overlay
			if let Some(anchor_overlay) = &overlays[ManipulatorType::Anchor as usize] {
				let scale = DVec2::splat(MANIPULATOR_GROUP_MARKER_SIZE);
				let angle = 0.;
				let translation = (parent_transform.transform_point2(manipulator_point.position) - (scale / 2.) + VIEWPORT_GRID_ROUNDING_BIAS).round();
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

				let message = Self::overlay_transform_message(anchor_overlay.clone(), transform);
				responses.push_back(message);
			}
		}
	}

	/// Removes the manipulator overlays from the overlay document.
	fn remove_manipulator_group_overlays(overlay_paths: &ManipulatorGroupOverlays, responses: &mut VecDeque<Message>) {
		overlay_paths.iter().flatten().for_each(|layer_id| {
			trace!("Overlay: Sending delete message for: {:?}", layer_id);
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer_id.clone() }.into()).into());
		});
	}

	fn remove_outline_overlays(overlay_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path }.into()).into());
	}

	/// Sets the visibility of the handles overlay.
	fn set_manipulator_group_overlay_visibility(manipulator_group_overlays: &ManipulatorGroupOverlays, visibility: bool, responses: &mut VecDeque<Message>) {
		manipulator_group_overlays.iter().flatten().for_each(|layer_id| {
			responses.push_back(Self::overlay_visibility_message(layer_id.clone(), visibility));
		});
	}

	fn set_outline_overlay_visibility(overlay_path: Vec<LayerId>, visibility: bool, responses: &mut VecDeque<Message>) {
		responses.push_back(Self::overlay_visibility_message(overlay_path, visibility));
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
	fn overlay_modify_message(layer_path: Vec<LayerId>, subpath: Subpath) -> Message {
		DocumentMessage::Overlays(Operation::SetShapePath { path: layer_path, subpath }.into()).into()
	}

	/// Sets the overlay style for this point.
	fn style_overlays(manipulator_group: &ManipulatorGroup, overlays: &ManipulatorGroupOverlays, responses: &mut VecDeque<Message>) {
		// TODO Move the style definitions out of the Subpath, should be looked up from a stylesheet or similar
		let selected_style = style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, POINT_STROKE_WEIGHT + 1.0)), Fill::solid(COLOR_ACCENT));
		let deselected_style = style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, POINT_STROKE_WEIGHT)), Fill::solid(Color::WHITE));

		// Update if the manipulator points are shown as selected
		// Here the index is important, even though overlays[..] has five elements we only care about the first three
		for (index, point) in manipulator_group.points.iter().enumerate() {
			if let Some(point) = point {
				if let Some(overlay) = &overlays[index] {
					let style = if point.editor_state.is_selected { selected_style.clone() } else { deselected_style.clone() };
					responses.push_back(DocumentMessage::Overlays(Operation::SetLayerStyle { path: overlay.clone(), style }.into()).into());
				}
			}
		}
	}
}
