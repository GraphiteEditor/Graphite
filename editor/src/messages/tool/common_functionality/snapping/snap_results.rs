use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{SnapSource, SnapTarget};
use bezier_rs::Bezier;
use glam::DVec2;
use graphene_core::renderer::Quad;
use graphene_core::uuid::ManipulatorGroupId;

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
	pub contrained: bool, // Found when looking for contrained
	pub target_bounds: Option<Quad>,
	pub source_bounds: Option<Quad>,
	pub curves: [Option<Bezier>; 2],
	pub distance: f64,
	pub tolerance: f64,
}
impl SnappedPoint {
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

		// We should prefer the most contrained option (e.g. intersection > path)
		let other_more_contrained = other.contrained && !self.contrained;
		let self_more_contrained = self.contrained && !other.contrained;

		// Prefer nodes to intersections if both are at the same position
		let contrained_at_same_pos = other.contrained && self.contrained && self.snapped_point_document.abs_diff_eq(other.snapped_point_document, 1.);
		let other_better_constraint = contrained_at_same_pos && self.at_intersection && !other.at_intersection;
		let self_better_constraint = contrained_at_same_pos && other.at_intersection && !self.at_intersection;

		(other_closer || other_more_contrained || other_better_constraint) && !self_more_contrained && !self_better_constraint
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
	pub start: ManipulatorGroupId,
	pub point: SnappedPoint,
	pub document_curve: Bezier,
}
