use super::*;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::*;
use crate::messages::prelude::*;
use glam::DVec2;
use graphene_core::renderer::Quad;

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
		let max_extent = (screen_bounds[1] - screen_bounds[0]).abs().max_element();

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

	fn exact_further_matches(source: Rect, rects: &[Rect], dist_fn: fn(Rect, Rect) -> f64, first_dist: f64, depth: u8) -> VecDeque<Rect> {
		if rects.is_empty() || depth > 10 {
			return VecDeque::from([source]);
		}
		for (index, &rect) in rects.iter().enumerate() {
			let next_dist = dist_fn(source, rect);
			println!("next {next_dist} first_dist {first_dist}");

			if (first_dist - next_dist).abs() < 5e-5 * depth as f64 {
				let mut results = Self::exact_further_matches(rect, &rects[(index + 1)..], dist_fn, first_dist, depth + 1);
				results.push_front(source);
				return results;
			}
		}

		VecDeque::from([source])
	}

	fn matches_within_tolerance(source: Rect, rects: &[Rect], tolerance: f64, dist_fn: fn(Rect, Rect) -> f64, first_dist: f64) -> Option<(f64, VecDeque<Rect>)> {
		for (index, &rect) in rects.iter().enumerate() {
			let next_dist = dist_fn(source, rect);

			if (first_dist - next_dist).abs() < tolerance {
				let this_dist = next_dist;
				let results = Self::exact_further_matches(rect, &rects[(index + 1)..], dist_fn, this_dist, 2);
				return Some((this_dist, results));
			}
		}

		None
	}

	fn top_level_matches(source: Rect, rects: &[Rect], tolerance: f64, dist_fn: fn(Rect, Rect) -> f64) -> (Option<DistributionMatch>, VecDeque<Rect>) {
		if rects.is_empty() {
			return (None, VecDeque::new());
		}
		let mut best: Option<(DistributionMatch, Rect, VecDeque<Rect>)> = None;
		for (index, &rect) in rects.iter().enumerate() {
			let first_dist = dist_fn(source, rect);
			let Some((equal_dist, results)) = Self::matches_within_tolerance(rect, &rects[(index + 1)..], tolerance, dist_fn, first_dist) else {
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
			(None, VecDeque::from([rects[0]]))
		}
	}

	fn snap_bbox_points(&self, tolerance: f64, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, bounds: Rect) {
		let mut consider_x = true;
		let mut consider_y = true;
		if let SnapConstraint::Line { direction, .. } = constraint {
			let direction = direction.normalize_or_zero();
			if direction.x == 0. {
				consider_x = false;
			} else if direction.y == 0. {
				consider_y = false;
			}
		}

		info!("Distribution {:#?} {bounds:?} tolerance {tolerance}", self.right);

		let mut snap_x: Option<SnappedPoint> = None;
		let mut snap_y: Option<SnappedPoint> = None;

		self.x(consider_x, bounds, tolerance, &mut snap_x, point);
		self.y(consider_y, bounds, tolerance, &mut snap_y, point);

		info!("Dist results {snap_x:#?} snap y {snap_y:#?}");

		match (snap_x, snap_y) {
			(Some(x), Some(y)) => unimplemented!(),
			(Some(x), None) => snap_results.points.push(x),
			(None, Some(y)) => snap_results.points.push(y),
			(None, None) => {}
		}
	}

	fn x(&self, consider_x: bool, bounds: Rect, tolerance: f64, snap_x: &mut Option<SnappedPoint>, point: &SnapCandidatePoint) {
		// Right
		if consider_x && !self.right.is_empty() {
			let (equal_dist, mut vec_right) = Self::top_level_matches(bounds, &self.right, tolerance, dist_right);
			if let Some(distribution_match) = equal_dist {
				let translation = bounds.translate(DVec2::X * (distribution_match.first - distribution_match.equal));
				vec_right.push_front(translation);

				for &left in Self::exact_further_matches(translation, &self.left, dist_left, distribution_match.equal, 2).iter().skip(1) {
					vec_right.push_front(left);
				}

				*snap_x = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Right, vec_right, distribution_match, translation, tolerance))
			}
		}

		// Left
		if consider_x && !self.left.is_empty() && snap_x.is_none() {
			let (equal_dist, mut vec_left) = Self::top_level_matches(bounds, &self.left, tolerance, dist_left);
			if let Some(distribution_match) = equal_dist {
				let translation = bounds.translate(-DVec2::X * (distribution_match.first - distribution_match.equal));
				vec_left.push_back(translation);

				for &right in Self::exact_further_matches(translation, &self.right, dist_right, distribution_match.equal, 2).iter().skip(1) {
					vec_left.push_back(right);
				}

				*snap_x = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Left, vec_left, distribution_match, translation, tolerance))
			}
		}

		// Centre X
		if consider_x && !self.left.is_empty() && !self.right.is_empty() && snap_x.is_none() {
			let target_x = (self.right[0].min() + self.left[0].max()).x / 2.;

			let offset = target_x - bounds.center().x;

			if offset.abs() < tolerance {
				let translation = bounds.translate(DVec2::X * offset);
				let equal = translation.min().x - self.left[0].max().x;
				let first = equal + offset;
				let distribution_match = DistributionMatch { first, equal };
				let boxes = VecDeque::from([self.left[0], translation, self.right[0]]);
				*snap_x = Some(SnappedPoint::distribute(point, DistributionSnapTarget::X, boxes, distribution_match, translation, tolerance))
			}
		}
	}

	fn y(&self, consider_y: bool, bounds: Rect, tolerance: f64, snap_y: &mut Option<SnappedPoint>, point: &SnapCandidatePoint) {
		// Down
		if consider_y && !self.down.is_empty() {
			let (equal_dist, mut vec_down) = Self::top_level_matches(bounds, &self.down, tolerance, dist_down);
			if let Some(distribution_match) = equal_dist {
				let translation = bounds.translate(DVec2::Y * (distribution_match.first - distribution_match.equal));
				vec_down.push_front(translation);

				for &up in Self::exact_further_matches(translation, &self.up, dist_up, distribution_match.equal, 2).iter().skip(1) {
					vec_down.push_front(up);
				}

				*snap_y = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Down, vec_down, distribution_match, translation, tolerance))
			}
		}

		// Up
		if consider_y && !self.up.is_empty() && snap_y.is_none() {
			let (equal_dist, mut vec_up) = Self::top_level_matches(bounds, &self.up, tolerance, dist_up);
			if let Some(distribution_match) = equal_dist {
				let translation = bounds.translate(-DVec2::Y * (distribution_match.first - distribution_match.equal));
				vec_up.push_back(translation);

				for &down in Self::exact_further_matches(translation, &self.down, dist_down, distribution_match.equal, 2).iter().skip(1) {
					vec_up.push_back(down);
				}

				*snap_y = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Up, vec_up, distribution_match, translation, tolerance))
			}
		}

		// Centre Y
		if consider_y && !self.up.is_empty() && !self.down.is_empty() && snap_y.is_none() {
			let target_y = (self.down[0].min() + self.up[0].max()).y / 2.;

			let offset = target_y - bounds.center().y;

			if offset.abs() < tolerance {
				let translation = bounds.translate(DVec2::Y * offset);
				let equal = translation.min().y - self.up[0].max().y;
				let first = equal + offset;
				let distribution_match = DistributionMatch { first, equal };
				let boxes = VecDeque::from([self.up[0], translation, self.down[0]]);
				*snap_y = Some(SnappedPoint::distribute(point, DistributionSnapTarget::Y, boxes, distribution_match, translation, tolerance))
			}
		}
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, bounds: Option<Rect>) {
		let Some(bounds) = bounds else { return };
		if point.source != SnapSource::BoundingBox(BoundingBoxSnapSource::Center) {
			return;
		}
		info!("src {:?}", point.source);

		self.collect_bounding_box_points(snap_data, point.source_index == 0, bounds);
		self.snap_bbox_points(snap_tolerance(snap_data.document), point, snap_results, SnapConstraint::None, bounds);
	}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint, bounds: Option<Rect>) {
		let Some(bounds) = bounds else { return };
		if point.source != SnapSource::BoundingBox(BoundingBoxSnapSource::Center) {
			return;
		}
		self.collect_bounding_box_points(snap_data, point.source_index == 0, bounds);
		self.snap_bbox_points(snap_tolerance(snap_data.document), point, snap_results, constraint, bounds);
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

#[test]
fn dist_simple_2() {
	let rects = [10., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let (offset, rects) = DistributionSnapper::top_level_matches(source, &rects, 1., dist_right);
	assert_eq!(offset, Some(DistributionMatch { first: 5.5, equal: 6. }));
	assert_eq!(rects.len(), 2);
}

#[test]
fn dist_simple_3() {
	let rects = [10., 20., 30.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let (offset, rects) = DistributionSnapper::top_level_matches(source, &rects, 1., dist_right);
	assert_eq!(offset, Some(DistributionMatch { first: 5.5, equal: 6. }));
	assert_eq!(rects.len(), 3);
}

#[test]
fn dist_out_of_tolerance() {
	let rects = [10., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let (offset, rects) = DistributionSnapper::top_level_matches(source, &rects, 0.4, dist_right);
	assert_eq!(offset, None);
	assert_eq!(rects.len(), 1);
}

#[test]
fn dist_withnonsense() {
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let rects = [2., 10., 15., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.));
	let (offset, rects) = DistributionSnapper::top_level_matches(source, &rects, 1., dist_right);
	assert_eq!(offset, Some(DistributionMatch { first: 5.5, equal: 6. }));
	assert_eq!(rects.len(), 2);
}

#[test]
fn dist_snap_point_right() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.right = [2., 10., 15., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	dist_snapper.left = [-2.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 3);
	assert_eq!(snap_results.points[0].distribution_boxes[0], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_right_left() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.right = [2., 10., 15., 20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	dist_snapper.left = [-2., -10., -15., -20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 5);
	assert_eq!(snap_results.points[0].distribution_boxes[1], Rect::from_square(DVec2::new(-10., 0.), 2.));
	assert_eq!(snap_results.points[0].distribution_boxes[2], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_left() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.left = [-2., -10., -15., -20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 3);
	assert_eq!(snap_results.points[0].distribution_boxes[2], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_left_right() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.left = [-2., -10., -15., -20.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	dist_snapper.right = [2., 10., 15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 4);
	assert_eq!(snap_results.points[0].distribution_boxes[2], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_centre_x() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.left = [-10., -15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	dist_snapper.right = [10., 15.].map(|x| Rect::from_square(DVec2::new(x, 0.), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0.5, 0.), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 3);
	assert_eq!(snap_results.points[0].distribution_boxes[1], Rect::from_square(DVec2::new(0., 0.), 2.));
}

// ----------------------------------

#[test]
fn dist_snap_point_down() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.down = [2., 10., 15., 20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	dist_snapper.up = [-2.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 3);
	assert_eq!(snap_results.points[0].distribution_boxes[0], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_down_up() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.down = [2., 10., 15., 20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	dist_snapper.up = [-2., -10., -15., -20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 5);
	assert_eq!(snap_results.points[0].distribution_boxes[1], Rect::from_square(DVec2::new(0., -10.), 2.));
	assert_eq!(snap_results.points[0].distribution_boxes[2], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_up() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.up = [-2., -10., -15., -20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 3);
	assert_eq!(snap_results.points[0].distribution_boxes[2], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_up_down() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.up = [-2., -10., -15., -20.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	dist_snapper.down = [2., 10., 15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 4);
	assert_eq!(snap_results.points[0].distribution_boxes[2], Rect::from_square(DVec2::new(0., 0.), 2.));
}

#[test]
fn dist_snap_point_centre_y() {
	let mut dist_snapper = DistributionSnapper::default();
	dist_snapper.up = [-10., -15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	dist_snapper.down = [10., 15.].map(|y| Rect::from_square(DVec2::new(0., y), 2.)).to_vec();
	let source = Rect::from_square(DVec2::new(0., 0.5), 2.);
	let snap_results = &mut SnapResults::default();
	dist_snapper.snap_bbox_points(1., &SnapCandidatePoint::default(), snap_results, SnapConstraint::None, source);
	assert_eq!(snap_results.points.len(), 1);
	assert_eq!(snap_results.points[0].distance, 0.5);
	assert_eq!(snap_results.points[0].distribution_equal_distance, Some(6.));
	assert_eq!(snap_results.points[0].distribution_boxes.len(), 3);
	assert_eq!(snap_results.points[0].distribution_boxes[1], Rect::from_square(DVec2::new(0., 0.), 2.));
}
