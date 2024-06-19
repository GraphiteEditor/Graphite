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
	bounding_box_points: Vec<SnapCandidatePoint>,
}
impl DistributionSnapper {
	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {}
}
