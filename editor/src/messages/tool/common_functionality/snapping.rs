use std::cmp::Ordering;

use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::portfolio::document::overlays::utility_types::OverlayContext;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{
	BoardSnapSource, BoardSnapTarget, BoundingBoxSnapSource, BoundingBoxSnapTarget, NodeSnapSource, NodeSnapTarget, SnapSource, SnapTarget,
};
use crate::messages::prelude::*;
use bezier_rs::{Bezier, Identifier, Subpath, TValue};
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::uuid::ManipulatorGroupId;

/// Handles snapping and snap overlays
#[derive(Debug, Clone, Default)]
pub struct SnapManager {
	indicator: Option<SnappedPoint>,
	object_snapper: ObjectSnapper,
	candidates: Option<Vec<LayerNodeIdentifier>>,
}
#[derive(Clone, Debug)]
struct SnapCandidatePath {
	document_curve: Bezier,
	layer: LayerNodeIdentifier,
	start: ManipulatorGroupId,
	target: SnapTarget,
	bounds: Option<Quad>,
}
#[derive(Clone, Debug, Default)]
pub struct SnapCandidatePoint {
	document_point: DVec2,
	source: SnapSource,
	target: SnapTarget,
	source_index: usize,
	quad: Option<Quad>,
}
impl SnapCandidatePoint {
	pub fn new(document_point: DVec2, source: SnapSource, target: SnapTarget) -> Self {
		Self::new_quad(document_point, source, target, None)
	}
	pub fn new_quad(document_point: DVec2, source: SnapSource, target: SnapTarget, quad: Option<Quad>) -> Self {
		Self {
			document_point,
			source,
			target,
			quad: quad,
			..Default::default()
		}
	}
	pub fn new_source(document_point: DVec2, source: SnapSource) -> Self {
		Self::new(document_point, source, SnapTarget::None)
	}
	pub fn handle(document_point: DVec2) -> Self {
		Self::new_source(document_point, SnapSource::Node(NodeSnapSource::Sharp))
	}
}
#[derive(Default, Debug, Clone)]
pub struct SnappedPoint {
	pub snapped_point_document: DVec2,
	pub curve_tangent: DVec2,
	pub source: SnapSource,
	pub target: SnapTarget,
	pub at_intersection: bool,
	pub contrained: bool,        // Found when looking for contrained
	pub fully_constrained: bool, // e.g. on point (on a path is not fully contrained)
	pub target_bounds: Option<Quad>,
	pub source_bounds: Option<Quad>,
	pub curves: [Option<Bezier>; 2],
	pub distance: f64,
	pub tollerance: f64,
}
impl SnappedPoint {
	pub fn from_source_point(snapped_point_document: DVec2, source: SnapSource) -> Self {
		Self {
			snapped_point_document,
			source,
			..Default::default()
		}
	}
	pub fn other_snap_better(&self, other: &Self) -> bool {
		if self.distance.is_finite() && !other.distance.is_finite() {
			return false;
		}
		if !self.distance.is_finite() && other.distance.is_finite() {
			return true;
		}

		let my_dist = self.distance;
		let other_dist = other.distance;

		// Prefer closest
		let other_closer = other_dist < my_dist;

		// We should prefer the most contrained option (e.g. intersection > path)
		let other_more_contrained = other.contrained && !self.contrained;
		let self_more_contrained = self.contrained && !other.contrained;

		// Prefer nodes to intersections if both are at the same position
		let contrained_at_same_pos = other.contrained && self.contrained && self.snapped_point_document.abs_diff_eq(other.snapped_point_document, 1.);
		let other_better_contraint = contrained_at_same_pos && self.at_intersection && !other.at_intersection;
		let self_better_contraint = contrained_at_same_pos && other.at_intersection && !self.at_intersection;

		(other_closer || other_more_contrained || other_better_contraint) && !self_more_contrained && !self_better_contraint
	}
	pub fn is_snapped(&self) -> bool {
		self.distance.is_finite()
	}
}
#[derive(Default)]
struct BBoxSnapValues {
	corner_source: SnapSource,
	corner_target: SnapTarget,
	edge_source: SnapSource,
	edge_target: SnapTarget,
	centre_source: SnapSource,
	centre_target: SnapTarget,
}
impl BBoxSnapValues {
	pub const fn new(corners: bool, edges: bool, centre: bool) -> Self {
		Self {
			corner_source: if corners { SnapSource::BoundingBox(BoundingBoxSnapSource::Corner) } else { SnapSource::None },
			corner_target: if corners { SnapTarget::BoundingBox(BoundingBoxSnapTarget::Corner) } else { SnapTarget::None },
			edge_source: if edges { SnapSource::BoundingBox(BoundingBoxSnapSource::EdgeMidpoint) } else { SnapSource::None },
			edge_target: if edges { SnapTarget::BoundingBox(BoundingBoxSnapTarget::EdgeMidpoint) } else { SnapTarget::None },
			centre_source: if centre { SnapSource::BoundingBox(BoundingBoxSnapSource::Centre) } else { SnapSource::None },
			centre_target: if centre { SnapTarget::BoundingBox(BoundingBoxSnapTarget::Centre) } else { SnapTarget::None },
		}
	}
}
fn get_bbox_points(quad: Quad, points: &mut Vec<SnapCandidatePoint>, values: BBoxSnapValues) {
	for index in 0..4 {
		let start = quad.0[index];
		let end = quad.0[(index + 1) % 4];
		if values.corner_source.is_some() || values.corner_target.is_some() {
			points.push(SnapCandidatePoint::new_quad(start, values.corner_source, values.corner_target, Some(quad)));
		}
		if values.edge_source.is_some() || values.edge_target.is_some() {
			points.push(SnapCandidatePoint::new_quad((start + end) / 2., values.edge_source, values.edge_target, Some(quad)));
		}
	}
	if values.centre_source.is_some() || values.centre_target.is_some() {
		points.push(SnapCandidatePoint::new_quad(quad.center(), values.centre_source, values.centre_target, Some(quad)));
	}
}
fn subpath_anchor_snap_points(layer: LayerNodeIdentifier, subpath: &Subpath<ManipulatorGroupId>, snap_data: &SnapData, points: &mut Vec<SnapCandidatePoint>, to_document: DAffine2) {
	let handle_not_under = |&offset: &DVec2| to_document.transform_vector2(offset).length_squared() >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE;
	let document = snap_data.document;
	// Midpoints of linear segments
	if document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::LineMidpoint)) {
		for (index, curve) in subpath.iter().enumerate() {
			if snap_data.ignore_manipulator(layer, subpath.manipulator_groups()[index].id) || snap_data.ignore_manipulator(layer, subpath.manipulator_groups()[(index + 1) % subpath.len()].id) {
				continue;
			}

			let in_handle = curve.handle_start().map(|handle| handle - curve.start).filter(handle_not_under);
			let out_handle = curve.handle_end().map(|handle| handle - curve.end).filter(handle_not_under);
			if in_handle.is_none() && out_handle.is_none() {
				points.push(SnapCandidatePoint::new(
					to_document.transform_point2(curve.start() * 0.5 + curve.end * 0.5),
					SnapSource::Node(NodeSnapSource::LineMidpoint),
					SnapTarget::Node(NodeSnapTarget::LineMidpoint),
				));
			}
		}
	}
	// Anchors
	for (index, group) in subpath.manipulator_groups().iter().enumerate() {
		if snap_data.ignore_manipulator(layer, group.id) {
			continue;
		}

		let anchor = group.anchor;
		let handle_in = group.in_handle.map(|handle| anchor - handle).filter(handle_not_under);
		let handle_out = group.out_handle.map(|handle| handle - anchor).filter(handle_not_under);
		let at_end = !subpath.closed() && (index == 0 || index == subpath.len() - 1);
		let smooth = handle_in.is_some_and(|handle_in| handle_out.is_some_and(|handle_out| handle_in.angle_between(handle_out) < 1e-5)) && !at_end;

		// Smooth points
		if smooth && document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Smooth)) {
			points.push(SnapCandidatePoint::new(
				to_document.transform_point2(anchor),
				SnapSource::Node(NodeSnapSource::Smooth),
				SnapTarget::Node(NodeSnapTarget::Smooth),
			));
		}
		// Sharp points
		if !smooth && document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Sharp)) {
			points.push(SnapCandidatePoint::new(
				to_document.transform_point2(anchor),
				SnapSource::Node(NodeSnapSource::Sharp),
				SnapTarget::Node(NodeSnapTarget::Sharp),
			));
		}
	}
}
fn get_layer_snap_points(layer: LayerNodeIdentifier, snap_data: &SnapData, points: &mut Vec<SnapCandidatePoint>) {
	let document = snap_data.document;
	if document.metadata().is_artboard(layer) {
	} else if document.metadata().is_folder(layer) {
		for child in layer.decendants(document.metadata()) {
			get_layer_snap_points(child, snap_data, points);
		}
	} else {
		// Skip empty paths
		if document.metadata.layer_outline(layer).next().is_none() {
			return;
		}
		let to_document = document.metadata.transform_to_document(layer);
		for subpath in document.metadata.layer_outline(layer) {
			subpath_anchor_snap_points(layer, subpath, snap_data, points, to_document);
		}
	}
}

