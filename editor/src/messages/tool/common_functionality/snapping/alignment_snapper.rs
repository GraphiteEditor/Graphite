use super::*;
use crate::messages::portfolio::document::utility_types::misc::*;
use glam::{DAffine2, DVec2};
use graphene_std::renderer::Quad;

#[derive(Clone, Debug, Default)]
pub struct AlignmentSnapper {
	bounding_box_points: Vec<SnapCandidatePoint>,
}

impl AlignmentSnapper {
	pub fn collect_bounding_box_points(&mut self, snap_data: &mut SnapData, first_point: bool) {
		if !first_point {
			return;
		}

		let document = snap_data.document;

		self.bounding_box_points.clear();
		if !document.snapping_state.bounding_box.align_with_edges {
			return;
		}

		for layer in document.metadata().all_layers() {
			if !document.network_interface.is_artboard(&layer.to_node(), &[]) || snap_data.ignore.contains(&layer) {
				continue;
			}

			if document.snapping_state.target_enabled(SnapTarget::Artboard(ArtboardSnapTarget::CornerPoint)) {
				let Some(bounds) = document.metadata().bounding_box_with_transform(layer, document.metadata().transform_to_document(layer)) else {
					continue;
				};

				get_bbox_points(Quad::from_box(bounds), &mut self.bounding_box_points, BBoxSnapValues::ALIGN_ARTBOARD, document);
			}
		}
		for &layer in snap_data.alignment_candidates.map_or([].as_slice(), |candidates| candidates.as_slice()) {
			if snap_data.ignore_bounds(layer) {
				continue;
			}
			let Some(bounds) = document.metadata().bounding_box_with_transform(layer, DAffine2::IDENTITY) else {
				continue;
			};

			let quad = document.metadata().transform_to_document(layer) * Quad::from_box(bounds);
			let values = BBoxSnapValues::ALIGN_BOUNDING_BOX;
			get_bbox_points(quad, &mut self.bounding_box_points, values, document);
		}
	}

