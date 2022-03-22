use std::collections::{HashMap, VecDeque};

use glam::DAffine2;
use kurbo::BezPath;

use crate::{
	consts::COLOR_ACCENT,
	message_prelude::{generate_uuid, DocumentMessage, Message},
};

use super::{constants::ControlPointType, vector_anchor::VectorAnchor, vector_control_point::VectorControlPoint, vector_shape::VectorShape};
use graphene::{
	color::Color,
	layers::style::{self, Fill, Stroke},
	LayerId, Operation,
};

struct OverlayRenderer {
	overlays: HashMap<VectorShape, Vec<Vec<LayerId>>>,
}

/// AnchorOverlay is the collection of overlays that make up an anchor
/// Notably the anchor point, the lines to the handles and the handles
type AnchorOverlays = Vec<[Option<Vec<LayerId>>; 5]>;

impl OverlayRenderer {
	pub fn new() -> Self {
		OverlayRenderer { overlays: HashMap::new() }
	}

	pub fn draw_overlays_for_shape(&mut self, shape: &VectorShape, responses: &mut VecDeque<Message>) {
		let outline = self.create_shape_outline_overlay(shape.to_bezpath(), responses);
		let anchors: AnchorOverlays = shape
			.anchors
			.iter()
			.map(|anchor| {
				[
					Some(self.create_anchor_overlay(anchor, responses)),
					self.create_handle_overlay(&anchor.points[ControlPointType::Handle1], responses),
					self.create_handle_overlay(&anchor.points[ControlPointType::Handle2], responses),
					self.create_handle_line_overlay(&anchor.points[ControlPointType::Handle1], responses),
					self.create_handle_line_overlay(&anchor.points[ControlPointType::Handle2], responses),
				]
			})
			.collect::<_>();
	}

	pub fn hide_overlays_for_shape(&mut self, shape: &VectorShape, responses: &mut VecDeque<Message>) {
		// Delete here
	}

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
	fn create_anchor_overlay(&self, anchor: &VectorAnchor, responses: &mut VecDeque<Message>) -> Vec<LayerId> {
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
}
