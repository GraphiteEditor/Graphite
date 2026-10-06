use super::snap_session::{BoxFeatures, FeatureMap, Locus, Moving, PointOptions, SnapSession, SnapView};
use super::utility_functions::push_nearest_anchors;
use crate::consts::{
	BOUNDS_ROTATE_THRESHOLD, BOUNDS_SELECT_THRESHOLD, MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT, MAXIMUM_ALT_SCALE_FACTOR, MIN_LENGTH_FOR_CORNERS_VISIBILITY, MIN_LENGTH_FOR_EDGE_RESIZE_PRIORITY_OVER_CORNERS,
	MIN_LENGTH_FOR_MIDPOINT_VISIBILITY, MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR, MIN_LENGTH_FOR_SKEW_TRIANGLE_VISIBILITY, RESIZE_HANDLE_SIZE, SKEW_TRIANGLE_OFFSET, SKEW_TRIANGLE_SIZE,
};
use crate::messages::frontend::utility_types::MouseCursorIcon;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::transformation::OriginalTransforms;
use crate::messages::prelude::*;
use glam::{DAffine2, DMat2, DVec2};
use graphene_std::renderer::Quad;

/// (top, bottom, left, right)
pub type EdgeBool = (bool, bool, bool, bool);

pub struct SizeSnapData<'a> {
	pub session: &'a mut SnapSession,
	pub view: SnapView<'a>,
}

/// Contains the edges that are being dragged along with the original bounds.
#[derive(Clone, Debug, Default)]
pub struct SelectedEdges {
	pub bounds: [DVec2; 2],
	pub top: bool,
	pub bottom: bool,
	pub left: bool,
	pub right: bool,
	/// Anchors of the resized layers, in the same space as the bounds before the resize, which snap along with the box.
	pub anchors: Vec<DVec2>,
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
			anchors: Vec::new(),
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
		let mut from_center = false;

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
				from_center = true;
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

		if let Some(snap) = snap {
			[min, max] = self.snap_stretch(snap, transform, [min, max], pivot, constrain, from_center);
		}

