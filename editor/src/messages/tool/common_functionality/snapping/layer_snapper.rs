use super::*;
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::*;
use crate::messages::prelude::*;
use bezier_rs::{Bezier, Identifier, Subpath, TValue};
use glam::{DAffine2, DVec2};
use graphene_std::math::math_ext::QuadExt;
use graphene_std::renderer::Quad;
use graphene_std::vector::PointId;

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

		let bounds = if document.network_interface.is_artboard(&layer.to_node(), &[]) {
			document
				.metadata()
				.bounding_box_with_transform(layer, document.metadata().transform_to_document(layer))
				.map(Quad::from_box)
		} else {
			document
				.metadata()
				.bounding_box_with_transform(layer, DAffine2::IDENTITY)
				.map(|bounds| document.metadata().transform_to_document(layer) * Quad::from_box(bounds))
		};
		let Some(bounds) = bounds else { return };

		if bounds.0.iter().any(|point| !point.is_finite()) {
			return;
		}

		for document_curve in bounds.bezier_lines() {
			self.paths_to_snap.push(SnapCandidatePath {
				document_curve,
				layer,
				start: PointId::new(),
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

		for layer in document.metadata().all_layers() {
			if !document.network_interface.is_artboard(&layer.to_node(), &[]) || snap_data.ignore.contains(&layer) {
				continue;
			}
			self.add_layer_bounds(document, layer, SnapTarget::Artboard(ArtboardSnapTarget::AlongEdge));
		}
		for &layer in snap_data.get_candidates() {
			let transform = document.metadata().transform_to_document(layer);
			if !transform.is_finite() {
				continue;
			}

			if document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::IntersectionPoint)) || document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::AlongPath)) {
				for subpath in document.metadata().layer_outline(layer) {
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
							target: SnapTarget::Path(PathSnapTarget::AlongPath),
							bounds: None,
						});
					}
				}
			}
		}
	}

	pub fn free_snap_paths(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, config: SnapTypeConfiguration) {
		self.collect_paths(snap_data, !config.use_existing_candidates);

		let document = snap_data.document;
		let normals = document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::NormalToPath));
		let tangents = document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::TangentToPath));
		let tolerance = snap_tolerance(document);

		for path in &self.paths_to_snap {
			// Skip very short paths
			if path.document_curve.start.distance_squared(path.document_curve.end) < tolerance * tolerance * 2. {
				continue;
			}
			let time = path.document_curve.project(point.document_point);
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
						outline_layers: [path.bounds.is_none().then_some(path.layer), None],
						source: point.source,
						target_bounds: path.bounds,
						..Default::default()
					},
				});
				normals_and_tangents(path, normals, tangents, point, tolerance, snap_results);
			}
		}
	}

	pub fn snap_paths_constrained(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, config: SnapTypeConfiguration) {
		let document = snap_data.document;
		self.collect_paths(snap_data, !config.use_existing_candidates);

		let tolerance = snap_tolerance(document);
		let constraint_path = if let SnapConstraint::Circle { center, radius } = constraint {
			Subpath::new_ellipse(center - DVec2::splat(radius), center + DVec2::splat(radius))
		} else {
			let constrained_point = constraint.projection(point.document_point);
			let direction = constraint.direction().normalize_or_zero();
			let start = constrained_point - tolerance * direction;
			let end = constrained_point + tolerance * direction;
			Subpath::<PointId>::new_line(start, end)
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
							outline_layers: [path.bounds.is_none().then_some(path.layer), None],
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

		for layer in document.metadata().all_layers() {
			if !document.network_interface.is_artboard(&layer.to_node(), &[]) || snap_data.ignore.contains(&layer) {
				continue;
			}
			if self.points_to_snap.len() >= crate::consts::MAX_LAYER_SNAP_POINTS {
				warn!("Snap point overflow; skipping.");
				return;
			}

			if document.snapping_state.target_enabled(SnapTarget::Artboard(ArtboardSnapTarget::CornerPoint)) {
				let Some(bounds) = document
					.network_interface
					.document_metadata()
					.bounding_box_with_transform(layer, document.metadata().transform_to_document(layer))
				else {
					continue;
				};

				get_bbox_points(Quad::from_box(bounds), &mut self.points_to_snap, BBoxSnapValues::ARTBOARD, document);
			}
		}
		for &layer in snap_data.get_candidates() {
			get_layer_snap_points(layer, snap_data, &mut self.points_to_snap);

			if snap_data.ignore_bounds(layer) {
				continue;
			}
			if self.points_to_snap.len() >= crate::consts::MAX_LAYER_SNAP_POINTS {
				warn!("Snap point overflow; skipping.");
				return;
			}
			let Some(bounds) = document.metadata().bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
				continue;
			};
			let quad = document.metadata().transform_to_document(layer) * Quad::from_box(bounds);
			let values = BBoxSnapValues::BOUNDING_BOX;
			get_bbox_points(quad, &mut self.points_to_snap, values, document);
		}
	}

	pub fn snap_anchors(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, c: SnapConstraint, constrained_point: DVec2) {
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
					constrained: true,
					target_bounds: candidate.quad,
					outline_layers: [candidate.outline_layer, None],
					..Default::default()
				});
			}
		}
		if let Some(result) = best {
			snap_results.points.push(result);
		}
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, config: SnapTypeConfiguration) {
		self.collect_anchors(snap_data, !config.use_existing_candidates);
		self.snap_anchors(snap_data, point, snap_results, SnapConstraint::None, point.document_point);
		self.free_snap_paths(snap_data, point, snap_results, config);
	}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, config: SnapTypeConfiguration) {
		self.collect_anchors(snap_data, !config.use_existing_candidates);
		self.snap_anchors(snap_data, point, snap_results, constraint, constraint.projection(point.document_point));
		self.snap_paths_constrained(snap_data, point, snap_results, constraint, config);
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
					target: SnapTarget::Path(PathSnapTarget::NormalToPath),
					distance,
					tolerance,
					outline_layers: [Some(path.layer), None],
					source: point.source,
					constrained: true,
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
					target: SnapTarget::Path(PathSnapTarget::TangentToPath),
					distance,
					tolerance,
					outline_layers: [Some(path.layer), None],
					source: point.source,
					constrained: true,
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
	start: PointId,
	target: SnapTarget,
	bounds: Option<Quad>,
}

