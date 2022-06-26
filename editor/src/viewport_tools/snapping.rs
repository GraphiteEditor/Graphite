use crate::consts::{
	COLOR_ACCENT, SNAP_AXIS_OVERLAY_FADE_DISTANCE, SNAP_AXIS_TOLERANCE, SNAP_AXIS_UNSNAPPED_OPACITY, SNAP_POINT_OVERLAY_FADE_FAR, SNAP_POINT_OVERLAY_FADE_NEAR, SNAP_POINT_SIZE, SNAP_POINT_TOLERANCE,
	SNAP_POINT_UNSNAPPED_OPACITY,
};
use crate::document::DocumentMessageHandler;
use crate::message_prelude::*;

use graphene::layers::layer_info::{Layer, LayerDataType};
use graphene::layers::style::{self, Stroke};
use graphene::layers::vector::constants::ControlPointType;
use graphene::{LayerId, Operation};

use glam::{DAffine2, DVec2};
use std::f64::consts::PI;

// Handles snap overlays
#[derive(Debug, Clone, Default)]
struct SnapOverlays {
	axis_overlay_paths: Vec<Vec<LayerId>>,
	point_overlay_paths: Vec<Vec<LayerId>>,
	axis_index: usize,
	point_index: usize,
}

/// Handles snapping and snap overlays
#[derive(Debug, Clone, Default)]
pub struct SnapHandler {
	point_targets: Option<Vec<DVec2>>,
	bound_targets: Option<Vec<DVec2>>,
	snap_overlays: SnapOverlays,
	snap_x: bool,
	snap_y: bool,
}

/// Converts a bounding box into a set of points for snapping
///
/// Puts a point in the middle of each edge (top, bottom, left, right)
pub fn expand_bounds([bound1, bound2]: [DVec2; 2]) -> [DVec2; 4] {
	[
		DVec2::new((bound1.x + bound2.x) / 2., bound1.y),
		DVec2::new((bound1.x + bound2.x) / 2., bound2.y),
		DVec2::new(bound1.x, (bound1.y + bound2.y) / 2.),
		DVec2::new(bound2.x, (bound1.y + bound2.y) / 2.),
	]
}

impl SnapOverlays {
	/// Draws an overlay (axis or point) with the correct transform and fade opacity, reusing lines from the pool if available.
	fn add_overlay(is_axis: bool, responses: &mut VecDeque<Message>, transform: [f64; 6], opacity: Option<f64>, index: usize, overlay_paths: &mut Vec<Vec<LayerId>>) {
		// If there isn't one in the pool to ruse, add a new alignment line to the pool with the intended transform
		let layer_path = if index >= overlay_paths.len() {
			let layer_path = vec![generate_uuid()];
			responses.push_back(
				DocumentMessage::Overlays(
					if is_axis {
						Operation::AddLine {
							path: layer_path.clone(),
							transform,
							style: style::PathStyle::new(Some(Stroke::new(COLOR_ACCENT, 1.0)), style::Fill::None),
							insert_index: -1,
						}
					} else {
						Operation::AddOverlayEllipse {
							path: layer_path.clone(),
							transform,
							style: style::PathStyle::new(None, style::Fill::Solid(COLOR_ACCENT)),
						}
					}
					.into(),
				)
				.into(),
			);
			overlay_paths.push(layer_path.clone());
			layer_path
		}
		// Otherwise, reuse an overlay from the pool and update its new transform
		else {
			let layer_path = overlay_paths[index].clone();
			responses.push_back(DocumentMessage::Overlays(Operation::SetLayerTransform { path: layer_path.clone(), transform }.into()).into());
			layer_path
		};

		// Then set its opacity to the fade amount
		if let Some(opacity) = opacity {
			responses.push_back(DocumentMessage::Overlays(Operation::SetLayerOpacity { path: layer_path, opacity }.into()).into());
		}
	}

