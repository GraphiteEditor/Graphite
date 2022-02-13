use crate::{
	consts::VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE,
	message_prelude::{DocumentMessage, Message},
};

use super::{
	constants::{ControlPointType, ROUNDING_BIAS},
	vector_control_point::VectorControlPoint,
};

use graphene::{LayerId, Operation};

use glam::{DAffine2, DVec2};
use kurbo::{PathEl, Point, Vec2};
use std::collections::VecDeque;

/// VectorAnchor is used to represent an anchor point on the path that can be moved.
/// It contains 0-2 handles that are optionally displayed.
#[derive(PartialEq, Clone, Debug, Default)]
pub struct VectorAnchor {
	// Editable points for the anchor & handles
	pub points: [Option<VectorControlPoint>; 3],
	// The overlays for this handle line rendering
	pub handle_line_overlays: (Option<Vec<LayerId>>, Option<Vec<LayerId>>),

	// Does this anchor point have a path close element?
	pub close_element_id: Option<usize>,
	// Should we maintain the angle between the handles?
	pub handle_mirror_angle: bool,
	// Should we make the handles equidistance from the anchor?
	pub handle_mirror_distance: bool,
}

impl VectorAnchor {
	/// Finds the closest VectorControlPoint owned by this anchor. This can be the handles or the anchor itself
	pub fn closest_point(&self, target: glam::DVec2) -> usize {
		let mut closest_index: usize = 0;
		let mut closest_distance_squared: f64 = f64::MAX; // Not ideal
		for (index, point) in self.points.iter().enumerate() {
			if let Some(point) = point {
				let distance_squared = point.position.distance_squared(target);
				if distance_squared < closest_distance_squared {
					closest_distance_squared = distance_squared;
					closest_index = index;
				}
			}
		}
		closest_index
	}

	// TODO Cleanup the internals of this function
	/// Move the selected points by the provided delta
	pub fn move_selected_points(&mut self, translation: DVec2, relative: bool, path_elements: &mut Vec<kurbo::PathEl>, transform: &DAffine2) {
		let place_mirrored_handle = |center: kurbo::Point, original: kurbo::Point, target: kurbo::Point, selected: bool, mirror_angle: bool, mirror_distance: bool| -> kurbo::Point {
			if !selected || !mirror_angle {
				return original;
			}

			// Keep rotational similarity, but distance variable
			let radius = if mirror_distance { center.distance(target) } else { center.distance(original) };
			let phi = (center - target).atan2();

			kurbo::Point {
				x: radius * phi.cos() + center.x,
				y: radius * phi.sin() + center.y,
			}
		};

		let offset = |point: Point| -> Point {
			if relative {
				let relative = transform.inverse().transform_vector2(translation);
				point + Vec2::new(relative.x, relative.y)
			} else {
				let absolute = transform.inverse().transform_point2(translation);
				Point { x: absolute.x, y: absolute.y }
			}
		};

		for selected_point in self.selected_points() {
			let h1_selected = ControlPointType::Handle1 == selected_point.manipulator_type;
			let h2_selected = ControlPointType::Handle2 == selected_point.manipulator_type;
			let dragging_anchor = !(h1_selected || h2_selected);

			// This section is particularly ugly and could use revision. Kurbo makes it somewhat difficult based on its approach.
			// If neither handle is selected, we are dragging an anchor point
			if dragging_anchor {
				let handle1_exists_and_selected = self.points[ControlPointType::Handle1].is_some() && self.points[ControlPointType::Handle1].as_ref().unwrap().is_selected;
				// Move the anchor point and handle on the same path element
				let selected_element = match &path_elements[selected_point.kurbo_element_id] {
					PathEl::MoveTo(p) => PathEl::MoveTo(offset(*p)),
					PathEl::LineTo(p) => PathEl::LineTo(offset(*p)),
					PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, offset(*p)),
					PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(*a1, if handle1_exists_and_selected { *a2 } else { offset(*a2) }, offset(*p)),
					PathEl::ClosePath => PathEl::ClosePath,
				};

				// Move the handle on the adjacent path element
				if let Some(handle) = &self.points[ControlPointType::Handle2] {
					if !handle.is_selected {
						let neighbor = match &path_elements[handle.kurbo_element_id] {
							PathEl::MoveTo(p) => PathEl::MoveTo(*p),
							PathEl::LineTo(p) => PathEl::LineTo(*p),
							PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p),
							PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(offset(*a1), *a2, *p),
							PathEl::ClosePath => PathEl::ClosePath,
						};
						path_elements[handle.kurbo_element_id] = neighbor;
					}
				}

				if let Some(close_id) = self.close_element_id {
					// Move the invisible point that can be caused by MoveTo / closing the path
					path_elements[close_id] = match &path_elements[close_id] {
						PathEl::MoveTo(p) => PathEl::MoveTo(offset(*p)),
						PathEl::LineTo(p) => PathEl::LineTo(offset(*p)),
						PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, offset(*p)),
						PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(*a1, offset(*a2), offset(*p)),
						PathEl::ClosePath => PathEl::ClosePath,
					};
				}