fn get_selected_snap_points(snap_data: &SnapData) -> Vec<SnapCandidatePoint> {
	let mut points = Vec::new();
	for layer in snap_data.document.selected_visible_layers() {
		get_layer_snap_points(layer, snap_data, &mut points);
	}
	points
}
#[derive(Clone, Debug, Default)]
pub struct SnappedLine {
	pub point: DVec2,
	pub normal: DVec2,
}
#[derive(Clone, Debug)]
pub struct SnappedCurve {
	pub layer: LayerNodeIdentifier,
	pub start: ManipulatorGroupId,
	pub point: SnappedPoint,
	pub document_curve: Bezier,
}
#[derive(Clone, Debug, Default)]
struct SnapResults {
	pub points: Vec<SnappedPoint>,
	pub grid_lines: Vec<SnappedLine>,
	pub curves: Vec<SnappedCurve>,
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
		centre: DVec2,
		radius: f64,
	},
}
impl SnapConstraint {
	pub fn projection(&self, point: DVec2) -> DVec2 {
		match *self {
			Self::Line { origin, direction } if direction != DVec2::ZERO => (point - origin).project_onto(direction) + origin,
			Self::Circle { centre, radius } => {
				let from_centre = point - centre;
				let distance = from_centre.length();
				if distance > 0. {
					centre + radius * from_centre / distance
				} else {
					// Point is exactly at the centre, so project right
					centre + DVec2::new(radius, 0.)
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
#[derive(Clone, Debug, Default)]
struct ObjectSnapper {
	points_to_snap: Vec<SnapCandidatePoint>,
	paths_to_snap: Vec<SnapCandidatePath>,
}
fn snap_tollerance(document: &DocumentMessageHandler) -> f64 {
	document.snapping_state.tolerance / document.navigation.zoom
}
impl ObjectSnapper {
	fn add_layer_bounds(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier, target: SnapTarget) {
		if !document.snapping_state.target_enabled(target) {
			return;
		}
		let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
			return;
		};
		let bounds = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
		for document_curve in bounds.bezier_lines() {
			self.paths_to_snap.push(SnapCandidatePath {
				document_curve,
				layer,
				start: ManipulatorGroupId::new(),
				target,
				bounds: Some(bounds),
			});
		}
	}
	pub fn collect_paths(&mut self, snap_data: &mut SnapData, first_point: bool) {
		if !first_point {
			return;
		}
		let document = snap_data.document;
		self.paths_to_snap.clear();

		for layer in document.metadata.all_layers() {
			if !document.metadata.is_artboard(layer) {
				continue;
			}
			self.add_layer_bounds(document, layer, SnapTarget::Board(BoardSnapTarget::Edge));
		}
		for &layer in snap_data.get_candidates() {
			let transform = document.metadata.transform_to_document(layer);

			if document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Intersection)) || document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Path)) {
				for subpath in document.metadata.layer_outline(layer) {
					for (start_index, curve) in subpath.iter().enumerate() {
						let document_curve = curve.apply_transformation(|p| transform.transform_point2(p));
						let start = subpath.manipulator_groups()[start_index].id;
						if snap_data.ignore_manipulator(layer, start) || snap_data.ignore_manipulator(layer, subpath.manipulator_groups()[(start_index + 1) % subpath.len()].id) {
							continue;
						}
						self.paths_to_snap.push(SnapCandidatePath {
							document_curve,
							layer,
							start,
							target: SnapTarget::Node(NodeSnapTarget::Path),
							bounds: None,
						});
					}
				}
			}
			if !snap_data.ignore_bounds(layer) {
				self.add_layer_bounds(document, layer, SnapTarget::BoundingBox(BoundingBoxSnapTarget::Edge));
			}
		}
	}
	pub fn free_snap_paths(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {
		self.collect_paths(snap_data, point.source_index == 0);

		let document = snap_data.document;
		let perp = document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Parpendicular));
		let tang = document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Tangent));

		for path in &self.paths_to_snap {
			let time = path.document_curve.project(point.document_point, None);
			let snapped_point_document = path.document_curve.evaluate(bezier_rs::TValue::Parametric(time));

			let distance = snapped_point_document.distance(point.document_point);

			if distance < snap_tollerance(document) {
				snap_results.curves.push(SnappedCurve {
					layer: path.layer,
					start: path.start,
					document_curve: path.document_curve,
					point: SnappedPoint {
						snapped_point_document,
						target: path.target,
						distance,
						tollerance: snap_tollerance(document),
						curves: [path.bounds.is_none().then(|| path.document_curve), None],
						source: point.source,
						target_bounds: path.bounds,
						..Default::default()
					},
				});
				if perp || tang {}
			}
		}
	}

	pub fn snap_paths_constrained(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		let document = snap_data.document;
		self.collect_paths(snap_data, point.source_index == 0);

		let tollerance = snap_tollerance(document);
		let constraint_path = if let SnapConstraint::Circle { centre, radius } = constraint {
			Subpath::new_ellipse(centre - DVec2::splat(radius), centre + DVec2::splat(radius))
		} else {
			let constrained_point = constraint.projection(point.document_point);
			let direction = constraint.direction().normalize_or_zero();
			let start = constrained_point - tollerance * direction;
			let end = constrained_point + tollerance * direction;
			Subpath::<ManipulatorGroupId>::new_line(start, end)
		};

		for path in &self.paths_to_snap {
			for constraint_path in constraint_path.iter() {
				for time in path.document_curve.intersections(&constraint_path, None, None) {
					let snapped_point_document = path.document_curve.evaluate(bezier_rs::TValue::Parametric(time));

					let distance = snapped_point_document.distance(point.document_point);

					if distance < tollerance {
						snap_results.points.push(SnappedPoint {
							snapped_point_document,
							target: path.target,
							distance,
							tollerance,
							curves: [path.bounds.is_none().then(|| path.document_curve), Some(constraint_path)],
							source: point.source,
							target_bounds: path.bounds,
							at_intersection: true,
							..Default::default()
						});
					}
				}
			}
		}
	}

	pub fn collect_anchors(&mut self, snap_data: &mut SnapData, first_point: bool) {
		if !first_point {
			return;
		}
		let document = snap_data.document;
		self.points_to_snap.clear();

		for layer in document.metadata.all_layers() {
			if !document.metadata.is_artboard(layer) {
				continue;
			}
			if document.snapping_state.target_enabled(SnapTarget::Board(BoardSnapTarget::Corner)) {
				let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
					continue;
				};
				let quad = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
				let values = BBoxSnapValues {
					corner_source: SnapSource::Board(BoardSnapSource::Corner),
					corner_target: SnapTarget::Board(BoardSnapTarget::Corner),
					centre_source: SnapSource::Board(BoardSnapSource::Centre),
					centre_target: SnapTarget::Board(BoardSnapTarget::Centre),
					..Default::default()
				};
				get_bbox_points(quad, &mut self.points_to_snap, values);
			}
		}
		for &layer in snap_data.get_candidates() {
			get_layer_snap_points(layer, &snap_data, &mut self.points_to_snap);

			if snap_data.ignore_bounds(layer) {
				continue;
			}
			let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
				continue;
			};
			let quad = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
			let target_enabled = |target: BoardSnapTarget| document.snapping_state.target_enabled(SnapTarget::Board(target));
			let values = BBoxSnapValues::new(target_enabled(BoardSnapTarget::Corner), target_enabled(BoardSnapTarget::Edge), target_enabled(BoardSnapTarget::Centre));
			get_bbox_points(quad, &mut self.points_to_snap, values);
		}
	}
	pub fn snap_anchors(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, c: SnapConstraint, constrained_point: DVec2) {
		self.collect_anchors(snap_data, point.source_index == 0);
		//info!("Points to snap {:#?}", self.points_to_snap);
		let mut best = None;
		for candidate in &self.points_to_snap {
			// Candidate is not on constraint
			if !candidate.document_point.abs_diff_eq(c.projection(candidate.document_point), 1e-5) {
				continue;
			}
			let distance = candidate.document_point.distance(constrained_point);
			let tollerance = snap_tollerance(snap_data.document);

			let candidate_better = |best: &SnappedPoint| {
				if best.snapped_point_document.abs_diff_eq(candidate.document_point, 1e-5) {
					!matches!(candidate.target, SnapTarget::BoundingBox(_))
				} else {
					distance < best.distance
				}
			};
			if distance < tollerance && (best.is_none() || best.as_ref().is_some_and(|best| candidate_better(best))) {
				best = Some(SnappedPoint {
					snapped_point_document: candidate.document_point,
					source: point.source,
					target: candidate.target,
					distance,
					tollerance,
					contrained: true,
					target_bounds: candidate.quad,
					..Default::default()
				});
			}
		}
		if let Some(result) = best {
			snap_results.points.push(result);
		}
	}
	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {
		self.snap_anchors(snap_data, point, snap_results, SnapConstraint::None, point.document_point);
		self.free_snap_paths(snap_data, point, snap_results);
	}

	pub fn contrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		self.snap_anchors(snap_data, point, snap_results, constraint, constraint.projection(point.document_point));
		self.snap_paths_constrained(snap_data, point, snap_results, constraint);
	}
}

