mod grid_snapper;
mod layer_snapper;
mod snap_results;
use crate::consts::COLOR_OVERLAY_BLUE;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{BoundingBoxSnapTarget, GeometrySnapTarget, GridSnapTarget, SnapTarget};
use crate::messages::prelude::*;
use bezier_rs::{Subpath, TValue};
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::uuid::ManipulatorGroupId;
use std::cmp::Ordering;
pub use {grid_snapper::*, layer_snapper::*, snap_results::*};

/// Handles snapping and snap overlays
#[derive(Debug, Clone, Default)]
pub struct SnapManager {
	indicator: Option<SnappedPoint>,
	layer_snapper: LayerSnapper,
	grid_snapper: GridSnapper,
	candidates: Option<Vec<LayerNodeIdentifier>>,
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
	document.snapping_state.tolerance / document.navigation.zoom
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

fn get_closest_point(points: &[SnappedPoint]) -> Option<&SnappedPoint> {
	points.iter().min_by(compare_points)
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
						contrained: true,
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
						contrained: true,
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
	pub manipulators: Vec<(LayerNodeIdentifier, ManipulatorGroupId)>,
	pub candidates: Option<&'a Vec<LayerNodeIdentifier>>,
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
			manipulators: Vec::new(),
		}
	}
	fn get_candidates(&self) -> &[LayerNodeIdentifier] {
		self.candidates.map_or([].as_slice(), |candidates| candidates.as_slice())
	}
	fn ignore_bounds(&self, layer: LayerNodeIdentifier) -> bool {
		self.manipulators.iter().any(|&(ignore, _)| ignore == layer)
	}
	fn ignore_manipulator(&self, layer: LayerNodeIdentifier, manipulator: ManipulatorGroupId) -> bool {
		self.manipulators.contains(&(layer, manipulator))
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

	fn find_best_snap(snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: SnapResults, contrained: bool, off_screen: bool, to_path: bool) -> SnappedPoint {
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

		if !contrained {
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

	fn find_candidates(snap_data: &SnapData, point: &SnapCandidatePoint, bbox: Option<Quad>) -> Vec<LayerNodeIdentifier> {
		let document = snap_data.document;
		let offset = snap_tolerance(document);
		let quad = bbox.map_or_else(|| Quad::from_box([point.document_point - offset, point.document_point + offset]), |quad| quad.inflate(offset));
		let mut candidates = Vec::new();

		fn add_candidates(layer: LayerNodeIdentifier, snap_data: &SnapData, quad: Quad, candidates: &mut Vec<LayerNodeIdentifier>) {
			let document = snap_data.document;
			if candidates.len() > 10 {
				return;
			}
			if !document.selected_nodes.layer_visible(layer, &document.network, &document.metadata) {
				return;
			}
			if snap_data.ignore.contains(&layer) {
				return;
			}
			if document.metadata.is_folder(layer) {
				for layer in layer.children(&document.metadata) {
					add_candidates(layer, snap_data, quad, candidates);
				}
				return;
			}
			let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
				return;
			};
			let layer_bounds = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
			let screen_bounds = document.metadata.document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, snap_data.input.viewport_bounds.size()]);
			if quad.intersects(layer_bounds) && screen_bounds.intersects(layer_bounds) {
				candidates.push(layer);
			}
		}
		add_candidates(LayerNodeIdentifier::ROOT, snap_data, quad, &mut candidates);
		if candidates.len() > 10 {
			warn!("Snap candidate overflow");
		}

		candidates
	}

	pub fn free_snap(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, bbox: Option<Quad>, to_paths: bool) -> SnappedPoint {
		if !point.document_point.is_finite() {
			warn!("Snapping non-finite position");
			return SnappedPoint::infinite_snap(DVec2::ZERO);
		}

		let mut snap_results = SnapResults::default();
		if point.source_index == 0 {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		snap_data.candidates = Some(&*self.candidates.get_or_insert_with(|| Self::find_candidates(&snap_data, point, bbox)));
		self.layer_snapper.free_snap(&mut snap_data, point, &mut snap_results);
		self.grid_snapper.free_snap(&mut snap_data, point, &mut snap_results);

		Self::find_best_snap(&mut snap_data, point, snap_results, false, false, to_paths)
	}

	pub fn constrained_snap(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, constraint: SnapConstraint, bbox: Option<Quad>) -> SnappedPoint {
		if !point.document_point.is_finite() {
			warn!("Snapping non-finite position");
			return SnappedPoint::infinite_snap(DVec2::ZERO);
		}

		let mut snap_results = SnapResults::default();
		if point.source_index == 0 {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		snap_data.candidates = Some(&*self.candidates.get_or_insert_with(|| Self::find_candidates(&snap_data, point, bbox)));
		self.layer_snapper.contrained_snap(&mut snap_data, point, &mut snap_results, constraint);
		self.grid_snapper.contrained_snap(&mut snap_data, point, &mut snap_results, constraint);

		Self::find_best_snap(&mut snap_data, point, snap_results, true, false, false)
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

			overlay_context.text(&format!("{:?} to {:?}", ind.source, ind.target), viewport - DVec2::new(0., 5.), "rgba(0, 0, 0, 0.8)", 3.);
			overlay_context.square(viewport, Some(4.), Some(COLOR_OVERLAY_BLUE), Some(COLOR_OVERLAY_BLUE));
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
