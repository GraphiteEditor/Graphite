use super::*;
use crate::messages::portfolio::document::utility_types::misc::*;

use graphene_core::renderer::Quad;

use glam::{DAffine2, DVec2};

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
		if !document.snapping_state.bounds.align {
			return;
		}

		for layer in document.metadata().all_layers() {
			if !document.network_interface.is_artboard(&layer.to_node(), &[]) || snap_data.ignore.contains(&layer) {
				continue;
			}

			if document.snapping_state.target_enabled(SnapTarget::Artboard(ArtboardSnapTarget::Corner)) {
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

	pub fn snap_bbox_points(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		self.collect_bounding_box_points(snap_data, point.source_index == 0);
		let unselected_geometry = if snap_data.document.snapping_state.target_enabled(SnapTarget::Alignment(AlignmentSnapTarget::Handle)) {
			snap_data.node_snap_cache.map(|cache| cache.unselected.as_slice()).unwrap_or(&[])
		} else {
			&[]
		};

		// TODO: snap handle points
		let document = snap_data.document;
		let tolerance = snap_tolerance(document);

		let mut consider_x = true;
		let mut consider_y = true;
		if let SnapConstraint::Line { direction, .. } = constraint {
			let direction = direction.normalize_or_zero();
			if direction.x.abs() < 1e-5 {
				consider_y = false;
			} else if direction.y.abs() < 1e-5 {
				consider_x = false;
			}
		}

		let mut snap_x: Option<SnappedPoint> = None;
		let mut snap_y: Option<SnappedPoint> = None;

		for target_point in self.bounding_box_points.iter().chain(unselected_geometry) {
			let target_position = target_point.document_point;

			let point_on_x = DVec2::new(point.document_point.x, target_position.y);
			let dist_x = (target_position.y - point.document_point.y).abs();

			let point_on_y = DVec2::new(target_position.x, point.document_point.y);
			let dist_y = (target_position.x - point.document_point.x).abs();

			let target_geometry = matches!(target_point.target, SnapTarget::Geometry(_));
			let updated_target = if target_geometry {
				SnapTarget::Alignment(AlignmentSnapTarget::Handle)
			} else {
				target_point.target
			};

			if consider_x && dist_x < tolerance && snap_x.as_ref().map_or(true, |point| dist_y < point.distance_to_align_target) {
				snap_x = Some(SnappedPoint {
					snapped_point_document: point_on_x,
					source: point.source, //ToDo map source
					target: updated_target,
					target_bounds: target_point.quad,
					distance: dist_x,
					tolerance,
					distance_to_align_target: dist_y,
					alignment_target_x: Some(target_position),
					fully_constrained: true,
					..Default::default()
				});
			}
			if consider_y && dist_y < tolerance && snap_y.as_ref().map_or(true, |point| dist_x < point.distance_to_align_target) {
				snap_y = Some(SnappedPoint {
					snapped_point_document: point_on_y,
					source: point.source, //ToDo map source
					target: updated_target,
					target_bounds: target_point.quad,
					distance: dist_y,
					tolerance,
					distance_to_align_target: dist_x,
					alignment_target_y: Some(target_position),
					fully_constrained: true,
					..Default::default()
				});
			}
		}

		match (snap_x, snap_y) {
			(Some(snap_x), Some(snap_y)) => {
				let intersection = DVec2::new(snap_y.snapped_point_document.x, snap_x.snapped_point_document.y);
				let distance = intersection.distance(point.document_point);

				if distance >= tolerance {
					snap_results.points.push(if snap_x.distance < snap_y.distance { snap_x } else { snap_y });
					return;
				}

				snap_results.points.push(SnappedPoint {
					snapped_point_document: intersection,
					source: point.source, // TODO: map source
					target: SnapTarget::Alignment(AlignmentSnapTarget::Intersection),
					target_bounds: snap_x.target_bounds,
					distance,
					tolerance,
					alignment_target_x: snap_x.alignment_target_x,
					alignment_target_y: snap_y.alignment_target_y,
					constrained: true,
					..Default::default()
				});
			}
			(Some(snap_x), None) => snap_results.points.push(snap_x),
			(None, Some(snap_y)) => snap_results.points.push(snap_y),
			(None, None) => {}
		}
	}
	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {
		let is_bbox = matches!(point.source, SnapSource::BoundingBox(_));
		let is_geometry = matches!(point.source, SnapSource::Geometry(_));
		let geometry_selected = snap_data.has_manipulators();

		if is_bbox || (is_geometry && geometry_selected) || (is_geometry && point.alignment) {
			self.snap_bbox_points(snap_data, point, snap_results, SnapConstraint::None);
		}
	}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		let is_bbox = matches!(point.source, SnapSource::BoundingBox(_));
		let is_geometry = matches!(point.source, SnapSource::Geometry(_));
		let geometry_selected = snap_data.has_manipulators();

		if is_bbox || (is_geometry && geometry_selected) || (is_geometry && point.alignment) {
			self.snap_bbox_points(snap_data, point, snap_results, constraint);
		}
	}
}
