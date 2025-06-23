use super::snapping::{self, SnapCandidatePoint, SnapConstraint, SnapData, SnapManager, SnappedPoint};
use crate::consts::{
	BOUNDS_ROTATE_THRESHOLD, BOUNDS_SELECT_THRESHOLD, COLOR_OVERLAY_WHITE, MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT, MAXIMUM_ALT_SCALE_FACTOR, MIN_LENGTH_FOR_CORNERS_VISIBILITY,
	MIN_LENGTH_FOR_EDGE_RESIZE_PRIORITY_OVER_CORNERS, MIN_LENGTH_FOR_MIDPOINT_VISIBILITY, MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR, MIN_LENGTH_FOR_SKEW_TRIANGLE_VISIBILITY, RESIZE_HANDLE_SIZE,
	SELECTION_DRAG_ANGLE, SKEW_TRIANGLE_OFFSET, SKEW_TRIANGLE_SIZE,
};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::transformation::OriginalTransforms;
use crate::messages::prelude::*;
use crate::messages::tool::common_functionality::compass_rose::Axis;
use crate::messages::tool::common_functionality::snapping::SnapTypeConfiguration;
use glam::{DAffine2, DMat2, DVec2};
use graphene_std::renderer::Quad;
use graphene_std::renderer::Rect;

/// (top, bottom, left, right)
pub type EdgeBool = (bool, bool, bool, bool);

pub struct SizeSnapData<'a> {
	pub manager: &'a mut SnapManager,
	pub points: &'a mut Vec<SnapCandidatePoint>,
	pub snap_data: SnapData<'a>,
}

/// Contains the edges that are being dragged along with the original bounds.
#[derive(Clone, Debug, Default)]
pub struct SelectedEdges {
	pub bounds: [DVec2; 2],
	pub top: bool,
	pub bottom: bool,
	pub left: bool,
	pub right: bool,
	// Aspect ratio in the form of width/height, so x:1 = width:height
	aspect_ratio: f64,
}

/// The different possible configurations for how the transform cage is presently viewed, depending on its per-axis sizes and the level of zoom.
/// See doc comments in each variant for a diagram of the configuration.
#[derive(Clone, Debug, Default, PartialEq)]
enum TransformCageSizeCategory {
	#[default]
	/// - ![Diagram](https://files.keavon.com/-/OrganicHelplessWalleye/capture.png)
	Full,
	/// - ![Diagram](https://files.keavon.com/-/AnyGoldenrodHawk/capture.png)
	ReducedLandscape,
	/// - ![Diagram](https://files.keavon.com/-/DarkslategrayAcidicFirebelliedtoad/capture.png)
	ReducedPortrait,
	/// - ![Diagram](https://files.keavon.com/-/GlisteningComplexSeagull/capture.png)
	ReducedBoth,
	/// - ![Diagram](https://files.keavon.com/-/InconsequentialCharmingLynx/capture.png)
	Narrow,
	/// - ![Diagram](https://files.keavon.com/-/OpenPaleturquoiseArthropods/capture.png)
	Flat,
	/// A single point in space with no width or height.
	Point,
}

impl SelectedEdges {
	pub fn new(top: bool, bottom: bool, left: bool, right: bool, bounds: [DVec2; 2]) -> Self {
		let size = (bounds[0] - bounds[1]).abs();
		let aspect_ratio = size.x / size.y;
		Self {
			top,
			bottom,
			left,
			right,
			bounds,
			aspect_ratio,
		}
	}

	/// Calculate the pivot for the operation (the opposite point to the edge dragged)
	pub fn calculate_pivot(&self) -> DVec2 {
		self.pivot_from_bounds(self.bounds[0], self.bounds[1])
	}