	/// Draw the alignment lines for an axis
	/// Note: horizontal refers to the overlay line being horizontal and the snap being along the Y axis
	fn draw_alignment_lines(&mut self, is_horizontal: bool, distances: impl Iterator<Item = (DVec2, DVec2, f64)>, responses: &mut VecDeque<Message>, closest_distance: DVec2) {
		for (target, goal, distance) in distances.filter(|(_target, _pos, dist)| dist.abs() < SNAP_AXIS_OVERLAY_FADE_DISTANCE) {
			let offset = if is_horizontal { target.y } else { target.x }.round() - 0.5;
			let offset_other = if is_horizontal { target.x } else { target.y }.round() - 0.5;
			let goal_axis = if is_horizontal { goal.x } else { goal.y }.round() - 0.5;

			let scale = DVec2::new(offset_other - goal_axis, 1.);
			let angle = if is_horizontal { 0. } else { PI / 2. };
			let translation = if is_horizontal { DVec2::new(goal_axis, offset) } else { DVec2::new(offset, goal_axis) };

			let transform = DAffine2::from_scale_angle_translation(scale, angle, translation).to_cols_array();
			let closest = if is_horizontal { closest_distance.y } else { closest_distance.x };

			let opacity = if (closest - distance).abs() < 1. {
				1.
			} else {
				SNAP_AXIS_UNSNAPPED_OPACITY - distance.abs() / (SNAP_AXIS_OVERLAY_FADE_DISTANCE / SNAP_AXIS_UNSNAPPED_OPACITY)
			};

			// Add line
			Self::add_overlay(true, responses, transform, Some(opacity), self.axis_index, &mut self.axis_overlay_paths);
			self.axis_index += 1;

			let size = DVec2::splat(SNAP_POINT_SIZE);

			// Add point at target
			let transform = DAffine2::from_scale_angle_translation(size, 0., target - size / 2.).to_cols_array();
			Self::add_overlay(false, responses, transform, Some(opacity), self.point_index, &mut self.point_overlay_paths);
			self.point_index += 1;

			// Add point along line but towards goal
			let translation = if is_horizontal { DVec2::new(goal.x, target.y) } else { DVec2::new(target.x, goal.y) };
			let transform = DAffine2::from_scale_angle_translation(size, 0., translation - size / 2.).to_cols_array();
			Self::add_overlay(false, responses, transform, Some(opacity), self.point_index, &mut self.point_overlay_paths);
			self.point_index += 1
		}
	}

	/// Draw the snap points
	fn draw_snap_points(&mut self, distances: impl Iterator<Item = (DVec2, DVec2, f64)>, responses: &mut VecDeque<Message>, closest_distance: DVec2) {
		for (target, offset, distance) in distances.filter(|(_pos, _offset, dist)| dist.abs() < SNAP_POINT_OVERLAY_FADE_FAR) {
			let active = (closest_distance - offset).length_squared() < 1.;

			if active {
				continue;
			}

			let opacity = (1. - (distance - SNAP_POINT_OVERLAY_FADE_NEAR) / (SNAP_POINT_OVERLAY_FADE_FAR - SNAP_POINT_OVERLAY_FADE_NEAR)).min(1.) / SNAP_POINT_UNSNAPPED_OPACITY;

			let size = DVec2::splat(SNAP_POINT_SIZE);
			let transform = DAffine2::from_scale_angle_translation(size, 0., target - size / 2.).to_cols_array();
			Self::add_overlay(false, responses, transform, Some(opacity), self.point_index, &mut self.point_overlay_paths);
			self.point_index += 1
		}
	}

