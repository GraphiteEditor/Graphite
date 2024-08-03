mod alignment_snapper;
mod distribution_snapper;
mod grid_snapper;
mod layer_snapper;
mod snap_results;
pub use {alignment_snapper::*, distribution_snapper::*, grid_snapper::*, layer_snapper::*, snap_results::*};

use crate::consts::COLOR_OVERLAY_BLUE;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{BoundingBoxSnapTarget, GeometrySnapTarget, GridSnapTarget, SnapTarget};
use crate::messages::prelude::*;

use bezier_rs::{Subpath, TValue};
use graphene_core::renderer::Quad;
use graphene_core::vector::PointId;
use graphene_std::renderer::Rect;

use glam::{DAffine2, DVec2};
use std::cmp::Ordering;

/// Handles snapping and snap overlays
#[derive(Debug, Clone, Default)]
pub struct SnapManager {
	indicator: Option<SnappedPoint>,
	layer_snapper: LayerSnapper,
	grid_snapper: GridSnapper,
	alignment_snapper: AlignmentSnapper,
	distribution_snapper: DistributionSnapper,
	candidates: Option<Vec<LayerNodeIdentifier>>,
	alignment_candidates: Option<Vec<LayerNodeIdentifier>>,
}

#[derive(Clone, Copy, Debug, Default)]
pub enum SnapConstraint {
	#[default]
	None,
	Line {
		origin: DVec2,
		direction: DVec2,
	},
	Direction(DVec2),
	Circle {
		center: DVec2,
		radius: f64,
	},
}
impl SnapConstraint {
	pub fn projection(&self, point: DVec2) -> DVec2 {
		match *self {
			Self::Line { origin, direction } if direction != DVec2::ZERO => (point - origin).project_onto(direction) + origin,
			Self::Circle { center, radius } => {
				let from_center = point - center;
				let distance = from_center.length();
				if distance > 0. {
					center + radius * from_center / distance
				} else {
					// Point is exactly at the center, so project right
					center + DVec2::new(radius, 0.)
				}
			}
			_ => point,
		}
	}
	pub fn direction(&self) -> DVec2 {
		match *self {
			Self::Line { direction, .. } | Self::Direction(direction) => direction,
			_ => DVec2::ZERO,
		}
	}
}
pub fn snap_tolerance(document: &DocumentMessageHandler) -> f64 {
	document.snapping_state.tolerance / document.document_ptz.zoom()
}

fn compare_points(a: &&SnappedPoint, b: &&SnappedPoint) -> Ordering {
	if (a.target.bounding_box() && !b.target.bounding_box()) || (a.at_intersection && !b.at_intersection) || (a.source.bounding_box() && !b.source.bounding_box()) {
		Ordering::Greater
	} else if (!a.target.bounding_box() && b.target.bounding_box()) || (!a.at_intersection && b.at_intersection) || (!a.source.bounding_box() && b.source.bounding_box()) {
		Ordering::Less
	} else {
		a.distance.partial_cmp(&b.distance).unwrap()
	}
}

fn find_align(a: &&SnappedPoint, b: &&SnappedPoint) -> Ordering {
	(a.distance, a.distance_to_align_target).partial_cmp(&(b.distance, b.distance_to_align_target)).unwrap()
}