	fn pivot_from_bounds(&self, min: DVec2, max: DVec2) -> DVec2 {
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
	pub fn new_size(&self, mouse: DVec2, transform: DAffine2, center_around: Option<DVec2>, constrain: bool, snap: Option<SizeSnapData>) -> (DVec2, DVec2) {
		let mouse = transform.inverse().transform_point2(mouse);

		let mut min = self.bounds[0];
		let mut max = self.bounds[1];

		if self.top {
			min.y = mouse.y;
		} else if self.bottom {
			max.y = mouse.y;
		}
		if self.left {
			min.x = mouse.x;
		} else if self.right {
			max.x = mouse.x;
		}

		let mut pivot = self.pivot_from_bounds(min, max);

		// Alt: Scaling around the pivot
		if let Some(center_around) = center_around {
			let center_around = transform.inverse().transform_point2(center_around);

			let calculate_distance = |moving_opposite_to_drag: &mut f64, center: f64, dragging: f64, original_dragging: f64, current_side: bool| {
				if !current_side {
					return true;
				}

				// The motion of the user's cursor by an `x` pixel offset results in `x * scale_factor` pixels of offset on the other side
				let scale_factor = (center - *moving_opposite_to_drag) / (center - original_dragging);
				let new_distance = center - scale_factor * (center - dragging);

				// Ignore the Alt key press and scale the dragged edge normally
				if !new_distance.is_finite() || scale_factor.abs() > MAXIMUM_ALT_SCALE_FACTOR {
					// Don't go on to check the other sides since this side is already invalid, so Alt-dragging is disabled and updating the pivot would be incorrect
					return false;
				}

				*moving_opposite_to_drag = new_distance;

				true
			};

			// Update the value of the first argument through mutation, and if we make it through all of them without
			// encountering a case where the pivot is too near the edge, we also update the pivot so scaling occurs around it
			if calculate_distance(&mut max.y, center_around.y, min.y, self.bounds[0].y, self.top)
				&& calculate_distance(&mut min.y, center_around.y, max.y, self.bounds[1].y, self.bottom)
				&& calculate_distance(&mut max.x, center_around.x, min.x, self.bounds[0].x, self.left)
				&& calculate_distance(&mut min.x, center_around.x, max.x, self.bounds[1].x, self.right)
			{
				pivot = center_around;
			}
		}

		// Shift: Aspect ratio constraint
		if constrain {
			let size = max - min;
			let min_pivot = (pivot - min) / size;
			let new_size = match ((self.top || self.bottom), (self.left || self.right)) {
				(true, true) => DVec2::new(size.x, size.x / self.aspect_ratio).abs().max(DVec2::new(size.y * self.aspect_ratio, size.y).abs()) * size.signum(),
				(true, false) => DVec2::new(size.y * self.aspect_ratio, size.y),
				(false, true) => DVec2::new(size.x, size.x / self.aspect_ratio),
				_ => size,
			};
			let delta_size = new_size - size;
			min -= delta_size * min_pivot;
			max = min + new_size;
		}

		if let Some(SizeSnapData { manager, points, snap_data }) = snap {
			let view_to_doc = snap_data.document.metadata().document_to_viewport.inverse();
			let bounds_to_doc = view_to_doc * transform;
			let mut best_snap = SnappedPoint::infinite_snap(pivot);
			let mut best_scale_factor = DVec2::ONE;
			let tolerance = snapping::snap_tolerance(snap_data.document);

			let bbox = Some(Rect::from_box((bounds_to_doc * Quad::from_box([min, max])).bounding_box()));
			for (index, point) in points.iter_mut().enumerate() {
				let config = SnapTypeConfiguration {
					bbox,
					use_existing_candidates: index != 0,
					..Default::default()
				};

				let old_position = point.document_point;
				let bounds_space = bounds_to_doc.inverse().transform_point2(point.document_point);
				let normalized = (bounds_space - self.bounds[0]) / (self.bounds[1] - self.bounds[0]);
				let updated = normalized * (max - min) + min;
				point.document_point = bounds_to_doc.transform_point2(updated);
				let mut snapped = if constrain {
					let constraint = SnapConstraint::Line {
						origin: point.document_point,
						direction: (point.document_point - bounds_to_doc.transform_point2(pivot)).normalize_or_zero(),
					};
					manager.constrained_snap(&snap_data, point, constraint, config)
				} else if !(self.top || self.bottom) || !(self.left || self.right) {
					let axis = if !(self.top || self.bottom) { DVec2::X } else { DVec2::Y };
					let constraint = SnapConstraint::Line {
						origin: point.document_point,
						direction: bounds_to_doc.transform_vector2(axis),
					};
					manager.constrained_snap(&snap_data, point, constraint, config)
				} else {
					manager.free_snap(&snap_data, point, config)
				};
				point.document_point = old_position;

				if !snapped.is_snapped() {
					continue;
				}
				let snapped_bounds = bounds_to_doc.inverse().transform_point2(snapped.snapped_point_document);

				let new_from_pivot = snapped_bounds - pivot; // The new vector from the snapped point to the pivot
				let original_from_pivot = updated - pivot; // The original vector from the point to the pivot
				let mut scale_factor = new_from_pivot / original_from_pivot;

				// Constrain should always scale by the same factor in x and y
				if constrain {
					// When the point is on the pivot, we simply copy the other axis.
					if original_from_pivot.x.abs() < 1e-5 {
						scale_factor.x = scale_factor.y;
					} else if original_from_pivot.y.abs() < 1e-5 {
						scale_factor.y = scale_factor.x;
					}

					debug_assert!((scale_factor.x - scale_factor.y).abs() < 1e-5);
				}

				if !(self.left || self.right || constrain) {
					scale_factor.x = 1.
				}
				if !(self.top || self.bottom || constrain) {
					scale_factor.y = 1.
				}

				snapped.distance = bounds_to_doc.transform_vector2((max - min) * (scale_factor - DVec2::ONE)).length();
				if snapped.distance > tolerance || !snapped.distance.is_finite() {
					continue;
				}
				if best_snap.other_snap_better(&snapped) {
					best_snap = snapped;
					best_scale_factor = scale_factor;
				}
			}
			manager.update_indicator(best_snap);

			min = pivot - (pivot - min) * best_scale_factor;
			max = pivot - (pivot - max) * best_scale_factor;
		}

		(min, max - min)
	}

	/// Calculates the required scaling to resize the bounding box
	pub fn bounds_to_scale_transform(&self, position: DVec2, size: DVec2) -> (DAffine2, DVec2) {
		let old_size = self.bounds[1] - self.bounds[0];
		let mut enlargement_factor = size / old_size;
		if !enlargement_factor.x.is_finite() || old_size.x.abs() < f64::EPSILON * 1000. {
			enlargement_factor.x = 1.;
		}
		if !enlargement_factor.y.is_finite() || old_size.y.abs() < f64::EPSILON * 1000. {
			enlargement_factor.y = 1.;
		}
		let mut pivot = (self.bounds[0] * enlargement_factor - position) / (enlargement_factor - DVec2::ONE);
		if !pivot.x.is_finite() {
			pivot.x = 0.;
		}
		if !pivot.y.is_finite() {
			pivot.y = 0.;
		}
		(DAffine2::from_scale(enlargement_factor), pivot)
	}

	pub fn skew_transform(&self, mouse: DVec2, to_viewport_transform: DAffine2, free_movement: bool) -> DAffine2 {
		// Skip if the matrix is singular
		if !to_viewport_transform.matrix2.determinant().recip().is_finite() {
			return DAffine2::IDENTITY;
		}

		let opposite = self.pivot_from_bounds(self.bounds[0], self.bounds[1]);
		let dragging_point = self.pivot_from_bounds(self.bounds[1], self.bounds[0]);

		let viewport_dragging_point = to_viewport_transform.transform_point2(dragging_point);
		let parallel_to_x = self.top || self.bottom;
		let parallel_to_y = !parallel_to_x && (self.left || self.right);

		let drag_vector = mouse - viewport_dragging_point;
		let document_drag_vector = to_viewport_transform.inverse().transform_vector2(drag_vector);

		let bounds = (self.bounds[1] - self.bounds[0]).abs();
		let sign = if self.top || self.left { -1. } else { 1. };
		let signed_bounds = sign * bounds;

		let scale_factor = if parallel_to_x { signed_bounds.y.recip() } else { signed_bounds.x.recip() };
		let scaled_document_drag = document_drag_vector * scale_factor;

		let skew = DAffine2::from_mat2(DMat2::from_cols_array(&[
			1. + if parallel_to_y && free_movement { scaled_document_drag.x } else { 0. },
			if parallel_to_y { scaled_document_drag.y } else { 0. },
			if parallel_to_x { scaled_document_drag.x } else { 0. },
			1. + if parallel_to_x && free_movement { scaled_document_drag.y } else { 0. },
		]));

		DAffine2::from_translation(opposite) * skew * DAffine2::from_translation(-opposite)
	}
}

/// Aligns the mouse position to the closest axis
pub fn axis_align_drag(axis_align: bool, axis: Axis, position: DVec2, start: DVec2) -> DVec2 {
	if axis_align {
		let mouse_position = position - start;
		let snap_resolution = SELECTION_DRAG_ANGLE.to_radians();
		let angle = -mouse_position.angle_to(DVec2::X);
		let snapped_angle = (angle / snap_resolution).round() * snap_resolution;
		let axis_vector = DVec2::from_angle(snapped_angle);
		if snapped_angle.is_finite() {
			start + axis_vector * mouse_position.dot(axis_vector).abs()
		} else {
			start
		}
	} else if axis.is_constraint() {
		let mouse_position = position - start;
		let axis_vector: DVec2 = axis.into();
		start + axis_vector * mouse_position.dot(axis_vector)
	} else {
		position
	}
}

/// Snaps a dragging event from the artboard or select tool
pub fn snap_drag(start: DVec2, current: DVec2, snap_to_axis: bool, axis: Axis, snap_data: SnapData, snap_manager: &mut SnapManager, candidates: &[SnapCandidatePoint]) -> DVec2 {
	let mouse_position = axis_align_drag(snap_to_axis, axis, snap_data.input.mouse.position, start);
	let document = snap_data.document;
	let total_mouse_delta_document = document.metadata().document_to_viewport.inverse().transform_vector2(mouse_position - start);
	let mouse_delta_document = document.metadata().document_to_viewport.inverse().transform_vector2(mouse_position - current);
	let mut offset = mouse_delta_document;
	let mut best_snap = SnappedPoint::infinite_snap(document.metadata().document_to_viewport.inverse().transform_point2(mouse_position));

	let bbox = Rect::point_iter(candidates.iter().map(|candidate| candidate.document_point + total_mouse_delta_document));

	for (index, point) in candidates.iter().enumerate() {
		let config = SnapTypeConfiguration {
			bbox,
			accept_distribution: true,
			use_existing_candidates: index != 0,
			..Default::default()
		};

		let mut point = point.clone();
		point.document_point += total_mouse_delta_document;

		let constrained_along_axis = snap_to_axis || axis.is_constraint();
		let snapped = if constrained_along_axis {
			let constraint = SnapConstraint::Line {
				origin: point.document_point,
				direction: total_mouse_delta_document.try_normalize().unwrap_or(DVec2::X),
			};
			snap_manager.constrained_snap(&snap_data, &point, constraint, config)
		} else {
			snap_manager.free_snap(&snap_data, &point, config)
		};

		if best_snap.other_snap_better(&snapped) {
			offset = snapped.snapped_point_document - point.document_point + mouse_delta_document;
			best_snap = snapped;
		}
	}

	snap_manager.update_indicator(best_snap);

	document.metadata().document_to_viewport.transform_vector2(offset)
}

/// Contains info on the overlays for the bounding box and transform handles
#[derive(Clone, Debug, Default)]
pub struct BoundingBoxManager {
	/// The corners of the box. Transform with original_bound_transform to get viewport co-ordinates.
	pub bounds: [DVec2; 2],
	/// The transform to viewport space for the bounds co-ordinates when the bounds were last updated.
	pub transform: DAffine2,
	/// Whether the transform is actually singular but adjusted to not be so.
	pub transform_tampered: bool,
	/// The transform to viewport space for the bounds co-ordinates when the transformation was started.
	pub original_bound_transform: DAffine2,
	pub selected_edges: Option<SelectedEdges>,
	pub original_transforms: OriginalTransforms,
	pub opposite_pivot: DVec2,
	pub center_of_transformation: DVec2,
}

impl BoundingBoxManager {
	/// Calculates the transformed handle positions based on the bounding box and the transform
	pub fn evaluate_transform_handle_positions(&self) -> [DVec2; 8] {
		let (left, top): (f64, f64) = self.bounds[0].into();
		let (right, bottom): (f64, f64) = self.bounds[1].into();
		[
			DVec2::new(left, top),
			DVec2::new(left, (top + bottom) / 2.),
			DVec2::new(left, bottom),
			DVec2::new((left + right) / 2., top),
			DVec2::new((left + right) / 2., bottom),
			DVec2::new(right, top),
			DVec2::new(right, (top + bottom) / 2.),
			DVec2::new(right, bottom),
		]
	}

