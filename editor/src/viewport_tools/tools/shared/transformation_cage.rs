use crate::consts::{BOUNDS_ROTATE_THRESHOLD, BOUNDS_SELECT_THRESHOLD, COLOR_ACCENT, SELECTION_DRAG_ANGLE, VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE};
use crate::document::transformation::OriginalTransforms;
use crate::frontend::utility_types::MouseCursorIcon;
use crate::input::InputPreprocessorMessageHandler;
use crate::message_prelude::*;

use graphene::color::Color;
use graphene::layers::style::{self, Fill, Stroke};
use graphene::Operation;

use glam::{DAffine2, DVec2, Vec2Swizzles};

/// Contains the edges that are being dragged along with the origional bounds
#[derive(Clone, Debug, Default)]
pub struct SelectedEdges {
	bounds: [DVec2; 2],
	top: bool,
	bottom: bool,
	left: bool,
	right: bool,
}

impl SelectedEdges {
	pub fn new(top: bool, bottom: bool, left: bool, right: bool, bounds: [DVec2; 2]) -> Self {
		Self { top, bottom, left, right, bounds }
	}

	/// Calculate the pivot for the operation (the opposite point to the edge dragged)
	pub fn calculate_pivot(&self) -> DVec2 {
		let min = self.bounds[0];
		let max = self.bounds[1];

		let x = if self.left {
			max.x
		} else if self.right {
			min.x
		} else {
			(min.x + max.x) / 2.
		};

		let y = if self.top {
			max.y
		} else if self.bottom {
			min.y
		} else {
			(min.y + max.y) / 2.
		};

		DVec2::new(x, y)
	}

	/// Computes the new bounds with the given mouse move and modifier keys
	pub fn new_size(&self, mouse: DVec2, transform: DAffine2, center: bool, constrain: bool) -> [DVec2; 2] {
		let mouse = transform.inverse().transform_point2(mouse);

		let mut min = self.bounds[0];
		let mut max = self.bounds[1];
		if self.top {
			min.y = mouse.y;
		} else if self.bottom {
			max.y = mouse.y;
		}
		if self.left {
			min.x = mouse.x
		} else if self.right {
			max.x = mouse.x;
		}

		let mut size = max - min;
		if constrain && ((self.top || self.bottom) && (self.left || self.right)) {
			size = size.abs().max(size.abs().yx()) * size.signum();
		}
		if center {
			if self.left || self.right {
				size.x *= 2.;
			}

			if self.bottom || self.top {
				size.y *= 2.;
			}
		}

		[min, size]
	}

	/// Offsets the transformation pivot in order to scale from the center
	fn offset_pivot(&self, center: bool, size: DVec2) -> DVec2 {
		let mut offset = DVec2::ZERO;

		if center && self.right {
			offset.x -= size.x / 2.;
		}
		if center && self.left {
			offset.x += size.x / 2.;
		}
		if center && self.bottom {
			offset.y -= size.y / 2.;
		}
		if center && self.top {
			offset.y += size.y / 2.;
		}
		offset
	}

	/// Moves the position to account for centring (only necessary with absolute transforms - e.g. with artboards)
	pub fn center_position(&self, mut position: DVec2, size: DVec2, center: bool) -> DVec2 {
		if center && self.right {
			position.x -= size.x / 2.;
		}
		if center && self.bottom {
			position.y -= size.y / 2.;
		}

		position
	}

	/// Calculates the required scaling to resize the bounding box
	pub fn bounds_to_scale_transform(&self, center: bool, size: DVec2) -> DAffine2 {
		DAffine2::from_translation(self.offset_pivot(center, size)) * DAffine2::from_scale(size / (self.bounds[1] - self.bounds[0]))
	}
}

/// Create a viewport relative bounding box overlay with no transform handles
pub fn add_bounding_box(responses: &mut Vec<Message>) -> Vec<LayerId> {
	let path = vec![generate_uuid()];

	let operation = Operation::AddOverlayRect {
		path: path.clone(),
		transform: DAffine2::ZERO.to_cols_array(),
		style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), None),
	};
	responses.push(DocumentMessage::Overlays(operation.into()).into());

	path
}