	/// Updates the snapping overlays with the specified distances.
	/// `positions_and_distances` is a tuple of `x`, `y` & `point` iterators,, each with `(position, goal, distance)` values.
	fn update_overlays<X, Y, P>(&mut self, responses: &mut VecDeque<Message>, positions_and_distances: (X, Y, P), closest_distance: DVec2)
	where
		X: Iterator<Item = (DVec2, DVec2, f64)>,
		Y: Iterator<Item = (DVec2, DVec2, f64)>,
		P: Iterator<Item = (DVec2, DVec2, f64)>,
	{
		self.axis_index = 0;
		self.point_index = 0;

		let (x, y, points) = positions_and_distances;
		self.draw_alignment_lines(true, y, responses, closest_distance);
		self.draw_alignment_lines(false, x, responses, closest_distance);
		self.draw_snap_points(points, responses, closest_distance);

		Self::remove_unused_overlays(&mut self.axis_overlay_paths, responses, self.axis_index);
		Self::remove_unused_overlays(&mut self.point_overlay_paths, responses, self.point_index);
	}

	/// Remove overlays from the pool beyond a given index. Pool entries up through that index will be kept.
	fn remove_unused_overlays(overlay_paths: &mut Vec<Vec<LayerId>>, responses: &mut VecDeque<Message>, remove_after_index: usize) {
		while overlay_paths.len() > remove_after_index {
			responses.push_back(DocumentMessage::Overlays(Operation::DeleteLayer { path: overlay_paths.pop().unwrap() }.into()).into());
		}
	}

	/// Deletes all overlays
	fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		Self::remove_unused_overlays(&mut self.axis_overlay_paths, responses, 0);
		Self::remove_unused_overlays(&mut self.point_overlay_paths, responses, 0);
	}
}