	pub fn get_closest_edge(&self, edges: EdgeBool, cursor: DVec2) -> EdgeBool {
		if !edges.0 && !edges.1 && !edges.2 && !edges.3 {
			return (false, false, false, false);
		}

		let cursor = self.transform.inverse().transform_point2(cursor);
		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);

		let distances = [
			edges.0.then(|| (cursor - DVec2::new(cursor.x, min.y)).length_squared()),
			edges.1.then(|| (cursor - DVec2::new(cursor.x, max.y)).length_squared()),
			edges.2.then(|| (cursor - DVec2::new(min.x, cursor.y)).length_squared()),
			edges.3.then(|| (cursor - DVec2::new(max.x, cursor.y)).length_squared()),
		];

		let min_distance = distances.iter().filter_map(|&x| x).min_by(|a, b| a.partial_cmp(b).unwrap());

		match min_distance {
			Some(min) => (
				edges.0 && distances[0].is_some_and(|d| (d - min).abs() < f64::EPSILON),
				edges.1 && distances[1].is_some_and(|d| (d - min).abs() < f64::EPSILON),
				edges.2 && distances[2].is_some_and(|d| (d - min).abs() < f64::EPSILON),
				edges.3 && distances[3].is_some_and(|d| (d - min).abs() < f64::EPSILON),
			),
			None => (false, false, false, false),
		}
	}

