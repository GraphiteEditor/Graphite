use std::collections::{HashMap, VecDeque};

use glam::{DAffine2, DVec2};
use kurbo::BezPath;

use crate::{
	consts::{COLOR_ACCENT, PATH_OUTLINE_WEIGHT, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE},
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

use super::{
	constants::{ControlPointType, ROUNDING_BIAS},
	vector_anchor::VectorAnchor,
	vector_control_point::VectorControlPoint,
};
use graphene::{
	color::Color,
	document::Document,
	layers::style::{self, Fill, Stroke},
	LayerId, Operation,
};

/// AnchorOverlay is the collection of overlays that make up an anchor
/// Notably the anchor point, handles and the lines for the handles
type AnchorOverlays = [Option<Vec<LayerId>>; 5];
type AnchorId = u64;

const POINT_STROKE_WEIGHT: f64 = 2.;

#[derive(Debug, Default)]
pub struct OverlayRenderer {
	shape_overlay_cache: HashMap<LayerId, Vec<LayerId>>,
	anchor_overlay_cache: HashMap<(LayerId, AnchorId), AnchorOverlays>,
}

impl<'a> OverlayRenderer {
	pub fn new() -> Self {
		OverlayRenderer {
			anchor_overlay_cache: HashMap::new(),
			shape_overlay_cache: HashMap::new(),
		}
	}

	// TODO Implement a trait called HasOverlays that is implemented by Vectorshape
	pub fn render_vector_shape_overlays(&mut self, document: &Document, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		let transform = document.generate_transform_relative_to_viewport(&layer_path).ok().unwrap();

		if let Ok(layer) = document.layer(&layer_path) {
			let layer_id = layer_path.last().unwrap();
			// log::debug!("Layer id is {:?}", layer_id);
			self.layer_overlay_visibility(document, layer_path.clone(), true, responses);

			if let Some(shape) = layer.as_vector_shape() {
				let outline_cache = self.shape_overlay_cache.get(layer_id);

				// Create an outline if we do not have a cached one
				if outline_cache == None {
					let outline = self.create_shape_outline_overlay(shape.into(), responses);
					self.place_outline_overlays(outline.clone(), &transform, responses);
					self.shape_overlay_cache.insert(*layer_id, outline);
				// log::debug!("Creating new overlays for {:?}", layer_path);
				} else if let Some(outline) = outline_cache {
					self.place_outline_overlays(outline.clone(), &transform, responses);
				}

				// Create, place and style the anchor / handle overlays
				for (anchor_id, anchor) in shape.anchors().enumerate() {
					let anchor_cache = self.anchor_overlay_cache.get(&(*layer_id, *anchor_id));

					// If cached update placement and style
					if let Some(anchor_overlays) = anchor_cache {
						self.place_anchor_overlays(anchor, anchor_overlays, &transform, responses);
						self.style_overlays(anchor, anchor_overlays, responses);
					} else {
						// Create if not cached
						let anchor_overlays = [
							Some(self.create_anchor_overlay(responses)),
							self.create_handle_overlay(&anchor.points[ControlPointType::Handle1], responses),
							self.create_handle_overlay(&anchor.points[ControlPointType::Handle2], responses),
							self.create_handle_line_overlay(&anchor.points[ControlPointType::Handle1], responses),
							self.create_handle_line_overlay(&anchor.points[ControlPointType::Handle2], responses),
						];
						self.place_anchor_overlays(anchor, &anchor_overlays, &transform, responses);
						self.style_overlays(anchor, &anchor_overlays, responses);
						self.anchor_overlay_cache.insert((*layer_id, *anchor_id), anchor_overlays);
					}
				}
				// TODO Handle removing shapes from cache so we don't memory leak
				// Eventually will get replaced with am immediate mode renderer for overlays
			}
		}
	}

	pub fn clear_vector_shape_overlays(&mut self, document: &Document, layer_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		let layer_id = layer_path.last().unwrap();

		// Remove the shape outline overlays
		if let Some(overlay_path) = self.shape_overlay_cache.get(layer_id) {
			self.remove_outline_overlays(overlay_path.clone(), responses)
		}
		self.shape_overlay_cache.remove(layer_id);

		// Remove the anchor overlays
		if let Ok(layer) = document.layer(&layer_path) {
			if let Some(shape) = layer.as_vector_shape() {
				for anchor_id in shape.anchors().keys() {
					if let Some(anchor_overlays) = self.anchor_overlay_cache.get(&(*layer_id, *anchor_id)) {
						self.remove_anchor_overlays(anchor_overlays, responses);
						self.anchor_overlay_cache.remove(&(*layer_id, *anchor_id));
					}
				}
			}
		}
	}

	pub fn layer_overlay_visibility(&mut self, document: &Document, layer_path: Vec<LayerId>, visibility: bool, responses: &mut VecDeque<Message>) {
		let layer_id = layer_path.last().unwrap();

		// Hide the shape outline overlays
		if let Some(overlay_path) = self.shape_overlay_cache.get(layer_id) {
			self.set_outline_overlay_visibility(overlay_path.clone(), visibility, responses);
		}

		// Hide the anchor overlays
		if let Ok(layer) = document.layer(&layer_path) {
			if let Some(shape) = layer.as_vector_shape() {
				for anchor_id in shape.anchors().keys() {
					if let Some(anchor_overlays) = self.anchor_overlay_cache.get(&(*layer_id, *anchor_id)) {
						self.set_anchor_overlay_visibility(anchor_overlays, visibility, responses);
					}
				}
			}
		}
	}

	/// Create the kurbo shape that matches the selected viewport shape
	fn create_shape_outline_overlay(&self, bez_path: BezPath, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayShape {
			path: layer_path.clone(),
			bez_path,
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, PATH_OUTLINE_WEIGHT)), Fill::None),
			closed: false,
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());

		layer_path
	}

	/// Create a single anchor overlay and return its layer id
	fn create_anchor_overlay(&self, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayRect {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Fill::solid(Color::WHITE)),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		layer_path
	}

	/// Create a single handle overlay and return its layer id
	fn create_handle_overlay(&self, handle: &Option<VectorControlPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		if handle.is_none() {
			return None;
		}

		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayEllipse {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Fill::solid(Color::WHITE)),
		};
		responses.push_back(DocumentMessage::Overlays(operation.into()).into());
		Some(layer_path)
	}

	/// Create the shape outline overlay and return its layer id
	fn create_handle_line_overlay(&self, handle: &Option<VectorControlPoint>, responses: &mut VecDeque<Message>) -> Option<Vec<LayerId>> {
		if handle.is_none() {
			return None;
		}

		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayLine {
			path: layer_path.clone(),
			transform: DAffine2::IDENTITY.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), Fill::None),
		};
		responses.push_front(DocumentMessage::Overlays(operation.into()).into());

		Some(layer_path)
	}

	fn place_outline_overlays(&self, outline_path: Vec<LayerId>, parent_transform: &DAffine2, responses: &mut VecDeque<Message>) {
		let message = self.overlay_transform_message(outline_path, parent_transform.to_cols_array());
		responses.push_back(message);
	}

	/// Updates the position of the overlays based on the VectorShape points
	fn place_anchor_overlays(&self, anchor: &VectorAnchor, overlays: &AnchorOverlays, parent_transform: &DAffine2, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &anchor.points[ControlPointType::Anchor] {
			// Helper function to keep things DRY
			let mut place_handle_and_line = |handle: &VectorControlPoint, line: &Option<Vec<LayerId>>| {
				if let Some(line_overlay) = line {
					let line_vector = parent_transform.transform_point2(anchor_point.position) - parent_transform.transform_point2(handle.position);
					let scale = DVec2::splat(line_vector.length());
					let angle = -line_vector.angle_between(DVec2::X);
					let translation = (parent_transform.transform_point2(handle.position) + ROUNDING_BIAS).round() + DVec2::splat(0.5);
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(self.overlay_transform_message(line_overlay.clone(), transform));
				}

				if let Some(line_overlay) = &overlays[handle.manipulator_type as usize] {
					let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
					let angle = 0.;
					let translation = (parent_transform.transform_point2(handle.position) - (scale / 2.) + ROUNDING_BIAS).round();
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(self.overlay_transform_message(line_overlay.clone(), transform));
				}
			};

			// Place the handle overlays
			let [_, h1, h2] = &anchor.points;
			let [_, _, _, line1, line2] = &overlays;
			if let Some(handle) = &h1 {
				place_handle_and_line(handle, line1);
			}
			if let Some(handle) = &h2 {
				place_handle_and_line(handle, line2);
			}

			// Place the anchor point overlay
			if let Some(anchor_overlay) = &overlays[ControlPointType::Anchor as usize] {
				let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
				let angle = 0.;
				let translation = (parent_transform.transform_point2(anchor_point.position) - (scale / 2.) + ROUNDING_BIAS).round();
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();

				let message = self.overlay_transform_message(anchor_overlay.clone(), transform);
				responses.push_back(message);
			}
		}
	}

	/// Removes the anchor / handle overlays from the overlay document
	fn remove_anchor_overlays(&self, overlay_paths: &AnchorOverlays, responses: &mut VecDeque<Message>) {
		overlay_paths.iter().flatten().for_each(|layer_id| {
			// log::debug!("Sending delete message for: {:?}", layer_id);
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer_id.clone() }.into()).into());
		});
	}

	fn remove_outline_overlays(&self, overlay_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path }.into()).into());
	}

	/// Sets the visibility of the handles overlay
	fn set_anchor_overlay_visibility(&self, anchor_overlays: &AnchorOverlays, visibility: bool, responses: &mut VecDeque<Message>) {
		anchor_overlays.iter().flatten().for_each(|layer_id| {
			responses.push_back(self.overlay_visibility_message(layer_id.clone(), visibility));
		});
	}

	fn set_outline_overlay_visibility(&self, overlay_path: Vec<LayerId>, visibility: bool, responses: &mut VecDeque<Message>) {
		responses.push_back(self.overlay_visibility_message(overlay_path, visibility));
	}

	/// Create a visibility message for an overlay
	fn overlay_visibility_message(&self, layer_path: Vec<LayerId>, visibility: bool) -> Message {
		DocumentMessage::Overlays(
			Operation::SetLayerVisibility {
				path: layer_path,
				visible: visibility,
			}
			.into(),
		)
		.into()
	}

	/// Create a transform message for an overlay
	fn overlay_transform_message(&self, layer_path: Vec<LayerId>, transform: [f64; 6]) -> Message {
		DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path: layer_path, transform }.into()).into()
	}

	/// Sets the overlay style for this point
	fn style_overlays(&self, anchor: &VectorAnchor, overlays: &AnchorOverlays, responses: &mut VecDeque<Message>) {
		// TODO Move the style definitions out of the VectorShape, should be looked up from a stylesheet or similar
		let selected_style = style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, POINT_STROKE_WEIGHT + 1.0)), Fill::solid(COLOR_ACCENT));
		let deselected_style = style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, POINT_STROKE_WEIGHT)), Fill::solid(Color::WHITE));

		// Update if the anchor / handle points are shown as selected
		// Here the index is important, even though overlays[..] has five elements we only care about the first three
		for (index, point) in anchor.points.iter().enumerate() {
			if let Some(point) = point {
				if let Some(overlay) = &overlays[index] {
					let style = if point.is_selected { selected_style.clone() } else { deselected_style.clone() };
					responses.push_back(DocumentMessage::Overlays(Operation::SetLayerStyle { path: overlay.clone(), style }.into()).into());
				}
			}
		}
	}
}