	pub fn snap_bbox_points(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, config: SnapTypeConfiguration) {
		self.collect_bounding_box_points(snap_data, !config.use_existing_candidates);
		let unselected_geometry = if snap_data.document.snapping_state.target_enabled(SnapTarget::Alignment(AlignmentSnapTarget::AlignWithAnchorPoint)) {
			snap_data.node_snap_cache.map(|cache| cache.unselected.as_slice()).unwrap_or(&[])
		} else {
			&[]
		};

		// TODO: snap handle points
		let document = snap_data.document;
		let tolerance = snap_tolerance(document);
		let tolerance_squared = tolerance.powi(2);
		let mut snap_x: Option<SnappedPoint> = None;
		let mut snap_y: Option<SnappedPoint> = None;

		for target_point in self.bounding_box_points.iter().chain(unselected_geometry) {
			let target_position = target_point.document_point;

			// Perpendicular snap for line's endpoints
			if let Some(quad) = target_point.quad.map(|q| q.0) {
				if quad[0] == quad[3] && quad[1] == quad[2] && quad[0] == target_point.document_point {
					let [p1, p2, ..] = quad;
					let Some(direction) = (p2 - p1).try_normalize() else { return };
					let normal = DVec2::new(-direction.y, direction.x);

					for endpoint in [p1, p2] {
						if let Some(perpendicular_snap) = Quad::intersect_rays(point.document_point, direction, endpoint, normal) {
							let distance_squared = point.document_point.distance_squared(perpendicular_snap);
							if distance_squared < tolerance_squared {
								let distance = distance_squared.sqrt();
								let distance_to_align_target = perpendicular_snap.distance_squared(endpoint).sqrt();

								let snap_point = SnappedPoint {
									snapped_point_document: perpendicular_snap,
									source: point.source,
									target: SnapTarget::Alignment(AlignmentSnapTarget::PerpendicularToEndpoint),
									target_bounds: Some(Quad(quad)),
									distance,
									tolerance,
									distance_to_align_target,
									fully_constrained: false,
									at_intersection: true,
									alignment_target_horizontal: Some(endpoint),
									..Default::default()
								};
								snap_results.points.push(snap_point);
							}
						}
					}
				}
			}
			let [point_on_x, point_on_y] = if let SnapConstraint::Line { origin, direction } = constraint {
				[
					Quad::intersect_rays(target_point.document_point, DVec2::Y, origin, direction),
					Quad::intersect_rays(target_point.document_point, DVec2::X, origin, direction),
				]
			} else {
				let Some(quad) = target_point.quad.map(|quad| quad.0) else { continue };
				let edges = [quad[1] - quad[0], quad[3] - quad[0]];
				edges.map(|edge| edge.try_normalize().map(|edge| (point.document_point - target_position).project_onto(edge) + target_position))
			};

			let target_path = matches!(target_point.target, SnapTarget::Path(_));
			let updated_target = if target_path {
				SnapTarget::Alignment(AlignmentSnapTarget::AlignWithAnchorPoint)
			} else {
				target_point.target
			};

			if let Some(point_on_x) = point_on_x {
				let distance_to_snapped = point.document_point.distance(point_on_x);
				let distance_to_align_target = point_on_x.distance(target_position);
				if distance_to_snapped < tolerance && snap_x.as_ref().is_none_or(|point| distance_to_align_target < point.distance_to_align_target) {
					snap_x = Some(SnappedPoint {
						snapped_point_document: point_on_x,
						source: point.source, // TODO(0Hypercube): map source
						target: updated_target,
						target_bounds: target_point.quad,
						distance: distance_to_snapped,
						tolerance,
						distance_to_align_target,
						alignment_target_horizontal: Some(target_position),
						fully_constrained: true,
						at_intersection: matches!(constraint, SnapConstraint::Line { .. }),
						..Default::default()
					});
				}
			}
			if let Some(point_on_y) = point_on_y {
				let distance_to_snapped = point.document_point.distance(point_on_y);
				let distance_to_align_target = point_on_y.distance(target_position);
				if distance_to_snapped < tolerance && snap_y.as_ref().is_none_or(|point| distance_to_align_target < point.distance_to_align_target) {
					snap_y = Some(SnappedPoint {
						snapped_point_document: point_on_y,
						source: point.source, // TODO(0Hypercube): map source
						target: updated_target,
						target_bounds: target_point.quad,
						distance: distance_to_snapped,
						tolerance,
						distance_to_align_target,
						alignment_target_vertical: Some(target_position),
						fully_constrained: true,
						at_intersection: matches!(constraint, SnapConstraint::Line { .. }),
						..Default::default()
					});
				}
			}
		}

		match (snap_x, snap_y) {
			(Some(snap_x), Some(snap_y)) if !matches!(constraint, SnapConstraint::Line { .. }) => {
				let intersection = DVec2::new(snap_y.snapped_point_document.x, snap_x.snapped_point_document.y);
				let distance = intersection.distance(point.document_point);

				if distance >= tolerance {
					snap_results.points.push(if snap_x.distance < snap_y.distance { snap_x } else { snap_y });
					return;
				}

				snap_results.points.push(SnappedPoint {
					snapped_point_document: intersection,
					source: point.source, // TODO: map source
					target: SnapTarget::Alignment(AlignmentSnapTarget::IntersectionPoint),
					target_bounds: snap_x.target_bounds,
					distance,
					tolerance,
					alignment_target_horizontal: snap_x.alignment_target_horizontal,
					alignment_target_vertical: snap_y.alignment_target_vertical,
					constrained: true,
					at_intersection: true,
					..Default::default()
				});
			}
			(Some(snap_x), Some(snap_y)) => snap_results.points.push(if snap_x.distance < snap_y.distance { snap_x } else { snap_y }),
			(Some(snap_x), _) => snap_results.points.push(snap_x),
			(_, Some(snap_y)) => snap_results.points.push(snap_y),
			_ => {}
		}
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, config: SnapTypeConfiguration) {
		let is_bbox = matches!(point.source, SnapSource::BoundingBox(_));
		let is_path = matches!(point.source, SnapSource::Path(_));
		let path_selected = snap_data.has_manipulators();

		if is_bbox || (is_path && path_selected) || (is_path && point.alignment) {
			self.snap_bbox_points(snap_data, point, snap_results, SnapConstraint::None, config);
		}
	}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, config: SnapTypeConfiguration) {
		let is_bbox = matches!(point.source, SnapSource::BoundingBox(_));
		let is_path = matches!(point.source, SnapSource::Path(_));
		let path_selected = snap_data.has_manipulators();

		if is_bbox || (is_path && path_selected) || (is_path && point.alignment) {
			self.snap_bbox_points(snap_data, point, snap_results, constraint, config);
		}
	}
}