	pub fn check_skew_handle(&self, cursor: DVec2, edge: EdgeBool) -> bool {
		let Some([start, end]) = self.edge_endpoints_vector_from_edge_bool(edge) else { return false };
		if (end - start).length_squared() < MIN_LENGTH_FOR_SKEW_TRIANGLE_VISIBILITY.powi(2) {
			return false;
		};

		let edge_dir = (end - start).normalize();
		let mid = start.midpoint(end);

		for direction in [-edge_dir, edge_dir] {
			let base = mid + direction * (3. + SKEW_TRIANGLE_OFFSET + SKEW_TRIANGLE_SIZE / 2.);
			let extension = cursor - base;
			let along_edge = extension.dot(edge_dir).abs();
			let along_perp = extension.perp_dot(edge_dir).abs();

			if along_edge <= SKEW_TRIANGLE_SIZE / 2. && along_perp <= BOUNDS_SELECT_THRESHOLD {
				return true;
			}
		}
		false
	}

	pub fn edge_endpoints_vector_from_edge_bool(&self, edges: EdgeBool) -> Option<[DVec2; 2]> {
		let quad = self.transform * Quad::from_box(self.bounds);
		let category = self.overlay_display_category();

		if matches!(
			category,
			TransformCageSizeCategory::Full | TransformCageSizeCategory::Narrow | TransformCageSizeCategory::ReducedLandscape
		) {
			if edges.0 {
				return Some([quad.top_left(), quad.top_right()]);
			}
			if edges.1 {
				return Some([quad.bottom_left(), quad.bottom_right()]);
			}
		}

		if matches!(
			category,
			TransformCageSizeCategory::Full | TransformCageSizeCategory::Narrow | TransformCageSizeCategory::ReducedPortrait
		) {
			if edges.2 {
				return Some([quad.top_left(), quad.bottom_left()]);
			}
			if edges.3 {
				return Some([quad.top_right(), quad.bottom_right()]);
			}
		}
		None
	}

