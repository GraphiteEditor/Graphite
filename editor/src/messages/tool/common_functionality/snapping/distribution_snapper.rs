use super::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::*;
use glam::DVec2;
use graphene_std::renderer::Quad;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct DistributionSnapper {
	right: Vec<Rect>,
	left: Vec<Rect>,
	down: Vec<Rect>,
	up: Vec<Rect>,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
pub struct DistributionMatch {
	pub equal: f64,
	pub first: f64,
}

fn dist_right(a: Rect, b: Rect) -> f64 {
	-a.max().x + b.min().x
}
fn dist_left(a: Rect, b: Rect) -> f64 {
	a.min().x - b.max().x
}
fn dist_down(a: Rect, b: Rect) -> f64 {
	-a.max().y + b.min().y
}
fn dist_up(a: Rect, b: Rect) -> f64 {
	a.min().y - b.max().y
}

impl DistributionSnapper {
	fn add_bounds(&mut self, layer: LayerNodeIdentifier, snap_data: &mut SnapData, bbox_to_snap: Rect, max_extent: f64) {
		let document = snap_data.document;

		let Some(bounds) = document.metadata().bounding_box_with_transform(layer, document.metadata().transform_to_document(layer)) else {
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
			if difference.y > 0. {
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

		let screen_bounds = (document.metadata().document_to_viewport.inverse() * Quad::from_box([DVec2::ZERO, snap_data.input.viewport_bounds.size()])).bounding_box();
		let max_extent = (screen_bounds[1] - screen_bounds[0]).abs().max_element();

		// Collect artboard bounds
		for layer in document.metadata().all_layers() {
			if document.network_interface.is_artboard(&layer.to_node(), &[]) && !snap_data.ignore.contains(&layer) {
				self.add_bounds(layer, snap_data, bbox_to_snap, max_extent);
			}
		}

		// Collect alignment candidate bounds
		for &layer in snap_data.alignment_candidates.map_or([].as_slice(), |candidates| candidates.as_slice()) {
			if !snap_data.ignore_bounds(layer) {
				self.add_bounds(layer, snap_data, bbox_to_snap, max_extent);
			}
		}

		// Sort and merge intersecting rectangles
		self.right.sort_unstable_by(|a, b| a.center().x.total_cmp(&b.center().x));
		self.left.sort_unstable_by(|a, b| b.center().x.total_cmp(&a.center().x));
		self.down.sort_unstable_by(|a, b| a.center().y.total_cmp(&b.center().y));
		self.up.sort_unstable_by(|a, b| b.center().y.total_cmp(&a.center().y));

		Self::merge_intersecting(&mut self.right);
		Self::merge_intersecting(&mut self.left);
		Self::merge_intersecting(&mut self.down);
		Self::merge_intersecting(&mut self.up);
	}

	fn merge_intersecting(rectangles: &mut Vec<Rect>) {
		let mut index = 0;
		while index < rectangles.len() {
			let insert_index = index;
			let mut obelisk = rectangles[index];

			while index + 1 < rectangles.len() && rectangles[index].intersects(rectangles[index + 1]) {
				index += 1;
				obelisk = Rect::combine_bounds(obelisk, rectangles[index]);
			}

			if index > insert_index {
				rectangles.insert(insert_index, obelisk);
				index += 1;
			}

			index += 1;
		}
	}

	fn exact_further_matches(source: Rect, rectangles: &[Rect], dist_fn: fn(Rect, Rect) -> f64, first_dist: f64, depth: u8) -> VecDeque<Rect> {
		if rectangles.is_empty() || depth > 10 {
			return VecDeque::from([source]);
		}

		for (index, &rect) in rectangles.iter().enumerate() {
			let next_dist = dist_fn(source, rect);

			if (first_dist - next_dist).abs() < 5e-5 * depth as f64 {
				let mut results = Self::exact_further_matches(rect, &rectangles[(index + 1)..], dist_fn, first_dist, depth + 1);
				results.push_front(source);
				return results;
			}
		}

		VecDeque::from([source])
	}

	fn matches_within_tolerance(source: Rect, rectangles: &[Rect], tolerance: f64, dist_fn: fn(Rect, Rect) -> f64, first_dist: f64) -> Option<(f64, VecDeque<Rect>)> {
		for (index, &rect) in rectangles.iter().enumerate() {
			let next_dist = dist_fn(source, rect);

			if (first_dist - next_dist).abs() < tolerance {
				let this_dist = next_dist;
				let results = Self::exact_further_matches(rect, &rectangles[(index + 1)..], dist_fn, this_dist, 2);
				return Some((this_dist, results));
			}
		}

		None
	}

	fn top_level_matches(source: Rect, rectangles: &[Rect], tolerance: f64, dist_fn: fn(Rect, Rect) -> f64) -> (Option<DistributionMatch>, VecDeque<Rect>) {
		if rectangles.is_empty() {
			return (None, VecDeque::new());
		}

		let mut best: Option<(DistributionMatch, Rect, VecDeque<Rect>)> = None;
		for (index, &rect) in rectangles.iter().enumerate() {
			let first_dist = dist_fn(source, rect);

			let Some((equal_dist, results)) = Self::matches_within_tolerance(rect, &rectangles[(index + 1)..], tolerance, dist_fn, first_dist) else {
				continue;
			};
			if best.as_ref().is_some_and(|(_, _, best)| best.len() >= results.len()) {
				continue;
			}

			best = Some((DistributionMatch { first: first_dist, equal: equal_dist }, rect, results));
		}

		if let Some((dist, rect, mut results)) = best {
			results.push_front(rect);
			(Some(dist), results)
		} else {
			(None, VecDeque::from([rectangles[0]]))
		}
	}

	fn snap_bbox_points(&self, tolerance: f64, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, bounds: Rect) {
		let mut consider_x = true;
		let mut consider_y = true;

		if let SnapConstraint::Line { direction, .. } = constraint {
			let direction = direction.normalize_or_zero();
			consider_x = direction.x != 0.;
			consider_y = direction.y != 0.;
		}

		let mut snap_x: Option<SnappedPoint> = None;
		let mut snap_y: Option<SnappedPoint> = None;

		self.horizontal_snap(consider_x, bounds, tolerance, &mut snap_x, point);
		self.vertical_snap(consider_y, bounds, tolerance, &mut snap_y, point);

		match (snap_x, snap_y) {
			(Some(x), Some(y)) => {
				let x_bounds = Rect::from_box(x.source_bounds.unwrap_or_default().bounding_box());
				let y_bounds = Rect::from_box(y.source_bounds.unwrap_or_default().bounding_box());
				let final_bounds = Rect::from_box([0, 1].map(|index| DVec2::new(x_bounds[index].x, y_bounds[index].y)));

				let mut final_point = x;
				final_point.snapped_point_document += y.snapped_point_document - point.document_point;
				final_point.source_bounds = Some(final_bounds.into());
				final_point.target = SnapTarget::DistributeEvenly(DistributionSnapTarget::XY);
				final_point.distribution_boxes_vertical = y.distribution_boxes_vertical;
				final_point.distribution_equal_distance_vertical = y.distribution_equal_distance_vertical;
				final_point.distance = (final_point.distance * final_point.distance + y.distance * y.distance).sqrt();
				snap_results.points.push(final_point);
			}
			(Some(x), None) => snap_results.points.push(x),
			(None, Some(y)) => snap_results.points.push(y),
			(None, None) => {}
		}
	}

	fn horizontal_snap(&self, consider_x: bool, bounds: Rect, tolerance: f64, snap_x: &mut Option<SnappedPoint>, point: &SnapCandidatePoint) {
		if !consider_x {
			return;
		}

		// Try right distribution first
		if !self.right.is_empty() {
			let (equal_dist, mut vec_right) = Self::top_level_matches(bounds, &self.right, tolerance, dist_right);
			if let Some(distances) = equal_dist {
				let translation = DVec2::X * (distances.first - distances.equal);
				vec_right.push_front(bounds.translate(translation));

				// Find matching left distribution
				for &left in Self::exact_further_matches(bounds.translate(translation), &self.left, dist_left, distances.equal, 2).iter().skip(1) {
					vec_right.push_front(left);
				}

				// Adjust bounds to maintain alignment
				if vec_right.len() > 1 {
					vec_right[0][0].y = vec_right[0][0].y.min(vec_right[1][1].y);
					vec_right[0][1].y = vec_right[0][1].y.min(vec_right[1][1].y);
				}

				*snap_x = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Right, vec_right, distances, bounds, translation, tolerance));
				return;
			}
		}

		// Try left distribution if right didn't work
		if !self.left.is_empty() {
			let (equal_dist, mut vec_left) = Self::top_level_matches(bounds, &self.left, tolerance, dist_left);
			if let Some(distances) = equal_dist {
				let translation = -DVec2::X * (distances.first - distances.equal);
				vec_left.make_contiguous().reverse();
				vec_left.push_back(bounds.translate(translation));

				// Find matching right distribution
				for &right in Self::exact_further_matches(bounds.translate(translation), &self.right, dist_right, distances.equal, 2).iter().skip(1) {
					vec_left.push_back(right);
				}

				*snap_x = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Left, vec_left, distances, bounds, translation, tolerance));
				return;
			}
		}

		// Try center distribution if both sides exist
		if !self.left.is_empty() && !self.right.is_empty() {
			let target_x = (self.right[0].min() + self.left[0].max()).x / 2.;
			let offset = target_x - bounds.center().x;

			if offset.abs() < tolerance {
				let translation = DVec2::X * offset;
				let equal = bounds.translate(translation).min().x - self.left[0].max().x;
				let first = equal + offset;
				let distances = DistributionMatch { first, equal };

				let mut boxes = VecDeque::from([self.left[0], bounds.translate(translation), self.right[0]]);

				// Adjust bounds to maintain alignment
				if boxes.len() > 1 {
					boxes[1][0].y = boxes[1][0].y.min(boxes[0][1].y);
					boxes[1][1].y = boxes[1][1].y.min(boxes[0][1].y);
				}

				*snap_x = Some(SnappedPoint::distribute(point, DistributionSnapTarget::X, boxes, distances, bounds, translation, tolerance));
			}
		}
	}

	fn vertical_snap(&self, consider_y: bool, bounds: Rect, tolerance: f64, snap_y: &mut Option<SnappedPoint>, point: &SnapCandidatePoint) {
		if !consider_y {
			return;
		}

		// Try down distribution first
		if !self.down.is_empty() {
			let (equal_dist, mut vec_down) = Self::top_level_matches(bounds, &self.down, tolerance, dist_down);
			if let Some(distances) = equal_dist {
				let translation = DVec2::Y * (distances.first - distances.equal);
				vec_down.push_front(bounds.translate(translation));

				// Find matching up distribution
				for &up in Self::exact_further_matches(bounds.translate(translation), &self.up, dist_up, distances.equal, 2).iter().skip(1) {
					vec_down.push_front(up);
				}

				// Adjust bounds to maintain alignment
				if vec_down.len() > 1 {
					vec_down[0][0].x = vec_down[0][0].x.min(vec_down[1][1].x);
					vec_down[0][1].x = vec_down[0][1].x.min(vec_down[1][1].x);
				}

				*snap_y = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Down, vec_down, distances, bounds, translation, tolerance));
				return;
			}
		}

		// Try up distribution if down didn't work
		if !self.up.is_empty() {
			let (equal_dist, mut vec_up) = Self::top_level_matches(bounds, &self.up, tolerance, dist_up);
			if let Some(distances) = equal_dist {
				let translation = -DVec2::Y * (distances.first - distances.equal);
				vec_up.make_contiguous().reverse();
				vec_up.push_back(bounds.translate(translation));

				// Find matching down distribution
				for &down in Self::exact_further_matches(bounds.translate(translation), &self.down, dist_down, distances.equal, 2).iter().skip(1) {
					vec_up.push_back(down);
				}

				*snap_y = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Up, vec_up, distances, bounds, translation, tolerance));
				return;
			}
		}

		// Try center distribution if both sides exist
		if !self.up.is_empty() && !self.down.is_empty() {
			let target_y = (self.down[0].min() + self.up[0].max()).y / 2.;
			let offset = target_y - bounds.center().y;

			if offset.abs() < tolerance {
				let translation = DVec2::Y * offset;
				let equal = bounds.translate(translation).min().y - self.up[0].max().y;
				let first = equal + offset;
				let distances = DistributionMatch { first, equal };

				let mut boxes = VecDeque::from([self.up[0], bounds.translate(translation), self.down[0]]);

				// Adjust bounds to maintain alignment
				if boxes.len() > 1 {
					boxes[1][0].x = boxes[1][0].x.min(boxes[0][1].x);
					boxes[1][1].x = boxes[1][1].x.min(boxes[0][1].x);
				}

				*snap_y = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Y, boxes, distances, bounds, translation, tolerance));
			}
		}
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, config: SnapTypeConfiguration) {
		let Some(bounds) = config.bbox else { return };
		if !snap_data.document.snapping_state.bounding_box.distribute_evenly {
			return;
		}

		self.collect_bounding_box_points(snap_data, config.accept_distribution, bounds);
		self.snap_bbox_points(snap_tolerance(snap_data.document), point, snap_results, SnapConstraint::None, bounds);
	}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, config: SnapTypeConfiguration) {
		let Some(bounds) = config.bbox else { return };
		if !snap_data.document.snapping_state.bounding_box.distribute_evenly {
			return;
		}

		self.collect_bounding_box_points(snap_data, config.accept_distribution, bounds);
		self.snap_bbox_points(snap_tolerance(snap_data.document), point, snap_results, constraint, bounds);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn merge_intersecting_test() {
		let mut rectangles = vec![Rect::from_square(DVec2::ZERO, 2.), Rect::from_square(DVec2::new(10., 0.), 2.)];
		DistributionSnapper::merge_intersecting(&mut rectangles);
		assert_eq!(rectangles.len(), 2);

		let mut rectangles = vec![
			Rect::from_square(DVec2::ZERO, 2.),
			Rect::from_square(DVec2::new(1., 0.), 2.),
			Rect::from_square(DVec2::new(10., 0.), 2.),
			Rect::from_square(DVec2::new(11., 0.), 2.),
		];
		DistributionSnapper::merge_intersecting(&mut rectangles);
		assert_eq!(rectangles.len(), 6);
		assert_eq!(rectangles[0], Rect::from_box([DVec2::new(-2., -2.), DVec2::new(3., 2.)]));
		assert_eq!(rectangles[3], Rect::from_box([DVec2::new(8., -2.), DVec2::new(13., 2.)]));
	}

	#[test]
	fn dist_simple_2() {
		let rectangles = [10., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let (offset, rectangles) = DistributionSnapper::top_level_matches(source, &rectangles, 1., dist_right);
		assert_eq!(offset, Some(DistributionMatch { first: 5.5, equal: 6. }));
		assert_eq!(rectangles.len(), 2);
	}

	#[test]
	fn dist_simple_3() {
		let rectangles = [10., 20., 30.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let (offset, rectangles) = DistributionSnapper::top_level_matches(source, &rectangles, 1., dist_right);
		assert_eq!(offset, Some(DistributionMatch { first: 5.5, equal: 6. }));
		assert_eq!(rectangles.len(), 3);
	}

	#[test]
	fn dist_out_of_tolerance() {
		let rectangles = [10., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let (offset, rectangles) = DistributionSnapper::top_level_matches(source, &rectangles, 0.4, dist_right);
		assert_eq!(offset, None);
		assert_eq!(rectangles.len(), 1);
	}

	#[test]
	fn dist_with_nonsense() {
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let rectangles = [2., 10., 15., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
		let (offset, rectangles) = DistributionSnapper::top_level_matches(source, &rectangles, 1., dist_right);
		assert_eq!(offset, Some(DistributionMatch { first: 5.5, equal: 6. }));
		assert_eq!(rectangles.len(), 2);
	}

	#[cfg(test)]
	fn assert_boxes_in_order(rectangles: &VecDeque<Rect>, index: usize) {
		for (&first, &second) in rectangles.iter().zip(rectangles.iter().skip(1)) {
			assert!(first.max()[index] < second.min()[index], "{first:?} {second:?} {index}")
		}
	}

	#[test]
	fn dist_snap_point_right() {
		let dist_snapper = DistributionSnapper {
			right: [2., 10., 15., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			left: [-2.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_horizontal, Some(6.));
		let mut expected_box = Rect::from_square(DVec2::new(0., 0.), 2.);
		expected_box[0].y = expected_box[0].y.min(dist_snapper.left[0][1].y);
		expected_box[1].y = expected_box[1].y.min(dist_snapper.left[0][1].y);

		assert_eq!(snap_results.points[0].distribution_boxes_horizontal.len(), 3);
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal[0], expected_box);
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_horizontal, 0);
	}

	#[test]
	fn dist_snap_point_right_left() {
		let dist_snapper = DistributionSnapper {
			right: [2., 10., 15., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			left: [-2., -10., -15., -20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			..Default::default()
		};

		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);

		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_horizontal, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal.len(), 5);

		let mut expected_left1 = dist_snapper.left[1];
		let mut expected_center = Rect::from_square(DVec2::new(0., 0.), 2.);

		expected_center[0].y = expected_center[0].y.min(dist_snapper.left[1][1].y).min(dist_snapper.right[0][1].y);
		expected_center[1].y = expected_center[1].y.min(dist_snapper.left[1][1].y).min(dist_snapper.right[0][1].y);

		expected_left1[0].y = expected_left1[0].y.min(dist_snapper.left[0][1].y).min(expected_center[1].y);
		expected_left1[1].y = expected_left1[1].y.min(dist_snapper.left[0][1].y).min(expected_center[1].y);

		assert_eq!(snap_results.points[0].distribution_boxes_horizontal[1], expected_left1);
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal[2], expected_center);
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_horizontal, 0);
	}

	#[test]
	fn dist_snap_point_left() {
		let dist_snapper = DistributionSnapper {
			left: [-2., -10., -15., -20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_horizontal, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal.len(), 3);
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal[2], Rect::from_square(DVec2::new(0., 0.), 2.));
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_horizontal, 0);
	}

	#[test]
	fn dist_snap_point_left_right() {
		let dist_snapper = DistributionSnapper {
			left: [-2., -10., -15., -20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			right: [2., 10., 15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_horizontal, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal.len(), 4);
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal[2], Rect::from_square(DVec2::new(0., 0.), 2.));
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_horizontal, 0);
	}

	#[test]
	fn dist_snap_point_center_x() {
		let dist_snapper = DistributionSnapper {
			left: [-10., -15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			right: [10., 15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_horizontal, Some(6.));

		let mut expected_box = Rect::from_square(DVec2::new(0., 0.), 2.);
		expected_box[0].y = expected_box[0].y.min(dist_snapper.left[0][1].y);
		expected_box[1].y = expected_box[1].y.min(dist_snapper.left[0][1].y);

		assert_eq!(snap_results.points[0].distribution_boxes_horizontal.len(), 3);
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal[1], expected_box);
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_horizontal, 0);
	}

	// ----------------------------------

	#[test]
	fn dist_snap_point_down() {
		let dist_snapper = DistributionSnapper {
			down: [2., 10., 15., 20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			up: [-2.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_vertical, Some(6.));

		let mut expected_box = Rect::from_square(DVec2::new(0., 0.), 2.);
		expected_box[0].x = expected_box[0].x.min(dist_snapper.down[0][1].x);
		expected_box[1].x = expected_box[1].x.min(dist_snapper.down[0][1].x);

		assert_eq!(snap_results.points[0].distribution_boxes_vertical.len(), 3);
		assert_eq!(snap_results.points[0].distribution_boxes_vertical[0], expected_box);
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_vertical, 1);
	}

	#[test]
	fn dist_snap_point_down_up() {
		let dist_snapper = DistributionSnapper {
			down: [2., 10., 15., 20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			up: [-2., -10., -15., -20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);

		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_vertical, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_vertical.len(), 5);

		let mut expected_center = Rect::from_square(DVec2::new(0., 0.), 2.);
		expected_center[0].x = expected_center[0].x.min(dist_snapper.up[1][1].x).min(dist_snapper.down[0][1].x);
		expected_center[1].x = expected_center[1].x.min(dist_snapper.up[1][1].x).min(dist_snapper.down[0][1].x);

		let mut expected_up = Rect::from_square(DVec2::new(0., -10.), 2.);
		expected_up[0].x = expected_up[0].x.min(dist_snapper.up[0][1].x).min(expected_center[0].x);
		expected_up[1].x = expected_up[1].x.min(dist_snapper.up[0][1].x).min(expected_center[1].x);

		assert_eq!(snap_results.points[0].distribution_boxes_vertical[1], expected_up);
		assert_eq!(snap_results.points[0].distribution_boxes_vertical[2], expected_center);
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_vertical, 1);
	}

	#[test]
	fn dist_snap_point_up() {
		let dist_snapper = DistributionSnapper {
			up: [-2., -10., -15., -20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_vertical, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_vertical.len(), 3);
		assert_eq!(snap_results.points[0].distribution_boxes_vertical[2], Rect::from_square(DVec2::new(0., 0.), 2.));
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_vertical, 1);
	}

	#[test]
	fn dist_snap_point_up_down() {
		let dist_snapper = DistributionSnapper {
			up: [-2., -10., -15., -20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			down: [2., 10., 15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_vertical, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_vertical.len(), 4);
		assert_eq!(snap_results.points[0].distribution_boxes_vertical[2], Rect::from_square(DVec2::new(0., 0.), 2.));
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_vertical, 1);
	}

	#[test]
	fn dist_snap_point_center_y() {
		let dist_snapper = DistributionSnapper {
			up: [-10., -15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			down: [10., 15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);

		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5);
		assert_eq!(snap_results.points[0].distribution_equal_distance_vertical, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_vertical.len(), 3);

		let mut expected_box = Rect::from_square(DVec2::new(0., 0.), 2.);
		expected_box[0].x = expected_box[0].x.min(dist_snapper.up[0][1].x).min(dist_snapper.down[0][1].x);
		expected_box[1].x = expected_box[1].x.min(dist_snapper.up[0][1].x).min(dist_snapper.down[0][1].x);

		assert_eq!(snap_results.points[0].distribution_boxes_vertical[1], expected_box);
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_vertical, 1);
	}

	#[test]
	fn dist_snap_point_center_xy() {
		let dist_snapper = DistributionSnapper {
			up: [-10., -15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			down: [10., 15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec(),
			left: [-12., -15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			right: [12., 15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec(),
			..Default::default()
		};
		let source = Rect::from_square(DVec2::new(0.3, 0.4), 2.);
		let snap_results = &mut SnapResults::default();
		dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);

		assert_eq!(snap_results.points.len(), 1);
		assert_eq!(snap_results.points[0].distance, 0.5000000000000001);
		assert_eq!(snap_results.points[0].distribution_equal_distance_horizontal, Some(8.));
		assert_eq!(snap_results.points[0].distribution_equal_distance_vertical, Some(6.));
		assert_eq!(snap_results.points[0].distribution_boxes_horizontal.len(), 3);
		assert_eq!(snap_results.points[0].distribution_boxes_vertical.len(), 3);

		assert!(snap_results.points[0].distribution_boxes_horizontal[1][0].y <= dist_snapper.left[0][1].y);
		assert!(snap_results.points[0].distribution_boxes_horizontal[1][1].y <= dist_snapper.left[0][1].y);
		assert!(snap_results.points[0].distribution_boxes_vertical[1][0].x <= dist_snapper.up[0][1].x);
		assert!(snap_results.points[0].distribution_boxes_vertical[1][1].x <= dist_snapper.up[0][1].x);

		assert_eq!(Rect::from_box(snap_results.points[0].source_bounds.unwrap().bounding_box()), Rect::from_square(DVec2::new(0., 0.), 2.));
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_horizontal, 0);
		assert_boxes_in_order(&snap_results.points[0].distribution_boxes_vertical, 1);
	}
}