/// Add the transform handle overlay
fn add_transform_handles(responses: &mut Vec<Message>) -> [Vec<LayerId>; 8] {
	const EMPTY_VEC: Vec<LayerId> = Vec::new();
	let mut transform_handle_paths = [EMPTY_VEC; 8];

	for item in &mut transform_handle_paths {
		let current_path = vec![generate_uuid()];

		let operation = Operation::AddOverlayRect {
			path: current_path.clone(),
			transform: DAffine2::ZERO.to_cols_array(),
			style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 2.0)), Some(Fill::new(Color::WHITE))),
		};
		responses.push(DocumentMessage::Overlays(operation.into()).into());

		*item = current_path;
	}

	transform_handle_paths
}

/// Converts a bounding box to a rounded transform (with translation and scale)
pub fn transform_from_box(pos1: DVec2, pos2: DVec2, transform: DAffine2) -> DAffine2 {
	let inverse = transform.inverse();
	transform
		* DAffine2::from_scale_angle_translation(
			inverse.transform_vector2(transform.transform_vector2(pos2 - pos1).round()),
			0.,
			inverse.transform_point2(transform.transform_point2(pos1).round() - DVec2::splat(0.5)),
		)
}

/// Aligns the mouse position to the closest axis
pub fn axis_align_drag(axis_align: bool, position: DVec2, start: DVec2) -> DVec2 {
	if axis_align {
		let mouse_position = position - start;
		let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
		let angle = -mouse_position.angle_between(DVec2::X);
		let snapped_angle = (angle / snap_resolution).round() * snap_resolution;
		DVec2::new(snapped_angle.cos(), snapped_angle.sin()) * mouse_position.length() + start
	} else {
		position
	}
}

/// Contains info on the overlays for the bounding box and transform handles
#[derive(Clone, Debug, Default)]
pub struct BoundingBoxOverlays {
	pub bounding_box: Vec<LayerId>,
	pub transform_handles: [Vec<LayerId>; 8],
	pub bounds: [DVec2; 2],
	pub transform: DAffine2,
	pub selected_edges: Option<SelectedEdges>,
	pub original_transforms: OriginalTransforms,
	pub pivot: DVec2,
}

impl BoundingBoxOverlays {
	#[must_use]
	pub fn new(buffer: &mut Vec<Message>) -> Self {
		Self {
			bounding_box: add_bounding_box(buffer),
			transform_handles: add_transform_handles(buffer),
			..Default::default()
		}
	}

	/// Calculats the transformed handle positions based on the bounding box and the transform
	pub fn evaluate_transform_handle_positions(&self) -> [DVec2; 8] {
		let (left, top): (f64, f64) = self.bounds[0].into();
		let (right, bottom): (f64, f64) = self.bounds[1].into();
		[
			self.transform.transform_point2(DVec2::new(left, top)),
			self.transform.transform_point2(DVec2::new(left, (top + bottom) / 2.)),
			self.transform.transform_point2(DVec2::new(left, bottom)),
			self.transform.transform_point2(DVec2::new((left + right) / 2., top)),
			self.transform.transform_point2(DVec2::new((left + right) / 2., bottom)),
			self.transform.transform_point2(DVec2::new(right, top)),
			self.transform.transform_point2(DVec2::new(right, (top + bottom) / 2.)),
			self.transform.transform_point2(DVec2::new(right, bottom)),
		]
	}