				path_elements[selected_point.kurbo_element_id] = selected_element;
			}
			// We are dragging a handle
			else {
				let should_mirror_angle = self.handle_mirror_angle;
				let should_mirror_distance = self.handle_mirror_distance;

				// Move the selected handle
				let (selected_element, anchor, selected_handle) = match &path_elements[selected_point.kurbo_element_id] {
					PathEl::MoveTo(p) => (PathEl::MoveTo(*p), *p, *p),
					PathEl::LineTo(p) => (PathEl::LineTo(*p), *p, *p),
					PathEl::QuadTo(a1, p) => (PathEl::QuadTo(offset(*a1), *p), *p, offset(*a1)),
					PathEl::CurveTo(a1, a2, p) => {
						let a1_point = if h2_selected { offset(*a1) } else { *a1 };
						let a2_point = if h1_selected { offset(*a2) } else { *a2 };
						(PathEl::CurveTo(a1_point, a2_point, *p), *p, if h1_selected { a2_point } else { a1_point })
					}
					PathEl::ClosePath => (PathEl::ClosePath, Point::ZERO, Point::ZERO),
				};

				let opposing_handle = self.opposing_handle(selected_point);
				let only_one_handle_selected = !(selected_point.is_selected && opposing_handle.is_some() && opposing_handle.as_ref().unwrap().is_selected);
				// Only move the handles if we don't have both handles selected
				if only_one_handle_selected {
					// Move the opposing handle on the adjacent path element
					if let Some(handle) = opposing_handle {
						let handle_point = transform.inverse().transform_point2(handle.position);
						let handle_point = Point { x: handle_point.x, y: handle_point.y };
						let neighbor = match &path_elements[handle.kurbo_element_id] {
							PathEl::MoveTo(p) => PathEl::MoveTo(*p),
							PathEl::LineTo(p) => PathEl::LineTo(*p),
							PathEl::QuadTo(a1, p) => PathEl::QuadTo(*a1, *p),
							PathEl::CurveTo(a1, a2, p) => PathEl::CurveTo(
								place_mirrored_handle(
									anchor,
									if h1_selected { handle_point } else { *a1 },
									selected_handle,
									h1_selected,
									should_mirror_angle,
									should_mirror_distance,
								),
								place_mirrored_handle(
									*p,
									if h2_selected { handle_point } else { *a2 },
									selected_handle,
									h2_selected,
									should_mirror_angle,
									should_mirror_distance,
								),
								*p,
							),
							PathEl::ClosePath => PathEl::ClosePath,
						};
						path_elements[handle.kurbo_element_id] = neighbor;
					}
				}
				path_elements[selected_point.kurbo_element_id] = selected_element;
			}
		}
	}

	/// Returns true is any points in this anchor are selected
	pub fn is_selected(&self) -> bool {
		self.points.iter().flatten().any(|pnt| pnt.is_selected)
	}

	/// Set a point to selected by ID
	pub fn select_point(&mut self, point_id: usize, selected: bool, responses: &mut VecDeque<Message>) -> Option<&mut VectorControlPoint> {
		if let Some(point) = self.points[point_id].as_mut() {
			point.set_selected(selected, responses);
		}
		self.points[point_id].as_mut()
	}

	/// Clear the selected points for this anchor
	pub fn clear_selected_points(&mut self, responses: &mut VecDeque<Message>) {
		for point in self.points.iter_mut().flatten() {
			point.set_selected(false, responses);
		}
	}

	/// Provides the selected points in this anchor
	pub fn selected_points(&self) -> impl Iterator<Item = &VectorControlPoint> {
		self.points.iter().flatten().filter(|pnt| pnt.is_selected)
	}

	/// Provides mutable selected points in this anchor
	pub fn selected_points_mut(&mut self) -> impl Iterator<Item = &mut VectorControlPoint> {
		self.points.iter_mut().flatten().filter(|pnt| pnt.is_selected)
	}

	/// Angle between handles in radians
	pub fn angle_between_handles(&self) -> f64 {
		if let [Some(a1), Some(h1), Some(h2)] = &self.points {
			return (a1.position - h1.position).angle_between(a1.position - h2.position);
		}
		0.0
	}

	/// Returns the opposing handle to the handle provided
	pub fn opposing_handle(&self, handle: &VectorControlPoint) -> &Option<VectorControlPoint> {
		if let Some(point) = &self.points[ControlPointType::Handle1] {
			if point == handle {
				return &self.points[ControlPointType::Handle2];
			}
		};

		if let Some(point) = &self.points[ControlPointType::Handle2] {
			if point == handle {
				return &self.points[ControlPointType::Handle1];
			}
		};
		&None
	}

	/// Set the mirroring state
	pub fn set_mirroring(&mut self, mirroring: bool) {
		self.handle_mirror_angle = mirroring;
	}

	/// Helper function to more easily set position of VectorControlPoints
	pub fn set_point_position(&mut self, point_index: usize, position: DVec2) {
		if let Some(point) = &mut self.points[point_index] {
			point.position = position;
		}
	}

	/// Updates the position of the anchor based on the kurbo path
	pub fn place_anchor_overlay(&self, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &self.points[ControlPointType::Anchor] {
			if let Some(anchor_overlay) = &anchor_point.overlay_path {
				let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
				let angle = 0.;
				let translation = (anchor_point.position - (scale / 2.) + ROUNDING_BIAS).round();
				let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
				responses.push_back(
					DocumentMessage::Overlays(
						Operation::SetLayerTransformInViewport {
							path: anchor_overlay.clone(),
							transform,
						}
						.into(),
					)
					.into(),
				);
			}
		}
	}

	/// Updates the position of the handle's overlays based on the kurbo path
	pub fn place_handle_overlay(&self, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &self.points[ControlPointType::Anchor] {
			// Helper function to keep things DRY
			let mut place_handle_and_line = |handle: &VectorControlPoint, line: &Option<Vec<LayerId>>| {
				if let Some(line_overlay) = line {
					let line_vector = anchor_point.position - handle.position;
					let scale = DVec2::splat(line_vector.length());
					let angle = -line_vector.angle_between(DVec2::X);
					let translation = (handle.position + ROUNDING_BIAS).round() + DVec2::splat(0.5);
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: line_overlay.clone(),
								transform,
							}
							.into(),
						)
						.into(),
					);
				}

				if let Some(line_overlay) = &handle.overlay_path {
					let scale = DVec2::splat(VECTOR_MANIPULATOR_ANCHOR_MARKER_SIZE);
					let angle = 0.;
					let translation = (handle.position - (scale / 2.) + ROUNDING_BIAS).round();
					let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
					responses.push_back(
						DocumentMessage::Overlays(
							Operation::SetLayerTransformInViewport {
								path: line_overlay.clone(),
								transform,
							}
							.into(),
						)
						.into(),
					);
				}
			};

			let [_, h1, h2] = &self.points;
			let (line1, line2) = &self.handle_line_overlays;

			if let Some(handle) = &h1 {
				place_handle_and_line(handle, line1);
			}

			if let Some(handle) = &h2 {
				place_handle_and_line(handle, line2);
			}
		}
	}

	/// Removes the anchor overlay from the overlay document
	pub fn remove_anchor_overlay(&mut self, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &mut self.points[ControlPointType::Anchor] {
			if let Some(overlay_path) = &anchor_point.overlay_path {
				responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
			}
			anchor_point.overlay_path = None;
		}
	}

	/// Removes the handles overlay from the overlay document
	pub fn remove_handle_overlay(&mut self, responses: &mut VecDeque<Message>) {
		let [_, h1, h2] = &mut self.points;
		let (line1, line2) = &mut self.handle_line_overlays;

		// Helper function to keep things DRY
		let mut delete_message = |handle: &Option<Vec<LayerId>>| {
			if let Some(overlay_path) = handle {
				responses.push_front(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_path.clone() }.into()).into());
			}
		};

		// Delete the handles themselves
		if let Some(handle) = h1 {
			delete_message(&handle.overlay_path);
			handle.overlay_path = None;
		}
		if let Some(handle) = h2 {
			delete_message(&handle.overlay_path);
			handle.overlay_path = None;
		}

		// Delete the handle line layers
		delete_message(line1);
		delete_message(line2);
		self.handle_line_overlays = (None, None);
	}

	/// Clear overlays for this anchor, do this prior to deletion
	pub fn remove_overlays(&mut self, responses: &mut VecDeque<Message>) {
		self.remove_anchor_overlay(responses);
		self.remove_handle_overlay(responses);
	}

	/// Sets the visibility of the anchors overlay
	pub fn set_anchor_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		if let Some(anchor_point) = &self.points[ControlPointType::Anchor] {
			if let Some(overlay_path) = &anchor_point.overlay_path {
				responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
			}
		}
	}

	/// Sets the visibility of the handles overlay
	pub fn set_handle_visiblity(&self, visibility: bool, responses: &mut VecDeque<Message>) {
		let [_, h1, h2] = &self.points;
		let (line1, line2) = &self.handle_line_overlays;

		if let Some(handle) = h1 {
			if let Some(overlay_path) = &handle.overlay_path {
				responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
			}
		}
		if let Some(handle) = h2 {
			if let Some(overlay_path) = &handle.overlay_path {
				responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
			}
		}

		if let Some(overlay_path) = &line1 {
			responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
		}
		if let Some(overlay_path) = &line2 {
			responses.push_back(self.visibility_message(overlay_path.clone(), visibility));
		}
	}

	/// Create a visibility message for an overlay
	fn visibility_message(&self, layer_path: Vec<LayerId>, visibility: bool) -> Message {
		DocumentMessage::Overlays(
			Operation::SetLayerVisibility {
				path: layer_path,
				visible: visibility,
			}
			.into(),
		)
		.into()
	}
}
