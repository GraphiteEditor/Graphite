use super::DistributionMatch;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{DistributionSnapTarget, SnapSource, SnapTarget};
use crate::messages::tool::common_functionality::snapping::SnapCandidatePoint;
use bezier_rs::Bezier;
use glam::DVec2;
use graphene_std::renderer::Quad;
use graphene_std::renderer::Rect;
use graphene_std::vector::PointId;
use std::collections::VecDeque;

#[derive(Clone, Debug, Default)]
pub struct SnapResults {
	pub points: Vec<SnappedPoint>,
	pub grid_lines: Vec<SnappedLine>,
	pub curves: Vec<SnappedCurve>,
}
#[derive(Default, Debug, Clone)]
pub struct SnappedPoint {
	pub snapped_point_document: DVec2,
	pub source: SnapSource,
	pub target: SnapTarget,
	pub at_intersection: bool,
	pub constrained: bool, // Found when looking for constrained
	pub fully_constrained: bool,
	pub target_bounds: Option<Quad>,
	pub source_bounds: Option<Quad>,
	/// These layer(s) are outlined in the overlays when the snap is used.
	pub outline_layers: [Option<LayerNodeIdentifier>; 2],
	pub distance: f64,
	pub tolerance: f64,
	pub distribution_boxes_horizontal: VecDeque<Rect>,
	pub distribution_equal_distance_horizontal: Option<f64>,
	pub distribution_boxes_vertical: VecDeque<Rect>,
	pub distribution_equal_distance_vertical: Option<f64>,
	pub distance_to_align_target: f64, // If aligning so that the top is aligned but the X pos is 200 from the target, this is 200.
	pub alignment_target_horizontal: Option<DVec2>,
	pub alignment_target_vertical: Option<DVec2>,
}
impl SnappedPoint {
	pub fn align(&self) -> bool {
		self.alignment_target_horizontal.is_some() || self.alignment_target_vertical.is_some()
	}
	pub fn infinite_snap(snapped_point_document: DVec2) -> Self {
		Self {
			snapped_point_document,
			distance: f64::INFINITY,
			..Default::default()
		}
	}
	pub fn from_source_point(snapped_point_document: DVec2, source: SnapSource) -> Self {
		Self {
			snapped_point_document,
			source,
			..Default::default()
		}
	}
	pub fn distribute(point: &SnapCandidatePoint, target: DistributionSnapTarget, boxes: VecDeque<Rect>, distances: DistributionMatch, bounds: Rect, translation: DVec2, tolerance: f64) -> Self {
		let is_x = target.is_x();

		let [distribution_boxes_horizontal, distribution_boxes_vertical] = if is_x { [boxes, Default::default()] } else { [Default::default(), boxes] };
		Self {
			snapped_point_document: point.document_point + translation,
			source: point.source,
			target: SnapTarget::DistributeEvenly(target),
			distribution_boxes_horizontal,
			distribution_equal_distance_horizontal: is_x.then_some(distances.equal),
			distribution_boxes_vertical,
			distribution_equal_distance_vertical: (!is_x).then_some(distances.equal),
			distance: (distances.first - distances.equal).abs(),
			constrained: true,
			source_bounds: Some(bounds.translate(translation).into()),
			tolerance,
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

		// Prevent flickering when two points are equally close
		let bias = 1e-2;

		// Prefer closest
		let other_closer = other_dist < my_dist + bias;

		// We should prefer the most constrained option (e.g. intersection > path)
		let other_more_constrained = other.constrained && !self.constrained;
		let self_more_constrained = self.constrained && !other.constrained;

		let both_align = other.align() && self.align();
		let other_better_align = !other.align() && self.align() || (both_align && !self.source.center() && other.source.center());
		let self_better_align = !self.align() && other.align() || (both_align && !other.source.center() && self.source.center());

		// Prefer nodes to intersections if both are at the same position
		let constrained_at_same_pos = other.constrained && self.constrained && self.snapped_point_document.abs_diff_eq(other.snapped_point_document, 1.);
		let other_better_constraint = constrained_at_same_pos && self.at_intersection && !other.at_intersection;
		let self_better_constraint = constrained_at_same_pos && other.at_intersection && !self.at_intersection;

		(other_closer || other_more_constrained || other_better_align || other_better_constraint) && !self_more_constrained && !self_better_align && !self_better_constraint
	}
	pub fn is_snapped(&self) -> bool {
		self.distance.is_finite()
	}
}
#[derive(Clone, Debug, Default)]
pub struct SnappedLine {
	pub point: SnappedPoint,
	pub direction: DVec2,
}
#[derive(Clone, Debug)]
pub struct SnappedCurve {
	pub layer: LayerNodeIdentifier,
	pub start: PointId,
	pub point: SnappedPoint,
	pub document_curve: Bezier,
}