	pub fn render_skew_gizmos(&mut self, overlay_context: &mut OverlayContext, hover_edge: EdgeBool) {
		let mut draw_edge_triangles = |start: DVec2, end: DVec2| {
			if (end - start).length() < MIN_LENGTH_FOR_SKEW_TRIANGLE_VISIBILITY {
				return;
			}

			let edge_dir = (end - start).normalize();
			let mid = end.midpoint(start);

			for edge in [edge_dir, -edge_dir] {
				overlay_context.draw_triangle(mid + edge * (3. + SKEW_TRIANGLE_OFFSET), edge, SKEW_TRIANGLE_SIZE, None, None);
			}
		};

		if let Some([start, end]) = self.edge_endpoints_vector_from_edge_bool(hover_edge) {
			draw_edge_triangles(start, end);
		}
	}

	pub fn over_extended_edge_midpoint(&self, mouse: DVec2, hover_edge: EdgeBool) -> bool {
		const HALF_WIDTH_OUTER_RECT: f64 = RESIZE_HANDLE_SIZE / 2. + SKEW_TRIANGLE_OFFSET + SKEW_TRIANGLE_SIZE;
		const HALF_WIDTH_INNER_RECT: f64 = SKEW_TRIANGLE_OFFSET + RESIZE_HANDLE_SIZE / 2.;

		const INNER_QUAD_CORNER: DVec2 = DVec2::new(HALF_WIDTH_INNER_RECT, RESIZE_HANDLE_SIZE / 2.);
		const FULL_QUAD_CORNER: DVec2 = DVec2::new(HALF_WIDTH_OUTER_RECT, BOUNDS_SELECT_THRESHOLD);

		let quad = self.transform * Quad::from_box(self.bounds);

		let Some([start, end]) = self.edge_endpoints_vector_from_edge_bool(hover_edge) else {
			return false;
		};
		if (end - start).length() < MIN_LENGTH_FOR_SKEW_TRIANGLE_VISIBILITY {
			return false;
		}

		let angle;
		let is_compact;
		if hover_edge.0 || hover_edge.1 {
			angle = (quad.top_left() - quad.top_right()).to_angle();
			is_compact = (quad.top_left() - quad.bottom_left()).length_squared() < MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR.powi(2);
		} else if hover_edge.2 || hover_edge.3 {
			angle = (quad.top_left() - quad.bottom_left()).to_angle();
			is_compact = (quad.top_left() - quad.top_right()).length_squared() < MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR.powi(2);
		} else {
			return false;
		};

		let has_triangle_hover = self.check_skew_handle(mouse, hover_edge);
		let point = start.midpoint(end);

		if is_compact {
			let upper_rect = DAffine2::from_angle_translation(angle, point) * Quad::from_box([-FULL_QUAD_CORNER.with_y(0.), FULL_QUAD_CORNER]);
			let inter_triangle_quad = DAffine2::from_angle_translation(angle, point) * Quad::from_box([-INNER_QUAD_CORNER, INNER_QUAD_CORNER]);

			upper_rect.contains(mouse) || has_triangle_hover || inter_triangle_quad.contains(mouse)
		} else {
			let rect = DAffine2::from_angle_translation(angle, point) * Quad::from_box([-FULL_QUAD_CORNER, FULL_QUAD_CORNER]);

			rect.contains(mouse) || has_triangle_hover
		}
	}

	pub fn render_quad(&self, overlay_context: &mut OverlayContext) {
		let quad = self.transform * Quad::from_box(self.bounds);

		// Draw the bounding box rectangle
		overlay_context.quad(quad, None, None);
	}

