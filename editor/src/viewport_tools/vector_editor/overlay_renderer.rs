use std::collections::{HashMap, VecDeque};

use glam::{DAffine2, DVec2};
use kurbo::BezPath;

use crate::{
	consts::{COLOR_ACCENT, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE},
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

use super::{
	constants::{ControlPointType, ROUNDING_BIAS},
	vector_anchor::VectorAnchor,
	vector_control_point::VectorControlPoint,
	vector_shape::VectorShape,
};
use graphene::{
	color::Color,
	layers::style::{self, Fill, Stroke},
	LayerId, Operation,
};

/// AnchorOverlay is the collection of overlays that make up an anchor
/// Notably the anchor point, handles and the lines for the handles
type AnchorOverlays = [Option<Vec<LayerId>>; 5];
type AnchorId = u64;

const POINT_STROKE_WIDTH: f32 = 2.0;

#[derive(Debug, Default)]
pub struct OverlayRenderer {
	shape_overlay_cache: HashMap<Vec<LayerId>, Vec<LayerId>>,
	anchor_overlay_cache: HashMap<AnchorId, AnchorOverlays>,
}

impl<'a> OverlayRenderer {
	pub fn new() -> Self {
		OverlayRenderer {
			anchor_overlay_cache: HashMap::new(),
			shape_overlay_cache: HashMap::new(),
		}
	}

	pub fn draw_overlays_for_vector_shape(&mut self, shape: &VectorShape, responses: &mut VecDeque<Message>) {
		// Draw the shape outline overlays
		if !self.shape_overlay_cache.contains_key(&shape.layer_path) {
			let outline = self.create_shape_outline_overlay(shape.into(), responses);

			// Cache outline overlay
			self.shape_overlay_cache.insert(shape.layer_path.clone(), outline);

			// TODO Handle removing shapes from cache so we don't memory leak
		}

		// Draw the anchor / handle overlays
		for (anchor_id, anchor) in shape.anchors.enumerate() {
			// If cached update them
			if let Some(anchor_overlays) = self.anchor_overlay_cache.get(anchor_id) {
				// Reposition cached overlays
				self.place_overlays(anchor, anchor_overlays, responses);

				// Change styles to reflect selection
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

				// Place the new overlays
				self.place_overlays(anchor, &anchor_overlays, responses);

				// Change styles to reflect selection
				self.style_overlays(anchor, &anchor_overlays, responses);

				// Cache overlays
				self.anchor_overlay_cache.insert(*anchor_id, anchor_overlays);
			}

			// TODO handle unused overlays
		}
	}

	pub fn clear_overlays_for_vector_shape(&mut self, shape: &VectorShape, responses: &mut VecDeque<Message>) {
		// Remove the shape outline overlays
		if let Some(outline) = self.shape_overlay_cache.get(&shape.layer_path) {
			self.remove_outline_overlays(outline.clone(), responses)
		}
	}
	// TODO add a way of updating overlays without destroying them and re-creating them

	/// Create the kurbo shape that matches the selected viewport shape
	fn create_shape_outline_overlay(&self, bez_path: BezPath, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
		let layer_path = vec![generate_uuid()];
		let operation = Operation::AddOverlayShape {
			path: layer_path.clone(),
			bez_path,
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
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
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
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
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
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
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
		};
		responses.push_front(DocumentMessage::Overlays(operation.into()).into());

		Some(layer_path)
	}

	/// Updates the position of the overlays based on the VectorShape points
	fn place_overlays(&self, anchor: &VectorAnchor, overlays: &AnchorOverlays, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &anchor.points[ControlPointType::Anchor] {
			// Helper function to keep things DRY
			let mut place_handle_and_line = |handle: &VectorControlPoint, line: &Option<Vec<LayerId>>| {
				if let Some(line_overlay) = line {
					let line_vector = anchor_point.position - handle.position;
					let scale = DVec2::splat(line_vector.length());
					let angle = -line_vector.angle_between(DVec2::X);
					let translation = (handle.position + ROUNDING_BIAS).round() + DVec2::splat(0.5);
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(self.overlay_transform_message(line_overlay.clone(), transform));
				}

				if let Some(line_overlay) = &overlays[handle.manipulator_type as usize] {
					let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
					let angle = 0.;
					let translation = (handle.position - (scale / 2.) + ROUNDING_BIAS).round();
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
				let translation = (anchor_point.position - (scale / 2.) + ROUNDING_BIAS).round();
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
				let message = self.overlay_transform_message(anchor_overlay.clone(), transform);
				responses.push_back(message);
			}
		}
	}

	/// Removes the anchor / handle overlays from the overlay document
	fn remove_anchor_overlays(&mut self, overlay_paths: &AnchorOverlays, responses: &mut VecDeque<Message>) {
		overlay_paths.iter().flatten().for_each(|layer_id| {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: layer_id.clone() }.into()).into());
		});
	}

	fn remove_outline_overlays(&mut self, overlay_path: Vec<LayerId>, responses: &mut VecDeque<Message>) {
		responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path }.into()).into());
	}
	/// Sets the visibility of the handles overlay
	fn set_overlay_visiblity(&self, anchor_overlays: &AnchorOverlays, visibility: bool, responses: &mut VecDeque<Message>) {
		anchor_overlays.iter().flatten().for_each(|layer_id| {
			responses.push_back(self.overlay_visibility_message(layer_id.clone(), visibility));
		});
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
		let selected_style = style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, POINT_STROKE_WIDTH + 1.0)), Some(Fill::new(COLOR_ACCENT)));
		let deselected_style = style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, POINT_STROKE_WIDTH)), None);

		// Update if the anchor / handle points are shown as selected
		// Here the index is important, even though overlays[..] has five elements we only care about the first three
		for (index, point) in anchor.points.iter().enumerate() {
			if let Some(point) = point {
				if let Some(overlay) = &overlays[index] {
					let style = if point.is_selected { selected_style } else { deselected_style };
					responses.push_back(DocumentMessage::Overlays(Operation::SetLayerStyle { path: overlay.clone(), style }.into()).into());
				}
			}
		}
	}
}