#[derive(Clone, Debug, Default)]
pub struct SnapCandidatePoint {
	pub document_point: DVec2,
	pub source: SnapSource,
	pub target: SnapTarget,
	pub quad: Option<Quad>,
	/// This layer is outlined if the snap candidate is used.
	pub outline_layer: Option<LayerNodeIdentifier>,
	pub neighbors: Vec<DVec2>,
	pub alignment: bool,
}
impl SnapCandidatePoint {
	pub fn new(document_point: DVec2, source: SnapSource, target: SnapTarget, outline_layer: Option<LayerNodeIdentifier>) -> Self {
		Self::new_quad(document_point, source, target, None, outline_layer, true)
	}

	pub fn new_quad(document_point: DVec2, source: SnapSource, target: SnapTarget, quad: Option<Quad>, outline_layer: Option<LayerNodeIdentifier>, alignment: bool) -> Self {
		Self {
			document_point,
			source,
			target,
			quad,
			outline_layer,
			alignment,
			..Default::default()
		}
	}

	pub fn new_source(document_point: DVec2, source: SnapSource) -> Self {
		Self::new(document_point, source, SnapTarget::None, None)
	}

	pub fn handle(document_point: DVec2) -> Self {
		Self::new_source(document_point, SnapSource::Path(PathSnapSource::AnchorPointWithFreeHandles))
	}