	/// Update the position of the bounding box and transform handles
	pub fn render_overlays(&mut self, overlay_context: &mut OverlayContext, render_quad: bool) {
		let quad = self.transform * Quad::from_box(self.bounds);
		let category = self.overlay_display_category();

		let horizontal_edges = [quad.top_right().midpoint(quad.bottom_right()), quad.bottom_left().midpoint(quad.top_left())];
		let vertical_edges = [quad.top_left().midpoint(quad.top_right()), quad.bottom_right().midpoint(quad.bottom_left())];

		if render_quad {
			self.render_quad(overlay_context);
		}

		let mut draw_handle = |point: DVec2, angle: f64| {
			let quad = DAffine2::from_angle_translation(angle, point) * Quad::from_box([DVec2::splat(-RESIZE_HANDLE_SIZE / 2.), DVec2::splat(RESIZE_HANDLE_SIZE / 2.)]);
			overlay_context.quad(quad, None, Some(COLOR_OVERLAY_WHITE));
		};

		let horizontal_angle = (quad.top_left() - quad.bottom_left()).to_angle();
		let vertical_angle = (quad.top_left() - quad.top_right()).to_angle();

		// Draw the horizontal midpoint drag handles
		if matches!(
			category,
			TransformCageSizeCategory::Full | TransformCageSizeCategory::Narrow | TransformCageSizeCategory::ReducedLandscape
		) {
			for point in horizontal_edges {
				draw_handle(point, horizontal_angle);
			}
		}

		// Draw the vertical midpoint drag handles
		if matches!(
			category,
			TransformCageSizeCategory::Full | TransformCageSizeCategory::Narrow | TransformCageSizeCategory::ReducedPortrait
		) {
			for point in vertical_edges {
				draw_handle(point, vertical_angle);
			}
		}

		let angle = quad
			.edges()
			.map(|[x, y]| x.distance_squared(y))
			.into_iter()
			.reduce(|horizontal_distance, vertical_distance| if horizontal_distance > vertical_distance { horizontal_angle } else { vertical_angle })
			.unwrap_or_default();

		// Draw the corner drag handles
		if matches!(
			category,
			TransformCageSizeCategory::Full | TransformCageSizeCategory::ReducedBoth | TransformCageSizeCategory::ReducedLandscape | TransformCageSizeCategory::ReducedPortrait
		) {
			for point in quad.0 {
				draw_handle(point, angle);
			}
		}

		// Draw the flat line endpoint drag handles
		if category == TransformCageSizeCategory::Flat {
			draw_handle(self.transform.transform_point2(self.bounds[0]), angle);
			draw_handle(self.transform.transform_point2(self.bounds[1]), angle);
		}
	}

	/// Find the [`TransformCageSizeCategory`] of this bounding box based on size thresholds.
	fn overlay_display_category(&self) -> TransformCageSizeCategory {
		let quad = self.transform * Quad::from_box(self.bounds);

		// Check if the bounds are essentially the same because the width and height are smaller than MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT
		if self.is_bounds_point() {
			return TransformCageSizeCategory::Point;
		}

		// Check if the area is essentially zero because either the width or height is smaller than MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT
		if self.is_bounds_flat() {
			return TransformCageSizeCategory::Flat;
		}

		let vertical_length = (quad.top_left() - quad.top_right()).length_squared();
		let horizontal_length = (quad.bottom_left() - quad.top_left()).length_squared();
		let corners_visible = vertical_length >= MIN_LENGTH_FOR_CORNERS_VISIBILITY.powi(2) && horizontal_length >= MIN_LENGTH_FOR_CORNERS_VISIBILITY.powi(2);

		if corners_visible {
			let vertical_edge_visible = vertical_length > MIN_LENGTH_FOR_MIDPOINT_VISIBILITY.powi(2);
			let horizontal_edge_visible = horizontal_length > MIN_LENGTH_FOR_MIDPOINT_VISIBILITY.powi(2);

			return match (vertical_edge_visible, horizontal_edge_visible) {
				(true, true) => TransformCageSizeCategory::Full,
				(true, false) => TransformCageSizeCategory::ReducedPortrait,
				(false, true) => TransformCageSizeCategory::ReducedLandscape,
				(false, false) => TransformCageSizeCategory::ReducedBoth,
			};
		}

		TransformCageSizeCategory::Narrow
	}

	/// Determine if these bounds are flat ([`TransformCageSizeCategory::Flat`]), which means that the width and/or height is essentially zero and the bounds are a line with effectively no area. This can happen on actual lines (axis-aligned, i.e. drawn horizontally or vertically) or when an element is scaled to zero in X or Y. A flat transform cage can still be rotated by a transformation, but its local space remains flat.
	fn is_bounds_flat(&self) -> bool {
		(self.bounds[0] - self.bounds[1]).abs().cmple(DVec2::splat(MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT)).any()
	}