fn get_closest_point(points: &[SnappedPoint]) -> Option<&SnappedPoint> {
	let not_align = points.iter().filter(|point| !point.align()).min_by(compare_points);
	let align = points.iter().filter(|point| point.align()).min_by(find_align);
	match (not_align, align) {
		(None, None) => None,
		(Some(result), None) | (None, Some(result)) => Some(result),
		(Some(not_align), Some(align)) => Some(not_align),
	}
}
fn get_closest_curve(curves: &[SnappedCurve], exclude_paths: bool) -> Option<&SnappedPoint> {
	let keep_curve = |curve: &&SnappedCurve| !exclude_paths || curve.point.target != SnapTarget::Geometry(GeometrySnapTarget::Path);
	curves.iter().filter(keep_curve).map(|curve| &curve.point).min_by(compare_points)
}
fn get_closest_line(lines: &[SnappedLine]) -> Option<&SnappedPoint> {
	lines.iter().map(|curve| &curve.point).min_by(compare_points)
}
fn get_closest_intersection(snap_to: DVec2, curves: &[SnappedCurve]) -> Option<SnappedPoint> {
	let mut best = None;
	for curve_i in curves {
		if curve_i.point.target == SnapTarget::BoundingBox(BoundingBoxSnapTarget::Edge) {
			continue;
		}

		for curve_j in curves {
			if curve_j.point.target == SnapTarget::BoundingBox(BoundingBoxSnapTarget::Edge) {
				continue;
			}
			if curve_i.start == curve_j.start && curve_i.layer == curve_j.layer {
				continue;
			}
			for curve_i_t in curve_i.document_curve.intersections(&curve_j.document_curve, None, None) {
				let snapped_point_document = curve_i.document_curve.evaluate(TValue::Parametric(curve_i_t));
				let distance = snap_to.distance(snapped_point_document);
				let i_closer = curve_i.point.distance < curve_j.point.distance;
				let close = if i_closer { curve_i } else { curve_j };
				let far = if i_closer { curve_j } else { curve_i };
				if !best.as_ref().is_some_and(|best: &SnappedPoint| best.distance < distance) {
					best = Some(SnappedPoint {
						snapped_point_document,
						distance,
						target: SnapTarget::Geometry(GeometrySnapTarget::Intersection),
						tolerance: close.point.tolerance,
						curves: [Some(close.document_curve), Some(far.document_curve)],
						source: close.point.source,
						at_intersection: true,
						constrained: true,
						..Default::default()
					})
				}
			}
		}
	}
	best
}
fn get_grid_intersection(snap_to: DVec2, lines: &[SnappedLine]) -> Option<SnappedPoint> {
	let mut best = None;
	for line_i in lines {
		for line_j in lines {
			if let Some(snapped_point_document) = Quad::intersect_rays(line_i.point.snapped_point_document, line_i.direction, line_j.point.snapped_point_document, line_j.direction) {
				let distance = snap_to.distance(snapped_point_document);
				if !best.as_ref().is_some_and(|best: &SnappedPoint| best.distance < distance) {
					best = Some(SnappedPoint {
						snapped_point_document,
						distance,
						target: SnapTarget::Grid(GridSnapTarget::Intersection),
						tolerance: line_i.point.tolerance,
						source: line_i.point.source,
						at_intersection: true,
						constrained: true,
						..Default::default()
					})
				}
			}
		}
	}
	best
}
#[derive(Clone)]
pub struct SnapData<'a> {
	pub document: &'a DocumentMessageHandler,
	pub input: &'a InputPreprocessorMessageHandler,
	pub ignore: &'a [LayerNodeIdentifier],
	pub manipulators: Vec<(LayerNodeIdentifier, PointId)>,
	pub candidates: Option<&'a Vec<LayerNodeIdentifier>>,
	pub alignment_candidates: Option<&'a Vec<LayerNodeIdentifier>>,
}
impl<'a> SnapData<'a> {
	pub fn new(document: &'a DocumentMessageHandler, input: &'a InputPreprocessorMessageHandler) -> Self {
		Self::ignore(document, input, &[])
	}
	pub fn ignore(document: &'a DocumentMessageHandler, input: &'a InputPreprocessorMessageHandler, ignore: &'a [LayerNodeIdentifier]) -> Self {
		Self {
			document,
			input,
			ignore,
			candidates: None,
			alignment_candidates: None,
			manipulators: Vec::new(),
		}
	}
	fn get_candidates(&self) -> &[LayerNodeIdentifier] {
		self.candidates.map_or([].as_slice(), |candidates| candidates.as_slice())
	}
	fn ignore_bounds(&self, layer: LayerNodeIdentifier) -> bool {
		self.manipulators.iter().any(|&(ignore, _)| ignore == layer)
	}
	fn ignore_manipulator(&self, layer: LayerNodeIdentifier, manipulator: impl Into<PointId>) -> bool {
		self.manipulators.contains(&(layer, manipulator.into()))
	}
}
impl SnapManager {
	pub fn update_indicator(&mut self, snapped_point: SnappedPoint) {
		self.indicator = snapped_point.is_snapped().then_some(snapped_point);
	}
	pub fn clear_indicator(&mut self) {
		self.indicator = None;
	}
	pub fn preview_draw(&mut self, snap_data: &SnapData, mouse: DVec2) {
		let point = SnapCandidatePoint::handle(snap_data.document.metadata.document_to_viewport.inverse().transform_point2(mouse));
		let snapped = self.free_snap(snap_data, &point, None, false);
		self.update_indicator(snapped);
	}