	pub fn handle_neighbors(document_point: DVec2, neighbors: impl Into<Vec<DVec2>>) -> Self {
		let mut point = Self::new_source(document_point, SnapSource::Path(PathSnapSource::AnchorPointWithFreeHandles));
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
		corner_source: SnapSource::BoundingBox(BoundingBoxSnapSource::CornerPoint),
		corner_target: SnapTarget::BoundingBox(BoundingBoxSnapTarget::CornerPoint),
		edge_source: SnapSource::BoundingBox(BoundingBoxSnapSource::EdgeMidpoint),
		edge_target: SnapTarget::BoundingBox(BoundingBoxSnapTarget::EdgeMidpoint),
		center_source: SnapSource::BoundingBox(BoundingBoxSnapSource::CenterPoint),
		center_target: SnapTarget::BoundingBox(BoundingBoxSnapTarget::CenterPoint),
	};

	pub const ARTBOARD: Self = Self {
		corner_source: SnapSource::Artboard(ArtboardSnapSource::CornerPoint),
		corner_target: SnapTarget::Artboard(ArtboardSnapTarget::CornerPoint),
		edge_source: SnapSource::None,
		edge_target: SnapTarget::None,
		center_source: SnapSource::Artboard(ArtboardSnapSource::CenterPoint),
		center_target: SnapTarget::Artboard(ArtboardSnapTarget::CenterPoint),
	};

	pub const ALIGN_BOUNDING_BOX: Self = Self {
		corner_source: SnapSource::Alignment(AlignmentSnapSource::BoundingBoxCornerPoint),
		corner_target: SnapTarget::Alignment(AlignmentSnapTarget::BoundingBoxCornerPoint),
		edge_source: SnapSource::None,
		edge_target: SnapTarget::None,
		center_source: SnapSource::Alignment(AlignmentSnapSource::BoundingBoxCenterPoint),
		center_target: SnapTarget::Alignment(AlignmentSnapTarget::BoundingBoxCenterPoint),
	};

	pub const ALIGN_ARTBOARD: Self = Self {
		corner_source: SnapSource::Alignment(AlignmentSnapSource::ArtboardCornerPoint),
		corner_target: SnapTarget::Alignment(AlignmentSnapTarget::ArtboardCornerPoint),
		edge_source: SnapSource::None,
		edge_target: SnapTarget::None,
		center_source: SnapSource::Alignment(AlignmentSnapSource::ArtboardCenterPoint),
		center_target: SnapTarget::Alignment(AlignmentSnapTarget::ArtboardCenterPoint),
	};
}

pub fn get_bbox_points(quad: Quad, points: &mut Vec<SnapCandidatePoint>, values: BBoxSnapValues, document: &DocumentMessageHandler) {
	for index in 0..4 {
		let start = quad.0[index];
		let end = quad.0[(index + 1) % 4];
		if document.snapping_state.target_enabled(values.corner_target) {
			points.push(SnapCandidatePoint::new_quad(start, values.corner_source, values.corner_target, Some(quad), None, false));
		}
		if document.snapping_state.target_enabled(values.edge_target) {
			points.push(SnapCandidatePoint::new_quad((start + end) / 2., values.edge_source, values.edge_target, Some(quad), None, false));
		}
	}

	if document.snapping_state.target_enabled(values.center_target) {
		points.push(SnapCandidatePoint::new_quad(quad.center(), values.center_source, values.center_target, Some(quad), None, false));
	}
}

fn handle_not_under(to_document: DAffine2) -> impl Fn(&DVec2) -> bool {
	move |&offset: &DVec2| to_document.transform_vector2(offset).length_squared() >= HIDE_HANDLE_DISTANCE * HIDE_HANDLE_DISTANCE
}