	/// Determine if these bounds are point ([`TransformCageSizeCategory::Point`]), which means that the width and height are essentially zero and the bounds are a point with no area. This can happen on points when an element is scaled to zero in both X and Y, or if an element is just a single anchor point. A point transform cage cannot be rotated by a transformation, and its local space remains a point.
	fn is_bounds_point(&self) -> bool {
		(self.bounds[0] - self.bounds[1]).abs().cmple(DVec2::splat(MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT)).all()
	}

	/// Determine if the given point in viewport space falls within the bounds of `self`.
	fn is_contained_in_bounds(&self, point: DVec2) -> bool {
		let document_point = self.transform.inverse().transform_point2(point);
		Quad::from_box(self.bounds).contains(document_point)
	}

	/// Compute the threshold in viewport space. This only works with affine transforms as it assumes lines remain parallel.
	fn compute_viewport_threshold(&self, scalar: f64) -> [f64; 2] {
		let inverse = self.transform.inverse();

		let viewport_x = self.transform.transform_vector2(DVec2::X).normalize_or_zero() * scalar;
		let viewport_y = self.transform.transform_vector2(DVec2::Y).normalize_or_zero() * scalar;

		let threshold_x = inverse.transform_vector2(viewport_x).length();
		let threshold_y = inverse.transform_vector2(viewport_y).length();

		[threshold_x, threshold_y]
	}

	/// Check if the user has selected the edge for dragging.
	///
	/// Returns which edge in the order:
	///
	/// `top, bottom, left, right`
	pub fn check_selected_edges(&self, cursor: DVec2) -> Option<EdgeBool> {
		let cursor = self.transform.inverse().transform_point2(cursor);

		let min = self.bounds[0].min(self.bounds[1]);
		let max = self.bounds[0].max(self.bounds[1]);

		let [threshold_x, threshold_y] = self.compute_viewport_threshold(BOUNDS_SELECT_THRESHOLD);
		let [corner_min_x, corner_min_y] = self.compute_viewport_threshold(MIN_LENGTH_FOR_CORNERS_VISIBILITY);
		let [edge_min_x, edge_min_y] = self.compute_viewport_threshold(MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR);
		let [midpoint_threshold_x, midpoint_threshold_y] = self.compute_viewport_threshold(MIN_LENGTH_FOR_EDGE_RESIZE_PRIORITY_OVER_CORNERS);

		if (min.x - cursor.x < threshold_x && min.y - cursor.y < threshold_y) && (cursor.x - max.x < threshold_x && cursor.y - max.y < threshold_y) {
			let mut top = (cursor.y - min.y).abs() < threshold_y;
			let mut bottom = (max.y - cursor.y).abs() < threshold_y;
			let mut left = (cursor.x - min.x).abs() < threshold_x;
			let mut right = (max.x - cursor.x).abs() < threshold_x;

			let width = max.x - min.x;
			let height = max.y - min.y;

			if (left || right) && (top || bottom) {
				let horizontal_midpoint_x = (min.x + max.x) / 2.;
				let vertical_midpoint_y = (min.y + max.y) / 2.;

				if (cursor.x - horizontal_midpoint_x).abs() < midpoint_threshold_x {
					left = false;
					right = false;
				} else if (cursor.y - vertical_midpoint_y).abs() < midpoint_threshold_y {
					top = false;
					bottom = false;
				}
			}

			if width < edge_min_x || height <= edge_min_y {
				if self.transform_tampered {
					return None;
				}

				if min.x < cursor.x && cursor.x < max.x && cursor.y < max.y && cursor.y > min.y {
					return None;
				}

				// Prioritize single axis transformations on very small bounds
				if height < corner_min_y && (left || right) {
					top = false;
					bottom = false;
				}
				if width < corner_min_x && (top || bottom) {
					left = false;
					right = false;
				}

				// On bounds with no width/height, disallow transformation in the relevant axis
				if width < MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT {
					left = false;
					right = false;
				}
				if height < MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT {
					top = false;
					bottom = false;
				}
			}

			if top || bottom || left || right {
				return Some((top, bottom, left, right));
			}
		}

		None
	}

	/// Check if the user is rotating with the bounds
	pub fn check_rotate(&self, cursor: DVec2) -> bool {
		if self.is_contained_in_bounds(cursor) {
			return false;
		}
		let [threshold_x, threshold_y] = self.compute_viewport_threshold(BOUNDS_ROTATE_THRESHOLD);
		let cursor = self.transform.inverse().transform_point2(cursor);

		let flat = self.is_bounds_flat();
		let point = self.is_bounds_point();
		let within_square_bounds = |center: &DVec2| center.x - threshold_x < cursor.x && cursor.x < center.x + threshold_x && center.y - threshold_y < cursor.y && cursor.y < center.y + threshold_y;
		if point {
			false
		} else if flat {
			[self.bounds[0], self.bounds[1]].iter().any(within_square_bounds)
		} else {
			self.evaluate_transform_handle_positions().iter().any(within_square_bounds)
		}
	}