	fn find_best_snap(snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: SnapResults, constrained: bool, off_screen: bool, to_path: bool) -> SnappedPoint {
		let mut snapped_points = Vec::new();
		let document = snap_data.document;

		if let Some(closest_point) = get_closest_point(&snap_results.points) {
			snapped_points.push(closest_point.clone());
		}
		let exclude_paths = !document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Path));
		if let Some(closest_curve) = get_closest_curve(&snap_results.curves, exclude_paths) {
			snapped_points.push(closest_curve.clone());
		}

		if document.snapping_state.target_enabled(SnapTarget::Grid(GridSnapTarget::Line)) {
			if let Some(closest_line) = get_closest_line(&snap_results.grid_lines) {
				snapped_points.push(closest_line.clone());
			}
		}

		if !constrained {
			if document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Intersection)) {
				if let Some(closest_curves_intersection) = get_closest_intersection(point.document_point, &snap_results.curves) {
					snapped_points.push(closest_curves_intersection);
				}
			}
			if document.snapping_state.target_enabled(SnapTarget::Grid(GridSnapTarget::Intersection)) {
				if let Some(closest_grid_intersection) = get_grid_intersection(point.document_point, &snap_results.grid_lines) {
					snapped_points.push(closest_grid_intersection);
				}
			}
		}

		if to_path {
			snapped_points.retain(|i| matches!(i.target, SnapTarget::Geometry(_)));
		}

		let mut best_point = None;

		for point in snapped_points {
			let viewport_point = document.metadata.document_to_viewport.transform_point2(point.snapped_point_document);
			let on_screen = viewport_point.cmpgt(DVec2::ZERO).all() && viewport_point.cmplt(snap_data.input.viewport_bounds.size()).all();
			if !on_screen && !off_screen {
				continue;
			}
			if point.distance > point.tolerance {
				continue;
			}
			if best_point.as_ref().is_some_and(|best: &SnappedPoint| point.other_snap_better(best)) {
				continue;
			}
			best_point = Some(point);
		}

		best_point.unwrap_or(SnappedPoint::infinite_snap(point.document_point))
	}

	fn add_candidates(&mut self, layer: LayerNodeIdentifier, snap_data: &SnapData, quad: Quad) {
		let document = snap_data.document;

		if !document.selected_nodes.layer_visible(layer, &document.metadata) {
			return;
		}
		if snap_data.ignore.contains(&layer) {
			return;
		}
		if document.metadata.is_folder(layer) {
			for layer in layer.children(&document.metadata) {
				self.add_candidates(layer, snap_data, quad);
			}
			return;
		}
		let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
			return;
		};
		let layer_bounds = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
		let screen_bounds = document.metadata.document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, snap_data.input.viewport_bounds.size()]);
		if screen_bounds.intersects(layer_bounds) {
			if !self.alignment_candidates.as_ref().is_some_and(|candidates| candidates.len() > 100) {
				self.alignment_candidates.get_or_insert_with(|| Vec::new()).push(layer);
			}
			if quad.intersects(layer_bounds) && !self.candidates.as_ref().is_some_and(|candidates| candidates.len() > 10) {
				self.candidates.get_or_insert_with(|| Vec::new()).push(layer);
			}
		}
	}

	fn find_candidates(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, bbox: Option<Rect>) {
		let document = snap_data.document;
		let offset = snap_tolerance(document);
		let quad = bbox.map_or_else(|| Quad::from_square(point.document_point, offset), |quad| Quad::from_box(quad.0).inflate(offset));

		self.candidates = None;
		self.alignment_candidates = None;
		for layer in LayerNodeIdentifier::ROOT_PARENT.children(&document.metadata) {
			self.add_candidates(layer, snap_data, quad);
		}

		if self.alignment_candidates.as_ref().is_some_and(|candidates| candidates.len() > 100) {
			warn!("Alignment candidate overflow");
		}
		if self.candidates.as_ref().is_some_and(|candidates| candidates.len() > 10) {
			warn!("Snap candidate overflow");
		}
	}

	pub fn free_snap(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, bbox: Option<Rect>, to_paths: bool) -> SnappedPoint {
		if !point.document_point.is_finite() {
			warn!("Snapping non-finite position");
			return SnappedPoint::infinite_snap(DVec2::ZERO);
		}

		let mut snap_results = SnapResults::default();
		if point.source_index == 0 {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		if snap_data.candidates.is_none() {
			self.find_candidates(&snap_data, point, bbox);
		}
		snap_data.candidates = self.candidates.as_ref();
		snap_data.alignment_candidates = self.alignment_candidates.as_ref();

		self.layer_snapper.free_snap(&mut snap_data, point, &mut snap_results);
		self.grid_snapper.free_snap(&mut snap_data, point, &mut snap_results);
		self.alignment_snapper.free_snap(&mut snap_data, point, &mut snap_results);
		self.distribution_snapper.free_snap(&mut snap_data, point, &mut snap_results, bbox);

		Self::find_best_snap(&mut snap_data, point, snap_results, false, false, to_paths)
	}

	pub fn constrained_snap(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, constraint: SnapConstraint, bbox: Option<Rect>) -> SnappedPoint {
		if !point.document_point.is_finite() {
			warn!("Snapping non-finite position");
			return SnappedPoint::infinite_snap(DVec2::ZERO);
		}

		let mut snap_results = SnapResults::default();
		if point.source_index == 0 {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		if snap_data.candidates.is_none() {
			self.find_candidates(&snap_data, point, bbox);
		}
		snap_data.candidates = self.candidates.as_ref();
		snap_data.alignment_candidates = self.alignment_candidates.as_ref();

		self.layer_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint);
		self.grid_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint);
		self.alignment_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint);
		self.distribution_snapper.constrained_snap(&mut snap_data, point, &mut snap_results, constraint, bbox);

		Self::find_best_snap(&mut snap_data, point, snap_results, true, false, false)
	}

	fn alignment_x_overlay(boxes: &VecDeque<Rect>, transform: DAffine2, overlay_context: &mut OverlayContext) {
		let y_size = transform.inverse().transform_vector2(DVec2::Y * 8.).length();
		for (&first, &second) in boxes.iter().zip(boxes.iter().skip(1)) {
			let bottom = first.center().y < second.center().y + y_size;
			let y = if bottom { first.max() } else { first.min() }.y;
			let start = DVec2::new(first.max().x, y);
			let end = DVec2::new(second.min().x, y);
			let signed_size = if bottom { y_size } else { -y_size };
			overlay_context.line(transform.transform_point2(start), transform.transform_point2(start + DVec2::Y * signed_size));
			overlay_context.line(transform.transform_point2(end), transform.transform_point2(end + DVec2::Y * signed_size));
			overlay_context.line(
				transform.transform_point2(start + DVec2::Y * signed_size / 2.),
				transform.transform_point2(end + DVec2::Y * signed_size / 2.),
			);
		}
	}

	fn alignment_y_overlay(boxes: &VecDeque<Rect>, transform: DAffine2, overlay_context: &mut OverlayContext) {
		let x_size = transform.inverse().transform_vector2(DVec2::X * 8.).length();
		for (&first, &second) in boxes.iter().zip(boxes.iter().skip(1)) {
			let right = first.center().x < second.center().x + x_size;
			let x = if right { first.max() } else { first.min() }.x;
			let start = DVec2::new(x, first.max().y);
			let end = DVec2::new(x, second.min().y);
			let signed_size = if right { x_size } else { -x_size };
			overlay_context.line(transform.transform_point2(start), transform.transform_point2(start + DVec2::X * signed_size));
			overlay_context.line(transform.transform_point2(end), transform.transform_point2(end + DVec2::X * signed_size));
			overlay_context.line(
				transform.transform_point2(start + DVec2::X * signed_size / 2.),
				transform.transform_point2(end + DVec2::X * signed_size / 2.),
			);
		}
	}

	pub fn draw_overlays(&mut self, snap_data: SnapData, overlay_context: &mut OverlayContext) {
		let to_viewport = snap_data.document.metadata.document_to_viewport;
		if let Some(ind) = &self.indicator {
			for curve in &ind.curves {
				let Some(curve) = curve else { continue };
				overlay_context.outline([Subpath::from_bezier(curve)].iter(), to_viewport);
			}
			if let Some(quad) = ind.target_bounds {
				overlay_context.quad(to_viewport * quad);
			}
			let viewport = to_viewport.transform_point2(ind.snapped_point_document);

			Self::alignment_x_overlay(&ind.distribution_boxes_x, to_viewport, overlay_context);
			Self::alignment_y_overlay(&ind.distribution_boxes_y, to_viewport, overlay_context);

			if let Some(alignment_target) = ind.alignment_target {
				let alignment_target = to_viewport.transform_point2(alignment_target);

				overlay_context.line(viewport, alignment_target);
				if let Some(alignment_target_intersect) = ind.alignment_target_intersect {
					let alignment_target_intersect = to_viewport.transform_point2(alignment_target_intersect);
					overlay_context.line(viewport, alignment_target_intersect);
					overlay_context.manipulator_handle(alignment_target_intersect, false);
				}
				overlay_context.manipulator_handle(viewport, false);
				overlay_context.manipulator_handle(alignment_target, false);
			}

			if ind.alignment_target.is_none() && ind.distribution_equal_distance_x.is_none() && ind.distribution_equal_distance_y.is_none() {
				overlay_context.text(&format!("{:?} to {:?}", ind.source, ind.target), viewport - DVec2::new(0., 5.), "rgba(0, 0, 0, 0.8)", 3.);
				overlay_context.square(viewport, Some(4.), Some(COLOR_OVERLAY_BLUE), Some(COLOR_OVERLAY_BLUE));
			}
		}
	}

	/// Removes snap target data and overlays. Call this when snapping is done.
	pub fn cleanup(&mut self, responses: &mut VecDeque<Message>) {
		self.candidates = None;
		self.indicator = None;
		responses.add(OverlaysMessage::Draw);
	}
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
