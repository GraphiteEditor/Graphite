use super::*;
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{
	BoardSnapSource, BoardSnapTarget, BoundingBoxSnapSource, BoundingBoxSnapTarget, GeometrySnapSource, GeometrySnapTarget, SnapSource, SnapTarget,
};
use crate::messages::prelude::*;
use bezier_rs::{Bezier, Identifier, Subpath, TValue};
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::uuid::ManipulatorGroupId;

#[derive(Clone, Debug, Default)]
pub struct LayerSnapper {
	points_to_snap: Vec<SnapCandidatePoint>,
	paths_to_snap: Vec<SnapCandidatePath>,
}

impl LayerSnapper {
	pub fn add_layer_bounds(&mut self, document: &DocumentMessageHandler, layer: LayerNodeIdentifier, target: SnapTarget) {
		if !document.snapping_state.target_enabled(target) {
			return;
		}
		let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
			return;
		};
		let bounds = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
		if bounds.0.iter().any(|point| !point.is_finite()) {
			return;
		}
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
			if !transform.is_finite() {
				continue;
			}

			if document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Intersection)) || document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Path))
			{
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
							target: SnapTarget::Geometry(GeometrySnapTarget::Path),
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
		let normals = document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Normal));
		let tangents = document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Tangent));
		let tolerance = snap_tolerance(document);

		for path in &self.paths_to_snap {
			// Skip very short paths
			if path.document_curve.start.distance_squared(path.document_curve.end) < tolerance * tolerance * 2. {
				continue;
			}
			let time = path.document_curve.project(point.document_point, None);
			let snapped_point_document = path.document_curve.evaluate(bezier_rs::TValue::Parametric(time));

			let distance = snapped_point_document.distance(point.document_point);

			if distance < tolerance {
				snap_results.curves.push(SnappedCurve {
					layer: path.layer,
					start: path.start,
					document_curve: path.document_curve,
					point: SnappedPoint {
						snapped_point_document,
						target: path.target,
						distance,
						tolerance,
						curves: [path.bounds.is_none().then_some(path.document_curve), None],
						source: point.source,
						target_bounds: path.bounds,
						..Default::default()
					},
				});
				normals_and_tangents(path, normals, tangents, point, tolerance, snap_results);
			}
		}
	}

	pub fn snap_paths_constrained(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		let document = snap_data.document;
		self.collect_paths(snap_data, point.source_index == 0);

		let tolerance = snap_tolerance(document);
		let constraint_path = if let SnapConstraint::Circle { center, radius } = constraint {
			Subpath::new_ellipse(center - DVec2::splat(radius), center + DVec2::splat(radius))
		} else {
			let constrained_point = constraint.projection(point.document_point);
			let direction = constraint.direction().normalize_or_zero();
			let start = constrained_point - tolerance * direction;
			let end = constrained_point + tolerance * direction;
			Subpath::<ManipulatorGroupId>::new_line(start, end)
		};

		for path in &self.paths_to_snap {
			for constraint_path in constraint_path.iter() {
				for time in path.document_curve.intersections(&constraint_path, None, None) {
					let snapped_point_document = path.document_curve.evaluate(bezier_rs::TValue::Parametric(time));

					let distance = snapped_point_document.distance(point.document_point);

					if distance < tolerance {
						snap_results.points.push(SnappedPoint {
							snapped_point_document,
							target: path.target,
							distance,
							tolerance,
							curves: [path.bounds.is_none().then_some(path.document_curve), Some(constraint_path)],
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
					center_source: SnapSource::Board(BoardSnapSource::Center),
					center_target: SnapTarget::Board(BoardSnapTarget::Center),
					..Default::default()
				};
				get_bbox_points(quad, &mut self.points_to_snap, values, document);
			}
		}
		for &layer in snap_data.get_candidates() {
			get_layer_snap_points(layer, snap_data, &mut self.points_to_snap);

			if snap_data.ignore_bounds(layer) {
				continue;
			}
			let Some(bounds) = document.metadata.bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
				continue;
			};
			let quad = document.metadata.transform_to_document(layer) * Quad::from_box(bounds);
			let values = BBoxSnapValues::BOUNDING_BOX;
			get_bbox_points(quad, &mut self.points_to_snap, values, document);
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
			let tolerance = snap_tolerance(snap_data.document);

			let candidate_better = |best: &SnappedPoint| {
				if best.snapped_point_document.abs_diff_eq(candidate.document_point, 1e-5) {
					!candidate.target.bounding_box()
				} else {
					distance < best.distance
				}
			};
			if distance < tolerance && (best.is_none() || best.as_ref().is_some_and(candidate_better)) {
				best = Some(SnappedPoint {
					snapped_point_document: candidate.document_point,
					source: point.source,
					target: candidate.target,
					distance,
					tolerance,
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

fn normals_and_tangents(path: &SnapCandidatePath, normals: bool, tangents: bool, point: &SnapCandidatePoint, tolerance: f64, snap_results: &mut SnapResults) {
	if normals && path.bounds.is_none() {
		for &neighbor in &point.neighbors {
			for t in path.document_curve.normals_to_point(neighbor) {
				let normal_point = path.document_curve.evaluate(TValue::Parametric(t));
				let distance = normal_point.distance(point.document_point);
				if distance > tolerance {
					continue;
				}
				snap_results.points.push(SnappedPoint {
					snapped_point_document: normal_point,
					target: SnapTarget::Geometry(GeometrySnapTarget::Normal),
					distance,
					tolerance,
					curves: [Some(path.document_curve), None],
					source: point.source,
					contrained: true,
					..Default::default()
				});
			}
		}
	}
	if tangents && path.bounds.is_none() {
		for &neighbor in &point.neighbors {
			for t in path.document_curve.tangents_to_point(neighbor) {
				let tangent_point = path.document_curve.evaluate(TValue::Parametric(t));
				let distance = tangent_point.distance(point.document_point);
				if distance > tolerance {
					continue;
				}
				snap_results.points.push(SnappedPoint {
					snapped_point_document: tangent_point,
					target: SnapTarget::Geometry(GeometrySnapTarget::Tangent),
					distance,
					tolerance,
					curves: [Some(path.document_curve), None],
					source: point.source,
					contrained: true,
					..Default::default()
				});
			}
		}
	}
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
	pub document_point: DVec2,
	pub source: SnapSource,
	pub target: SnapTarget,
	pub source_index: usize,
	pub quad: Option<Quad>,
	pub neighbors: Vec<DVec2>,
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
			quad,
			..Default::default()
		}
	}
	pub fn new_source(document_point: DVec2, source: SnapSource) -> Self {
		Self::new(document_point, source, SnapTarget::None)
	}
	pub fn handle(document_point: DVec2) -> Self {
		Self::new_source(document_point, SnapSource::Geometry(GeometrySnapSource::Sharp))
	}
	pub fn handle_neighbors(document_point: DVec2, neighbors: impl Into<Vec<DVec2>>) -> Self {
		let mut point = Self::new_source(document_point, SnapSource::Geometry(GeometrySnapSource::Sharp));
		point.neighbors = neighbors.into();
		point
	}
}
#[derive(Default)]
pub struct BBoxSnapValues {
	corner_source: SnapSource,
	corner_target: SnapTarget,
	edge_source: SnapSource,
	edge_target: SnapTarget,
	center_source: SnapSource,
	center_target: SnapTarget,
}
impl BBoxSnapValues {
	pub const BOUNDING_BOX: Self = Self {
		corner_source: SnapSource::BoundingBox(BoundingBoxSnapSource::Corner),
		corner_target: SnapTarget::BoundingBox(BoundingBoxSnapTarget::Corner),
		edge_source: SnapSource::BoundingBox(BoundingBoxSnapSource::EdgeMidpoint),
		edge_target: SnapTarget::BoundingBox(BoundingBoxSnapTarget::EdgeMidpoint),
		center_source: SnapSource::BoundingBox(BoundingBoxSnapSource::Center),
		center_target: SnapTarget::BoundingBox(BoundingBoxSnapTarget::Center),
	};
}
pub fn get_bbox_points(quad: Quad, points: &mut Vec<SnapCandidatePoint>, values: BBoxSnapValues, document: &DocumentMessageHandler) {
	for index in 0..4 {
		let start = quad.0[index];
		let end = quad.0[(index + 1) % 4];
		if document.snapping_state.target_enabled(values.corner_target) {
			points.push(SnapCandidatePoint::new_quad(start, values.corner_source, values.corner_target, Some(quad)));
		}
		if document.snapping_state.target_enabled(values.edge_target) {
			points.push(SnapCandidatePoint::new_quad((start + end) / 2., values.edge_source, values.edge_target, Some(quad)));
		}
	}
	if document.snapping_state.target_enabled(values.center_target) {
		points.push(SnapCandidatePoint::new_quad(quad.center(), values.center_source, values.center_target, Some(quad)));
	}
}

fn handle_not_under(to_document: DAffine2) -> impl Fn(&DVec2) -> bool {
	move |&offset: &DVec2| to_document.transform_vector2(offset).length_squared() >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE
}
fn subpath_anchor_snap_points(layer: LayerNodeIdentifier, subpath: &Subpath<ManipulatorGroupId>, snap_data: &SnapData, points: &mut Vec<SnapCandidatePoint>, to_document: DAffine2) {
	let document = snap_data.document;
	// Midpoints of linear segments
	if document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::LineMidpoint)) {
		for (index, curve) in subpath.iter().enumerate() {
			if snap_data.ignore_manipulator(layer, subpath.manipulator_groups()[index].id) || snap_data.ignore_manipulator(layer, subpath.manipulator_groups()[(index + 1) % subpath.len()].id) {
				continue;
			}

			let in_handle = curve.handle_start().map(|handle| handle - curve.start).filter(handle_not_under(to_document));
			let out_handle = curve.handle_end().map(|handle| handle - curve.end).filter(handle_not_under(to_document));
			if in_handle.is_none() && out_handle.is_none() {
				points.push(SnapCandidatePoint::new(
					to_document.transform_point2(curve.start() * 0.5 + curve.end * 0.5),
					SnapSource::Geometry(GeometrySnapSource::LineMidpoint),
					SnapTarget::Geometry(GeometrySnapTarget::LineMidpoint),
				));
			}
		}
	}
	// Anchors
	for (index, group) in subpath.manipulator_groups().iter().enumerate() {
		if snap_data.ignore_manipulator(layer, group.id) {
			continue;
		}

		let smooth = group_smooth(group, to_document, subpath, index);

		if smooth && document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Smooth)) {
			// Smooth points
			points.push(SnapCandidatePoint::new(
				to_document.transform_point2(group.anchor),
				SnapSource::Geometry(GeometrySnapSource::Smooth),
				SnapTarget::Geometry(GeometrySnapTarget::Smooth),
			));
		} else if !smooth && document.snapping_state.target_enabled(SnapTarget::Geometry(GeometrySnapTarget::Sharp)) {
			// Sharp points
			points.push(SnapCandidatePoint::new(
				to_document.transform_point2(group.anchor),
				SnapSource::Geometry(GeometrySnapSource::Sharp),
				SnapTarget::Geometry(GeometrySnapTarget::Sharp),
			));
		}
	}
}

pub fn group_smooth(group: &bezier_rs::ManipulatorGroup<ManipulatorGroupId>, to_document: DAffine2, subpath: &Subpath<ManipulatorGroupId>, index: usize) -> bool {
	let anchor = group.anchor;
	let handle_in = group.in_handle.map(|handle| anchor - handle).filter(handle_not_under(to_document));
	let handle_out = group.out_handle.map(|handle| handle - anchor).filter(handle_not_under(to_document));
	let at_end = !subpath.closed() && (index == 0 || index == subpath.len() - 1);

	handle_in.is_some_and(|handle_in| handle_out.is_some_and(|handle_out| handle_in.angle_between(handle_out) < 1e-5)) && !at_end
}
pub fn get_layer_snap_points(layer: LayerNodeIdentifier, snap_data: &SnapData, points: &mut Vec<SnapCandidatePoint>) {
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