	/// Gets the required mouse cursor to show resizing bounds or optionally rotation
	pub fn get_cursor(&self, input: &InputPreprocessorMessageHandler, rotate: bool, dragging_bounds: bool, skew_edge: Option<EdgeBool>) -> MouseCursorIcon {
		let edges = self.check_selected_edges(input.mouse.position);

		let is_near_square = edges.is_some_and(|hover_edge| self.over_extended_edge_midpoint(input.mouse.position, hover_edge));
		if dragging_bounds && is_near_square {
			if let Some(skew_edge) = skew_edge {
				if self.check_skew_handle(input.mouse.position, skew_edge) {
					if skew_edge.0 || skew_edge.1 {
						return MouseCursorIcon::EWResize;
					} else if skew_edge.2 || skew_edge.3 {
						return MouseCursorIcon::NSResize;
					}
				}
			};
		}

		match edges {
			Some((top, bottom, left, right)) => match (top, bottom, left, right) {
				(true, _, false, false) | (_, true, false, false) => MouseCursorIcon::NSResize,
				(false, false, true, _) | (false, false, _, true) => MouseCursorIcon::EWResize,
				(true, _, true, _) | (_, true, _, true) => MouseCursorIcon::NWSEResize,
				(true, _, _, true) | (_, true, true, _) => MouseCursorIcon::NESWResize,
				_ => MouseCursorIcon::Default,
			},
			_ if rotate && self.check_rotate(input.mouse.position) => MouseCursorIcon::Rotate,
			_ => MouseCursorIcon::Default,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn skew_transform_singular() {
		for edge in [
			SelectedEdges::new(true, false, false, false, [DVec2::NEG_ONE, DVec2::ONE]),
			SelectedEdges::new(false, true, false, false, [DVec2::NEG_ONE, DVec2::ONE]),
			SelectedEdges::new(false, false, true, false, [DVec2::NEG_ONE, DVec2::ONE]),
			SelectedEdges::new(false, false, false, true, [DVec2::NEG_ONE, DVec2::ONE]),
		] {
			// The determinant is 0.
			let transform = DAffine2::from_cols_array(&[2.; 6]);
			// This shouldn't panic. We don't really care about the behavior in this test.
			let _ = edge.skew_transform(DVec2::new(1.5, 1.5), transform, false);
		}
	}

	#[test]
	fn skew_transform_correct() {
		for edge in [
			SelectedEdges::new(true, false, false, false, [DVec2::NEG_ONE, DVec2::ONE]),
			SelectedEdges::new(false, true, false, false, [DVec2::NEG_ONE, DVec2::ONE]),
			SelectedEdges::new(false, false, true, false, [DVec2::NEG_ONE, DVec2::ONE]),
			SelectedEdges::new(false, false, false, true, [DVec2::NEG_ONE, DVec2::ONE]),
		] {
			// Random transform with det != 0.
			let to_viewport_transform = DAffine2::from_cols_array(&[2., 1., 0., 1., 2., 3.]);
			// Random mouse position.
			let mouse = DVec2::new(1.5, 1.5);
			let final_transform = edge.skew_transform(mouse, to_viewport_transform, false);

			// This is the current handle that goes under the mouse.
			let opposite = edge.pivot_from_bounds(edge.bounds[0], edge.bounds[1]);
			let dragging_point = edge.pivot_from_bounds(edge.bounds[1], edge.bounds[0]);

			let viewport_dragging_point = to_viewport_transform.transform_point2(dragging_point);
			let parallel_to_x = edge.top || edge.bottom;
			let parallel_to_y = !parallel_to_x && (edge.left || edge.right);

			let drag_vector = mouse - viewport_dragging_point;
			let document_drag_vector = to_viewport_transform.inverse().transform_vector2(drag_vector);

			let sign = if edge.top || edge.left { -1. } else { 1. };
			let scale_factor = (edge.bounds[1] - edge.bounds[0])[parallel_to_x as usize].abs().recip() * sign;
			let scaled_document_drag = document_drag_vector * scale_factor;

			let skew = DAffine2::from_mat2(DMat2::from_cols_array(&[
				1.,
				if parallel_to_y { scaled_document_drag.y } else { 0. },
				if parallel_to_x { scaled_document_drag.x } else { 0. },
				1.,
			]));

			let constructed_transform = DAffine2::from_translation(opposite) * skew * DAffine2::from_translation(-opposite);

			assert_eq!(constructed_transform, final_transform);
		}
	}
}
