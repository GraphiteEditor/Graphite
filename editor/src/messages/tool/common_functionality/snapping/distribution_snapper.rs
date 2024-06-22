use std::ops::Range;

use super::*;
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::*;
use crate::messages::prelude::*;
use bezier_rs::{Bezier, Identifier, Subpath, TValue};
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::vector::PointId;

#[derive(Clone, Debug, Default)]
pub struct DistributionSnapper {
	right: Vec<Rect>,
	left: Vec<Rect>,
	down: Vec<Rect>,
	up: Vec<Rect>,
}

impl DistributionSnapper {
	fn add_bounds(&mut self, layer: LayerNodeIdentifier, snap_data: &mut SnapData, bbox_to_snap: Rect, max_extent: f64) {
		let document = snap_data.document;
		let Some(bounds) = document.metadata.bounding_box_with_transform(layer, document.metadata.transform_to_document(layer)) else {
			return;
		};
		let bounds = Rect::from_box(bounds);
		if bounds.intersects(bbox_to_snap) {
			return;
		}
		let difference = bounds.center() - bbox_to_snap.center();
		let x_bounds = bbox_to_snap.expand_by(max_extent, 0.);
		let y_bounds = bbox_to_snap.expand_by(0., max_extent);

		if x_bounds.intersects(bounds) {
			if difference.x > 0. {
				self.right.push(bounds);
			} else {
				self.left.push(bounds);
			}
		} else if y_bounds.intersects(bounds) {
			if difference.x > 0. {
				self.down.push(bounds);
			} else {
				self.up.push(bounds);
			}
		}
	}