fn subpath_anchor_snap_points(layer: LayerNodeIdentifier, subpath: &Subpath<PointId>, snap_data: &SnapData, points: &mut Vec<SnapCandidatePoint>, to_document: DAffine2) {
	let document = snap_data.document;

	// Midpoints of linear segments
	if document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::LineMidpoint)) {
		for (index, curve) in subpath.iter().enumerate() {
			if snap_data.ignore_manipulator(layer, subpath.manipulator_groups()[index].id) || snap_data.ignore_manipulator(layer, subpath.manipulator_groups()[(index + 1) % subpath.len()].id) {
				continue;
			}
			if points.len() >= crate::consts::MAX_LAYER_SNAP_POINTS {
				return;
			}

			let in_handle = curve.handle_start().map(|handle| handle - curve.start).filter(handle_not_under(to_document));
			let out_handle = curve.handle_end().map(|handle| handle - curve.end).filter(handle_not_under(to_document));
			if in_handle.is_none() && out_handle.is_none() {
				points.push(SnapCandidatePoint::new(
					to_document.transform_point2(curve.start() * 0.5 + curve.end * 0.5),
					SnapSource::Path(PathSnapSource::LineMidpoint),
					SnapTarget::Path(PathSnapTarget::LineMidpoint),
					Some(layer),
				));
			}
		}
	}

	// Anchors
	for (index, group) in subpath.manipulator_groups().iter().enumerate() {
		if snap_data.ignore_manipulator(layer, group.id) {
			continue;
		}

		if points.len() >= crate::consts::MAX_LAYER_SNAP_POINTS {
			return;
		}

		let colinear = are_manipulator_handles_colinear(group, to_document, subpath, index);

		// Colinear handles
		if colinear && document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::AnchorPointWithColinearHandles)) {
			points.push(SnapCandidatePoint::new(
				to_document.transform_point2(group.anchor),
				SnapSource::Path(PathSnapSource::AnchorPointWithColinearHandles),
				SnapTarget::Path(PathSnapTarget::AnchorPointWithColinearHandles),
				Some(layer),
			));
		}
		// Free handles
		else if !colinear && document.snapping_state.target_enabled(SnapTarget::Path(PathSnapTarget::AnchorPointWithFreeHandles)) {
			points.push(SnapCandidatePoint::new(
				to_document.transform_point2(group.anchor),
				SnapSource::Path(PathSnapSource::AnchorPointWithFreeHandles),
				SnapTarget::Path(PathSnapTarget::AnchorPointWithFreeHandles),
				Some(layer),
			));
		}
	}
}

pub fn are_manipulator_handles_colinear(group: &bezier_rs::ManipulatorGroup<PointId>, to_document: DAffine2, subpath: &Subpath<PointId>, index: usize) -> bool {
	let anchor = group.anchor;
	let handle_in = group.in_handle.map(|handle| anchor - handle).filter(handle_not_under(to_document));
	let handle_out = group.out_handle.map(|handle| handle - anchor).filter(handle_not_under(to_document));
	let anchor_is_endpoint = !subpath.closed() && (index == 0 || index == subpath.len() - 1);

	// Unless this is an endpoint, check if both handles are colinear (within an angular epsilon)
	!anchor_is_endpoint && handle_in.is_some_and(|handle_in| handle_out.is_some_and(|handle_out| handle_in.angle_to(handle_out) < 1e-5))
}

pub fn get_layer_snap_points(layer: LayerNodeIdentifier, snap_data: &SnapData, points: &mut Vec<SnapCandidatePoint>) {
	let document = snap_data.document;

	if document.network_interface.is_artboard(&layer.to_node(), &[]) {
		return;
	}
	if points.len() >= crate::consts::MAX_LAYER_SNAP_POINTS {
		return;
	}

	if layer.has_children(document.metadata()) {
		for child in layer.descendants(document.metadata()) {
			get_layer_snap_points(child, snap_data, points);
		}
	} else if document.metadata().layer_outline(layer).next().is_some() {
		let to_document = document.metadata().transform_to_document(layer);
		for subpath in document.metadata().layer_outline(layer) {
			subpath_anchor_snap_points(layer, subpath, snap_data, points, to_document);
		}
	}
}