impl SnapHandler {
	/// Computes the necessary translation to the layer to snap it (as well as updating necessary overlays)
	fn calculate_snap<R>(&mut self, targets: R, responses: &mut VecDeque<Message>) -> DVec2
	where
		R: Iterator<Item = DVec2> + Clone,
	{
		let empty = Vec::new();
		let snap_points = self.snap_x && self.snap_y;

		let axis = self.bound_targets.as_ref().unwrap_or(&empty);
		let points = if snap_points { self.point_targets.as_ref().unwrap_or(&empty) } else { &empty };

		let x_axis = if self.snap_x { axis } else { &empty }
			.iter()
			.flat_map(|&pos| targets.clone().map(move |goal| (pos, goal, (pos - goal).x)));
		let y_axis = if self.snap_y { axis } else { &empty }
			.iter()
			.flat_map(|&pos| targets.clone().map(move |goal| (pos, goal, (pos - goal).y)));
		let points = points.iter().flat_map(|&pos| targets.clone().map(move |goal| (pos, pos - goal, (pos - goal).length())));

		let min_x = x_axis.clone().min_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).expect("Could not compare position."));
		let min_y = y_axis.clone().min_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).expect("Could not compare position."));
		let min_points = points.clone().min_by(|a, b| a.2.abs().partial_cmp(&b.2.abs()).expect("Could not compare position."));

		// Snap to a point if possible
		let clamped_closest_distance = if let Some(min_points) = min_points.filter(|&(_, _, dist)| dist <= SNAP_POINT_TOLERANCE) {
			min_points.1
		} else {
			// Do not move if over snap tolerance
			let closest_distance = DVec2::new(min_x.unwrap_or_default().2, min_y.unwrap_or_default().2);
			DVec2::new(
				if closest_distance.x.abs() > SNAP_AXIS_TOLERANCE { 0. } else { closest_distance.x },
				if closest_distance.y.abs() > SNAP_AXIS_TOLERANCE { 0. } else { closest_distance.y },
			)
		};

		self.snap_overlays.update_overlays(responses, (x_axis, y_axis, points), clamped_closest_distance);

		clamped_closest_distance
	}

	/// Gets a list of snap targets for the X and Y axes (if specified) in Viewport coords for the target layers (usually all layers or all non-selected layers.)
	/// This should be called at the start of a drag.
	pub fn start_snap(&mut self, document_message_handler: &DocumentMessageHandler, bounding_boxes: impl Iterator<Item = [DVec2; 2]>, snap_x: bool, snap_y: bool) {
		if document_message_handler.snapping_enabled {
			self.snap_x = snap_x;
			self.snap_y = snap_y;

			// Could be made into sorted Vec or a HashSet for more performant lookups.
			self.bound_targets = Some(bounding_boxes.flat_map(expand_bounds).collect());
			self.point_targets = None;
		}
	}

	/// Add arbitrary snapping points
	///
	/// This should be called after start_snap
	pub fn add_snap_points(&mut self, document_message_handler: &DocumentMessageHandler, snap_points: impl Iterator<Item = DVec2>) {
		if document_message_handler.snapping_enabled {
			if let Some(targets) = &mut self.point_targets {
				targets.extend(snap_points);
			} else {
				self.point_targets = Some(snap_points.collect());
			}
		}
	}

	/// Add the control points (optionally including bézier handles) of the specified shape layer to the snapping points
	///
	/// This should be called after start_snap
	pub fn add_snap_path(&mut self, document_message_handler: &DocumentMessageHandler, layer: &Layer, path: &[LayerId], include_handles: bool, ignore_points: &[(&[LayerId], u64, ControlPointType)]) {
		if let LayerDataType::Shape(shape_layer) = &layer.data {
			let transform = document_message_handler.graphene_document.multiply_transforms(path).unwrap();
			let snap_points = shape_layer
				.shape
				.anchors()
				.enumerate()
				.flat_map(|(id, shape)| {
					if include_handles {
						[
							(*id, &shape.points[ControlPointType::Anchor]),
							(*id, &shape.points[ControlPointType::InHandle]),
							(*id, &shape.points[ControlPointType::OutHandle]),
						]
					} else {
						[(*id, &shape.points[ControlPointType::Anchor]), (0, &None), (0, &None)]
					}
				})
				.filter_map(|(id, point)| point.as_ref().map(|val| (id, val)))
				.filter(|(id, point)| !ignore_points.contains(&(path, *id, point.manipulator_type)))
				.map(|(_id, point)| DVec2::new(point.position.x, point.position.y))
				.map(|pos| transform.transform_point2(pos));
			self.add_snap_points(document_message_handler, snap_points);
		}
	}

	/// Adds all of the shape handles in the document, including bézier handles of the points specified
	pub fn add_all_document_handles(
		&mut self,
		document_message_handler: &DocumentMessageHandler,
		include_handles: &[&[LayerId]],
		exclude: &[&[LayerId]],
		ignore_points: &[(&[LayerId], u64, ControlPointType)],
	) {
		for path in document_message_handler.all_layers() {
			if !exclude.contains(&path) {
				let layer = document_message_handler.graphene_document.layer(path).expect("Could not get layer for snapping");
				self.add_snap_path(document_message_handler, layer, path, include_handles.contains(&path), ignore_points);
			}
		}
	}

	/// Finds the closest snap from an array of layers to the specified snap targets in viewport coords.
	/// Returns 0 for each axis that there is no snap less than the snap tolerance.
	pub fn snap_layers(&mut self, responses: &mut VecDeque<Message>, document_message_handler: &DocumentMessageHandler, snap_anchors: Vec<DVec2>, mouse_delta: DVec2) -> DVec2 {
		if document_message_handler.snapping_enabled {
			self.calculate_snap(snap_anchors.iter().map(move |&snap| mouse_delta + snap), responses)
		} else {
			DVec2::ZERO
		}
	}

	/// Handles snapping of a viewport position, returning another viewport position.
	pub fn snap_position(&mut self, responses: &mut VecDeque<Message>, document_message_handler: &DocumentMessageHandler, position_viewport: DVec2) -> DVec2 {
		if document_message_handler.snapping_enabled {
			self.calculate_snap([position_viewport].into_iter(), responses) + position_viewport
		} else {
			position_viewport
		}
	}

	/// Removes snap target data and overlays. Call this when snapping is done.
	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.snap_overlays.cleanup(responses);
		self.bound_targets = None;
		self.point_targets = None;
	}
}