fn compare_points(a: &&SnappedPoint, b: &&SnappedPoint) -> Ordering {
	if (a.target.bounding_box() && !b.target.bounding_box()) || (a.at_intersection && !b.at_intersection) {
		Ordering::Greater
	} else if (!a.target.bounding_box() && b.target.bounding_box()) || (!a.at_intersection && b.at_intersection) {
		Ordering::Less
	} else {
		a.distance.partial_cmp(&b.distance).unwrap()
	}
}

fn get_closest_point(points: &[SnappedPoint]) -> Option<&SnappedPoint> {
	points.iter().min_by(compare_points)
}
fn get_closest_curve(curves: &[SnappedCurve], exclude_paths: bool) -> Option<&SnappedPoint> {
	let keep_curve = |curve: &&SnappedCurve| !exclude_paths || curve.point.target != SnapTarget::Node(NodeSnapTarget::Path);
	curves.iter().filter(keep_curve).map(|curve| &curve.point).min_by(compare_points)
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
						target: SnapTarget::Node(NodeSnapTarget::Intersection),
						tollerance: close.point.tollerance,
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
		self.indicator = snapped_point.is_snapped().then(|| snapped_point);
	}
	pub fn clear_indicator(&mut self) {
		self.indicator = None;
	}
	pub fn preview_draw(&mut self, snap_data: &SnapData, mouse: DVec2) {
		let mut point = SnapCandidatePoint::handle(snap_data.document.metadata.document_to_viewport.inverse().transform_point2(mouse));
		point.source_index = 0;
		let point = self.free_snap(snap_data, &point, None, false);
		self.update_indicator(point);
	}

	fn find_best_snap(snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: SnapResults, contrained: bool, off_screen: bool, to_path: bool) -> SnappedPoint {
		let mut snapped_points = Vec::new();
		let document = snap_data.document;

		if let Some(closest_point) = get_closest_point(&snap_results.points) {
			snapped_points.push(closest_point.clone());
		}
		let exclude_paths = !document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Path));
		if let Some(closest_curve) = get_closest_curve(&snap_results.curves, exclude_paths) {
			snapped_points.push(closest_curve.clone());
		}

		if !contrained {
			if document.snapping_state.target_enabled(SnapTarget::Node(NodeSnapTarget::Intersection)) {
				if let Some(closest_curves_intersection) = get_closest_intersection(point.document_point, &snap_results.curves) {
					snapped_points.push(closest_curves_intersection);
				}
			}

			// TODO grid
		}

		if to_path {
			snapped_points.retain(|i| matches!(i.target, SnapTarget::Node(_)));
		}
		//info!("Snap points {snapped_points:#?}");

		let mut best_point = None;

		for point in snapped_points {
			let viewport_point = document.metadata.document_to_viewport.transform_point2(point.snapped_point_document);
			let on_screen = viewport_point.cmpgt(DVec2::ZERO).all() && viewport_point.cmplt(snap_data.input.viewport_bounds.size()).all();
			if !on_screen && !off_screen {
				continue;
			}
			if point.distance > point.tollerance {
				continue;
			}
			if best_point.as_ref().is_some_and(|best: &SnappedPoint| point.other_snap_better(best)) {
				continue;
			}
			best_point = Some(point);
		}

		info!("Best {best_point:#?}");

		best_point.unwrap_or(SnappedPoint {
			snapped_point_document: point.document_point,
			distance: f64::INFINITY,
			..Default::default()
		})
	}

	fn find_candidates(snap_data: &SnapData, point: &SnapCandidatePoint, bbox: Option<Quad>) -> Vec<LayerNodeIdentifier> {
		let document = snap_data.document;
		let offset = snap_tollerance(document);
		let quad = bbox.map_or_else(|| Quad::from_box([point.document_point - offset, point.document_point + offset]), |quad| quad.inflate(offset));
		let mut candidates = Vec::new();

		fn add_candidates(layer: LayerNodeIdentifier, snap_data: &SnapData, quad: Quad, candidates: &mut Vec<LayerNodeIdentifier>) {
			let document = snap_data.document;
			if candidates.len() > 10 {
				return;
			}
			if !document.layer_visible(layer) {
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
		let mut snap_results = SnapResults::default();
		if point.source_index == 0 {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		snap_data.candidates = Some(&*self.candidates.get_or_insert_with(|| Self::find_candidates(&snap_data, point, bbox)));
		self.object_snapper.free_snap(&mut snap_data, point, &mut snap_results);

		Self::find_best_snap(&mut snap_data, point, snap_results, false, false, to_paths)
	}

	pub fn constrained_snap(&mut self, snap_data: &SnapData, point: &SnapCandidatePoint, contraint: SnapConstraint, bbox: Option<Quad>) -> SnappedPoint {
		let mut snap_results = SnapResults::default();
		if point.source_index == 0 {
			self.candidates = None;
		}

		let mut snap_data = snap_data.clone();
		snap_data.candidates = Some(&*self.candidates.get_or_insert_with(|| Self::find_candidates(&snap_data, point, bbox)));
		self.object_snapper.contrained_snap(&mut snap_data, point, &mut snap_results, contraint);

		info!("SR {snap_results:#?}");
		Self::find_best_snap(&mut snap_data, point, snap_results, true, false, false)
	}

	/// Gets a list of snap targets for the X and Y axes (if specified) in Viewport coords for the target layers (usually all layers or all non-selected layers.)
	/// This should be called at the start of a drag.
	pub fn start_snap(&mut self, document_message_handler: &DocumentMessageHandler, input: &InputPreprocessorMessageHandler) {}

	pub fn grid_overlay(&self, document: &DocumentMessageHandler, overlay_context: &mut OverlayContext) {
		let offset = document.snapping_state.grid.origin;
		let spacing = document.snapping_state.grid.size;
		let document_to_viewport = document.metadata().document_to_viewport;
		let bounds = document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, overlay_context.size]);

		for primary in 0..2 {
			let secondary = 1 - primary;
			let min = bounds.0.iter().map(|&corner| corner[secondary]).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
			let max = bounds.0.iter().map(|&corner| corner[secondary]).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
			let primary1 = bounds.0.iter().map(|&corner| corner[primary]).min_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
			let primary2 = bounds.0.iter().map(|&corner| corner[primary]).max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap_or_default();
			let mut spacing = spacing[secondary];
			while (max - min) / spacing > 30. {
				spacing *= 2.;
			}
			for line_index in 0..=((max - min) / spacing).ceil() as i32 {
				let secondary_pos = (((min + offset[secondary]) / spacing).ceil() + line_index as f64) * spacing;
				let start = if primary == 0 { DVec2::new(primary1, secondary_pos) } else { DVec2::new(secondary_pos, primary1) };
				let end = if primary == 0 { DVec2::new(primary2, secondary_pos) } else { DVec2::new(secondary_pos, primary2) };
				overlay_context.line(document_to_viewport.transform_point2(start), document_to_viewport.transform_point2(end));
			}
		}
	}

	pub fn draw_overlays(&mut self, snap_data: SnapData, overlay_context: &mut OverlayContext) {
		if snap_data.document.snapping_state.grid_snapping {
			self.grid_overlay(snap_data.document, overlay_context);
		}
		// let mut snap_results = InterimSnapResults::default();
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

			overlay_context.text(&format!("{:?} to {:?}", ind.source, ind.target), viewport - DVec2::new(0., 5.), "#0008", 3.);
			overlay_context.square(viewport, true);
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