	/// Update the position of the bounding box and transform handles
	pub fn transform(&mut self, buffer: &mut Vec<Message>) {
		let transform = transform_from_box(self.bounds[0], self.bounds[1], self.transform).to_cols_array();
		let path = self.bounding_box.clone();
		buffer.push(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path, transform }.into()).into());

		// Helps push values that end in approximately half, plus or minus some floating point imprecision, towards the same side of the round() function
		const BIAS: f64 = 0.0001;

		for (position, path) in self.evaluate_transform_handle_positions().into_iter().zip(&self.transform_handles) {
			let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
			let translation = (position - (scale / 2.) - 0.5 + BIAS).round();
			let transform = DAffine2::from_scale_angle_translation(scale, 0., translation).to_cols_array();
			let path = path.clone();
			buffer.push(DocumentMessage::Overlays(Operation::SetLayerTransformInViewport { path, transform }.into()).into());
		}
	}

	/// Check if the user has selected the edge for dragging (returns which edge in order top, bottom, left, right)
	pub fn check_selected_edges(&self, cursor: DVec2) -> Option<(bool, bool, bool, bool)> {
		let cursor = self.transform.inverse().transform_point2(cursor);
		let select_threshold = self.transform.inverse().transform_vector2(DVec2::new(0., BOUNDS_SELECT_THRESHOLD)).length();

		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);
		if min.x - cursor.x < select_threshold && min.y - cursor.y < select_threshold && cursor.x - max.x < select_threshold && cursor.y - max.y < select_threshold {
			let mut top = (cursor.y - min.y).abs() < select_threshold;
			let mut bottom = (max.y - cursor.y).abs() < select_threshold;
			let mut left = (cursor.x - min.x).abs() < select_threshold;
			let mut right = (max.x - cursor.x).abs() < select_threshold;
			if cursor.y - min.y + max.y - cursor.y < select_threshold * 2. && (left || right) {
				top = false;
				bottom = false;
			}
			if cursor.x - min.x + max.x - cursor.x < select_threshold * 2. && (top || bottom) {
				left = false;
				right = false;
			}

			if top || bottom || left || right {
				return Some((top, bottom, left, right));
			}
		}

		None
	}

	/// Check if the user is rotating with the bounds
	pub fn check_rotate(&self, cursor: DVec2) -> bool {
		let cursor = self.transform.inverse().transform_point2(cursor);
		let rotate_threshold = self.transform.inverse().transform_vector2(DVec2::new(0., BOUNDS_ROTATE_THRESHOLD)).length();

		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);

		let outside_bounds = (min.x > cursor.x || cursor.x > max.x) || (min.y > cursor.y || cursor.y > max.y);
		let inside_extended_bounds = min.x - cursor.x < rotate_threshold && min.y - cursor.y < rotate_threshold && cursor.x - max.x < rotate_threshold && cursor.y - max.y < rotate_threshold;

		outside_bounds & inside_extended_bounds
	}

	/// Gets the required mouse cursor to show resizing bounds or optionally rotation
	pub fn get_cursor(&self, input: &InputPreprocessorMessageHandler, rotate: bool) -> MouseCursorIcon {
		if let Some(directions) = self.check_selected_edges(input.mouse.position) {
			match directions {
				(true, false, false, false) | (false, true, false, false) => MouseCursorIcon::NSResize,
				(false, false, true, false) | (false, false, false, true) => MouseCursorIcon::EWResize,
				(true, false, true, false) | (false, true, false, true) => MouseCursorIcon::NWSEResize,
				(true, false, false, true) | (false, true, true, false) => MouseCursorIcon::NESWResize,
				_ => MouseCursorIcon::Default,
			}
		} else if rotate && self.check_rotate(input.mouse.position) {
			MouseCursorIcon::Grabbing
		} else {
			MouseCursorIcon::Default
		}
	}

	/// Removes the overlays
	pub fn delete(self, buffer: &mut impl Extend<Message>) {
		buffer.extend([DocumentMessage::Overlays(Operation::DeleteLayer { path: self.bounding_box }.into()).into()]);
		buffer.extend(
			self.transform_handles
				.iter()
				.map(|path| DocumentMessage::Overlays(Operation::DeleteLayer { path: path.clone() }.into()).into()),
		);
	}
}