	pub fn collect_bounding_box_points(&mut self, snap_data: &mut SnapData, first_point: bool, bbox_to_snap: Rect) {
		if !first_point {
			return;
		}
		let document = snap_data.document;
		self.right.clear();
		self.left.clear();
		self.down.clear();
		self.up.clear();
		let screen_bounds = (document.metadata.document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, snap_data.input.viewport_bounds.size()])).bounding_box();
		let max_extent = (screen_bounds[1] - screen_bounds[0]).max_element();

		for layer in document.metadata.all_layers() {
			if !document.metadata.is_artboard(layer) || snap_data.ignore.contains(&layer) {
				continue;
			}
			self.add_bounds(layer, snap_data, bbox_to_snap, max_extent);
		}
		for &layer in snap_data.alignment_candidates.map_or([].as_slice(), |candidates| candidates.as_slice()) {
			if snap_data.ignore_bounds(layer) {
				continue;
			}
			self.add_bounds(layer, snap_data, bbox_to_snap, max_extent);
		}

		self.right.sort_unstable_by(|a, b| a.center().x.total_cmp(&b.center().x));
		self.left.sort_unstable_by(|a, b| b.center().x.total_cmp(&a.center().x));
		self.down.sort_unstable_by(|a, b| a.center().y.total_cmp(&b.center().y));
		self.up.sort_unstable_by(|a, b| b.center().y.total_cmp(&a.center().y));

		Self::merge_intersecting(&mut self.right);
		Self::merge_intersecting(&mut self.left);
		Self::merge_intersecting(&mut self.down);
		Self::merge_intersecting(&mut self.up);
	}

	fn merge_intersecting(rects: &mut Vec<Rect>) {
		let mut index = 0;
		while index < rects.len() {
			let insert_index = index;
			let mut obelisk = rects[index];
			while index + 1 < rects.len() && rects[index].intersects(rects[index + 1]) {
				index += 1;
				obelisk = Rect::combine_bounds(obelisk, rects[index]);
			}
			if index > insert_index {
				rects.insert(insert_index, obelisk);
				index += 1;
			}
			index += 1;
		}
	}

	fn best_snaps(source: Rect, rects: &[Rect], tolerance: f64, dist: fn(Rect, Rect) -> f64, result: &mut Vec<Rect>) {
		let mut best = None;
		for (index, &rect) in rects.iter().enumerate() {}
	}

	fn find_snaps(source: Rect, rects: &[Rect], tolerance: f64, dist: fn(Rect, Rect) -> f64) -> (Option<f64>, Vec<Rect>) {
		let mut best = None;
		for (index, &rect) in rects.iter().enumerate() {
			let first_dist = dist(source, rect);
			let mut result = Vec::new();
			if !Self::best_snaps(source, &rects[(index + 1)..], tolerance, dist, &mut result) {
				continue;
			}
			result.insert(0, rect);

			if best.as_ref().is_some_and(|(_, best)| best.len() >= result.len()) {
				continue;
			}
			best = Some((first_dist, result));
		}
		(best.as_ref().map(|(dist, _)| *dist), best.map_or_else(|| vec![rects[0]], |(_, result)| result))
	}

	fn snap_bbox_points(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, bounds: Rect) {
		self.collect_bounding_box_points(snap_data, point.source_index == 0, bounds);

		if point.source != SnapSource::BoundingBox(BoundingBoxSnapSource::Center) {
			return;
		}

		let tolerance = snap_tolerance(document);

		let mut consider_x = true;
		let mut consider_y = true;
		if let SnapConstraint::Line { direction, .. } = constraint {
			let direction = direction.normalize_or_zero();
			if direction.x == 0. {
				consider_y = false;
			} else if direction.y == 0. {
				consider_x = false;
			}
		}

		let mut snap_right: Option<SnappedPoint> = None;
		let mut snap_y: Option<SnappedPoint> = None;

		if consider_x && !self.right.is_empty() {}

		// for target_point in &self.bounding_box_points {
		// 	let target_position = target_point.document_point;

		// 	let point_on_x = DVec2::new(point.document_point.x, target_position.y);
		// 	let dist_x = (target_position.y - point.document_point.y).abs();

		// 	let point_on_y = DVec2::new(target_position.x, point.document_point.y);
		// 	let dist_y = (target_position.x - point.document_point.x).abs();

		// 	let target_geometry = matches!(target_point.target, SnapTarget::Geometry(_));
		// 	let updated_target = if target_geometry {
		// 		SnapTarget::Alignment(AlignmentSnapTarget::Handle)
		// 	} else {
		// 		target_point.target
		// 	};

		// 	if consider_x && dist_x < tolerance && snap_x.as_ref().map_or(true, |point| dist_y < point.distance_to_align_target) {
		// 		snap_x = Some(SnappedPoint {
		// 			snapped_point_document: point_on_x,
		// 			source: point.source, //ToDo map source
		// 			target: updated_target,
		// 			target_bounds: target_point.quad,
		// 			distance: dist_x,
		// 			tolerance,
		// 			distance_to_align_target: dist_y,
		// 			alignment_target: Some(target_position),
		// 			fully_constrained: true,
		// 			..Default::default()
		// 		});
		// 	}
		// 	if consider_y && dist_y < tolerance && snap_y.as_ref().map_or(true, |point| dist_x < point.distance_to_align_target) {
		// 		snap_y = Some(SnappedPoint {
		// 			snapped_point_document: point_on_y,
		// 			source: point.source, //ToDo map source
		// 			target: updated_target,
		// 			target_bounds: target_point.quad,
		// 			distance: dist_y,
		// 			tolerance,
		// 			distance_to_align_target: dist_x,
		// 			alignment_target: Some(target_position),
		// 			fully_constrained: true,
		// 			..Default::default()
		// 		});
		// 	}
		// }
		// info!("Snap x {snap_x:?} snap y {snap_y:?}");
		// match (snap_x, snap_y) {
		// 	(Some(snap_x), Some(snap_y)) => {
		// 		let intersection = DVec2::new(snap_y.snapped_point_document.x, snap_x.snapped_point_document.y);
		// 		let distance = intersection.distance(point.document_point);
		// 		if distance >= tolerance {
		// 			snap_results.points.push(if snap_x.distance < snap_y.distance { snap_x } else { snap_y });
		// 			return;
		// 		}
		// 		snap_results.points.push(SnappedPoint {
		// 			snapped_point_document: intersection,
		// 			source: point.source, //ToDo map source
		// 			target: SnapTarget::Alignment(AlignmentSnapTarget::Intersection),
		// 			target_bounds: snap_x.target_bounds,
		// 			distance,
		// 			tolerance,
		// 			alignment_target: snap_x.alignment_target,
		// 			alignment_target_intersect: snap_y.alignment_target,
		// 			constrained: true,
		// 			..Default::default()
		// 		});
		// 	}
		// 	(Some(snap_x), None) => snap_results.points.push(snap_x),
		// 	(None, Some(snap_y)) => snap_results.points.push(snap_y),
		// 	(None, None) => {}
		// }
	}
	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, bounds: Option<Rect>) {
		let Some(bounds) = bounds else { return };
		info!("src {:?}", point.source);

		info!("Snapping points");
		self.snap_bbox_points(snap_data, point, snap_results, SnapConstraint::None, bounds);
	}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, bounds: Option<Rect>) {
		let Some(bounds) = bounds else { return };

		self.snap_bbox_points(snap_data, point, snap_results, constraint, bounds);
	}
}

#[test]
fn merge_intersecting_test() {
	let mut rects = vec![Rect::from_square(DVec2::ZERO, 2.), Rect::from_square(DVec2::new(10., 0.), 2.)];
	DistributionSnapper::merge_intersecting(&mut rects);
	assert_eq!(rects.len(), 2);

	let mut rects = vec![
		Rect::from_square(DVec2::ZERO, 2.),
		Rect::from_square(DVec2::new(1., 0.), 2.),
		Rect::from_square(DVec2::new(10., 0.), 2.),
		Rect::from_square(DVec2::new(11., 0.), 2.),
	];
	DistributionSnapper::merge_intersecting(&mut rects);
	assert_eq!(rects.len(), 6);
	assert_eq!(rects[0], Rect::from_box([DVec2::new(-2., -2.), DVec2::new(3., 2.)]));
	assert_eq!(rects[3], Rect::from_box([DVec2::new(8., -2.), DVec2::new(13., 2.)]));
}