		(min, max - min)
	}

	/// Snaps the original bounds, stretched about a pivot to the unsnapped corners, and returns the snapped corners, all in the cage's space.
	fn snap_stretch(&self, snap: SizeSnapData, transform: DAffine2, unsnapped: [DVec2; 2], pivot: DVec2, constrain: bool, from_center: bool) -> [DVec2; 2] {
		let SizeSnapData { session, view } = snap;
		let [first, second] = self.bounds;
		let [min, max] = unsnapped;

		// A scale is held at 1 along an axis the drag leaves alone (unless Shift carries it along) or where the box has no extent
		let dragged = [self.left || self.right, self.top || self.bottom];
		let has_extent = [0, 1].map(|axis| (second[axis] - first[axis]).abs() > MAX_LENGTH_FOR_NO_WIDTH_OR_HEIGHT);
		let held = [0, 1].map(|axis| !has_extent[axis] || !(dragged[axis] || constrain));
		if held == [true, true] {
			return unsnapped;
		}

		// Each scale is read at the original side further from the pivot, since the other side may lie on it
		let scale = |axis: usize| {
			let from_pivot = [first[axis] - pivot[axis], second[axis] - pivot[axis]];
			let side = usize::from(from_pivot[1].abs() > from_pivot[0].abs());
			([min[axis], max[axis]][side] - pivot[axis]) / from_pivot[side]
		};
		let free = DVec2::from([0, 1].map(|axis| if held[axis] { 1. } else { scale(axis) }));
		if !free.is_finite() {
			return unsnapped;
		}

		let to_document = view.document.metadata().document_to_viewport.inverse() * transform;
		let axes = [DVec2::X, DVec2::Y].map(|axis| to_document.transform_vector2(axis).try_normalize());
		let [Some(x_axis), Some(y_axis)] = axes else { return unsnapped };
		let axes = [x_axis, y_axis];
		let pivot_in_document = to_document.transform_point2(pivot);

		// Built from the original box alone, so the features stay the same from move to move and an engaged snap holds
		let corners = [first, DVec2::new(second.x, first.y), second, DVec2::new(first.x, second.y)];
		let middles = [0, 1, 2, 3].map(|i| (corners[i] + corners[(i + 1) % 4]) / 2.);
		let stretched = |start: DVec2| {
			let mut feature = FeatureMap::stretching(to_document.transform_point2(start), pivot_in_document, axes);

			// A held scale gets a direction of its own, so features level with the pivot along that axis can still snap
			for axis in (0..2).filter(|&axis| held[axis]) {
				let column = feature.matrix.col_mut(axis);
				feature.offset += *column - axes[axis];
				*column = axes[axis];
			}
			feature
		};
		let box_points = corners.into_iter().chain(middles).chain([(first + second) / 2.]);
		let features: Vec<FeatureMap> = box_points.chain(self.anchors.iter().copied()).map(stretched).collect();

		// The pointer carries the dragged corner, or for an edge, its end further from the pivot so the two are never level
		let dragged_second_side = [self.right, self.bottom];
		let on_second_side = [0, 1].map(|axis| {
			if dragged[axis] {
				dragged_second_side[axis]
			} else {
				(second[axis] - pivot[axis]).abs() > (first[axis] - pivot[axis]).abs()
			}
		});
		let lead = match on_second_side {
			[false, false] => 0,
			[true, false] => 1,
			[true, true] => 2,
			[false, true] => 3,
		};

		// An edge drag keeps the scale along the edge at 1, and Shift keeps the scales in proportion
		let required = match held {
			[true, false] => Locus::line(DVec2::new(1., 0.), DVec2::Y),
			[false, true] => Locus::line(DVec2::new(0., 1.), DVec2::X),
			_ if constrain => Locus::line(DVec2::ZERO, free.signum()),
			_ => None,
		};

		// Gaps and sizes are matched only for a cage upright in the document, and only along an axis where just the lead's side moves
		let upright = axes.iter().all(|axis| axis.x.abs() < 1e-9 || axis.y.abs() < 1e-9);
		let lead_position = features[lead].position(free);
		let box_features = upright.then(|| {
			let sides = [0, 1].map(|document_axis| {
				let along = usize::from(y_axis[document_axis].abs() > x_axis[document_axis].abs());
				let mut sides = [None, None];
				if !held[along] && !constrain && !from_center {
					sides[usize::from(lead_position[document_axis] >= pivot_in_document[document_axis])] = Some(lead);
				}
				sides
			});
			BoxFeatures { corners: [0, 1, 2, 3], sides }
		});

		let options = PointOptions {
			required,
			box_features,
			..Default::default()
		};
		let snapped = session.snap_features(view, &features, lead, free, options);

		// The snapped scales stretch the original box about the pivot, while a held axis keeps what it got unsnapped
		let stretch = |bound: DVec2| pivot + (bound - pivot) * snapped;
		let [mut snapped_min, mut snapped_max] = [stretch(first), stretch(second)];
		for axis in (0..2).filter(|&axis| held[axis]) {
			snapped_min[axis] = min[axis];
			snapped_max[axis] = max[axis];
		}
		[snapped_min, snapped_max]
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

/// How far a feature of a turn or skew must move per unit the lead moves to be snapped.
/// Less is rounding error on a feature that stays put, which the solver would divide by.
const MIN_FEATURE_TRAVEL: f64 = 1e-9;

/// What a turn of the cage snaps by, built once as it begins so that an engaged snap holds from move to move.
#[derive(Clone, Debug)]
pub struct Turning {
	/// What the cage turns about, in document space.
	pivot: DVec2,
	/// Where the pointer took hold, in document space.
	grip: DVec2,
	/// The cage's corners and edge middles and the nearest anchors of the turned layers, then the grip, which only follows.
	features: Vec<FeatureMap>,
}

/// What a skew of the cage snaps by, built once as it begins so that an engaged snap holds from move to move.
#[derive(Clone, Debug)]
pub struct Skewing {
	/// Where the pointer took hold, in document space.
	grip: DVec2,
	/// The middle of the dragged edge, which the pointer carries, in document space.
	lead_start: DVec2,
	/// The direction of the dragged edge in document space.
	edge_direction: DVec2,
	/// The middle of the dragged edge, then the cage's other corners and edge middles and the nearest anchors of the skewed layers.
	features: Vec<FeatureMap>,
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
	/// What a turn of the cage under way snaps by.
	pub turning: Option<Turning>,
	/// What a skew of the cage under way snaps by.
	pub skewing: Option<Skewing>,
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
			overlay_context.skew_handles(start, end);
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

		let (angle, is_compact) = if hover_edge.0 || hover_edge.1 {
			let angle = (quad.top_left() - quad.top_right()).to_angle();
			(angle, (quad.top_left() - quad.bottom_left()).length_squared() < MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR.powi(2))
		} else if hover_edge.2 || hover_edge.3 {
			let angle = (quad.top_left() - quad.bottom_left()).to_angle();
			(angle, (quad.top_left() - quad.top_right()).length_squared() < MIN_LENGTH_FOR_RESIZE_TO_INCLUDE_INTERIOR.powi(2))
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

		let horizontal_angle = (quad.top_left() - quad.bottom_left()).to_angle();
		let vertical_angle = (quad.top_left() - quad.top_right()).to_angle();

		// Draw the horizontal midpoint drag handles
		if matches!(
			category,
			TransformCageSizeCategory::Full | TransformCageSizeCategory::Narrow | TransformCageSizeCategory::ReducedLandscape
		) {
			for point in horizontal_edges {
				overlay_context.resize_handle(point, horizontal_angle);
			}
		}

		// Draw the vertical midpoint drag handles
		if matches!(
			category,
			TransformCageSizeCategory::Full | TransformCageSizeCategory::Narrow | TransformCageSizeCategory::ReducedPortrait
		) {
			for point in vertical_edges {
				overlay_context.resize_handle(point, vertical_angle);
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
				overlay_context.resize_handle(point, angle);
			}
		}

		// Draw the flat line endpoint drag handles
		if category == TransformCageSizeCategory::Flat {
			overlay_context.resize_handle(self.transform.transform_point2(self.bounds[0]), angle);
			overlay_context.resize_handle(self.transform.transform_point2(self.bounds[1]), angle);
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
		if dragging_bounds
			&& is_near_square
			&& let Some(skew_edge) = skew_edge
			&& self.check_skew_handle(input.mouse.position, skew_edge)
		{
			if skew_edge.0 || skew_edge.1 {
				return MouseCursorIcon::EWResize;
			} else if skew_edge.2 || skew_edge.3 {
				return MouseCursorIcon::NSResize;
			}
		};

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

	/// Begins a turn of the cage about its center of transformation, held where the pointer is, which snaps the cage and the layers in it.
	pub fn begin_turning(&mut self, view: SnapView, session: &mut SnapSession, layers: &[LayerNodeIdentifier]) {
		let document = view.document;
		let viewport_to_document = document.metadata().document_to_viewport.inverse();
		let pivot = viewport_to_document.transform_point2(self.center_of_transformation);
		let grip = viewport_to_document.transform_point2(view.input.mouse.position);

		let cage_to_document = viewport_to_document * self.transform;
		let mut starts: Vec<DVec2> = corners_and_middles(self.bounds).into_iter().map(|point| cage_to_document.transform_point2(point)).collect();
		push_nearest_anchors(&mut starts, document, layers, grip);

		let moves = |start: &DVec2| start.distance(pivot) > MIN_FEATURE_TRAVEL * grip.distance(pivot);
		let turned = |start: DVec2| FeatureMap::turning_about(start, grip, pivot);
		let features: Vec<FeatureMap> = starts.into_iter().filter(moves).chain([grip]).filter_map(turned).collect();
		self.turning = (!features.is_empty()).then_some(Turning { pivot, grip, features });

		let moving = Moving {
			layers: layers.to_vec(),
			..Default::default()
		};
		session.begin_gesture(view, moving);
	}

	/// Snaps the turn the pointer asks for, and returns its angle as seen in the viewport.
	pub fn snap_turn(&self, view: SnapView, session: &mut SnapSession) -> Option<f64> {
		let Turning { pivot, grip, features } = self.turning.as_ref()?;
		let document_to_viewport = view.document.metadata().document_to_viewport;
		let pointer = document_to_viewport.inverse().transform_point2(view.input.mouse.position);

		// The grip keeps to its circle so the turn does not also scale
		let lead = features.len().saturating_sub(1);
		let options = PointOptions {
			required: Locus::circle(*pivot, pivot.distance(*grip)),
			snapping: Some(lead),
			ignores_grid: true,
			..Default::default()
		};
		let turned = session.snap_features(view, features, lead, pointer, options);

		// Measured in the viewport, where the turn is applied, which a flipped canvas mirrors
		let seen = |point: DVec2| document_to_viewport.transform_point2(point);
		(seen(*grip) - seen(*pivot)).try_angle_to(seen(turned) - seen(*pivot))
	}

	/// Begins a skew of the cage by the edge it was taken hold of by, held where the pointer is, which snaps the cage and the layers in it.
	pub fn begin_skewing(&mut self, view: SnapView, session: &mut SnapSession, layers: &[LayerNodeIdentifier]) {
		let moving = Moving {
			layers: layers.to_vec(),
			..Default::default()
		};
		session.begin_gesture(view, moving);

		self.skewing = None;
		let Some(edges) = &self.selected_edges else { return };

		let document = view.document;
		let viewport_to_document = document.metadata().document_to_viewport.inverse();
		let cage_to_document = viewport_to_document * self.original_bound_transform;
		let grip = viewport_to_document.transform_point2(view.input.mouse.position);

		let dragged = edges.pivot_from_bounds(edges.bounds[1], edges.bounds[0]);
		let fixed = edges.pivot_from_bounds(edges.bounds[0], edges.bounds[1]);
		let (along, across) = if edges.top || edges.bottom { (DVec2::X, DVec2::Y) } else { (DVec2::Y, DVec2::X) };

		let lead_start = cage_to_document.transform_point2(dragged);
		let mut starts = vec![lead_start];
		let others = corners_and_middles(edges.bounds).into_iter().filter(|point| !point.abs_diff_eq(dragged, 1e-9));
		starts.extend(others.map(|point| cage_to_document.transform_point2(point)));
		push_nearest_anchors(&mut starts, document, layers, grip);

		// Each point follows by the fraction of the way it lies from the fixed edge to the dragged one
		let document_to_cage = cage_to_document.inverse();
		let height = (dragged - fixed).dot(across);
		let following = |start: DVec2| {
			let fraction = (document_to_cage.transform_point2(start) - fixed).dot(across) / height;
			(fraction.is_finite() && fraction.abs() > MIN_FEATURE_TRAVEL).then(|| FeatureMap::following(start, lead_start, fraction))
		};
		let features = std::iter::once(FeatureMap::IDENTITY).chain(starts.into_iter().skip(1).filter_map(following)).collect();

		self.skewing = Some(Skewing {
			grip,
			lead_start,
			edge_direction: cage_to_document.transform_vector2(along),
			features,
		});
	}

	/// Snaps the skew the pointer asks for, and returns the viewport position that brings the middle of the dragged edge to where it lands.
	pub fn snap_skew(&self, view: SnapView, session: &mut SnapSession, free_movement: bool) -> DVec2 {
		let pointer = view.input.mouse.position;
		let (Some(skewing), Some(edges)) = (&self.skewing, &self.selected_edges) else { return pointer };
		let document_to_viewport = view.document.metadata().document_to_viewport;

		// Unless free movement is on, the edge slides only along itself
		let required = if free_movement { None } else { Locus::line(skewing.lead_start, skewing.edge_direction) };
		let options = PointOptions { required, ..Default::default() };
		let free = skewing.lead_start + document_to_viewport.inverse().transform_point2(pointer) - skewing.grip;
		let skewed = session.snap_features(view, &skewing.features, 0, free, options);

		let dragged = edges.pivot_from_bounds(edges.bounds[1], edges.bounds[0]);
		self.original_bound_transform.transform_point2(dragged) + document_to_viewport.transform_vector2(skewed - skewing.lead_start)
	}
}

/// The corners of a box, in order around it, then the middles of its edges.
fn corners_and_middles([first, second]: [DVec2; 2]) -> Vec<DVec2> {
	let corners = [first, DVec2::new(second.x, first.y), second, DVec2::new(first.x, second.y)];
	let middles = [0, 1, 2, 3].map(|i| (corners[i] + corners[(i + 1) % 4]) / 2.);
	corners.into_iter().chain(middles).collect()
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
