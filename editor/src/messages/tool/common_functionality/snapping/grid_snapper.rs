use super::*;
use crate::consts::HIDE_HANDLE_DISTANCE;
use crate::messages::portfolio::document::utility_types::document_metadata::LayerNodeIdentifier;
use crate::messages::portfolio::document::utility_types::misc::{
	BoardSnapSource, BoardSnapTarget, BoundingBoxSnapSource, BoundingBoxSnapTarget, GridSnapTarget, NodeSnapSource, NodeSnapTarget, SnapSource, SnapTarget,
};
use crate::messages::prelude::*;
use bezier_rs::{Bezier, Identifier, Subpath, TValue};
use glam::{DAffine2, DVec2};
use graphene_core::renderer::Quad;
use graphene_core::uuid::ManipulatorGroupId;

struct Line {
	pub point: DVec2,
	pub direction: DVec2,
}

#[derive(Clone, Debug, Default)]

pub struct GridSnapper;

impl GridSnapper {
	fn get_snap_lines(&self, document_point: DVec2, snap_data: &mut SnapData) -> Vec<Line> {
		let document = snap_data.document;
		let mut lines = Vec::new();

		let spacing = document.snapping_state.grid.computed_size(&document.navigation);
		let origin = document.snapping_state.grid.origin;
		for (direction, perpendicular) in [(DVec2::X, DVec2::Y), (DVec2::Y, DVec2::X)] {
			lines.push(Line {
				direction,
				point: perpendicular * (((document_point - origin) / spacing).ceil() * spacing + origin),
			});
			lines.push(Line {
				direction,
				point: perpendicular * (((document_point - origin) / spacing).floor() * spacing + origin),
			});
		}
		lines
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {
		let lines = self.get_snap_lines(point.document_point, snap_data);
		let tollerance = snap_tollerance(snap_data.document);

		for line in lines {
			let projected = (point.document_point - line.point).project_onto_normalized(line.direction) + line.point;
			let distance = point.document_point.distance(projected);

			if distance > tollerance {
				continue;
			}

			snap_results.grid_lines.push(SnappedLine {
				direction: line.direction,
				point: SnappedPoint {
					snapped_point_document: projected,
					source: point.source,
					target: SnapTarget::Grid(GridSnapTarget::Line),
					source_bounds: point.quad,
					distance,
					tollerance,
					..Default::default()
				},
			});

			let normal_target = SnapTarget::Grid(GridSnapTarget::LineNormal);
			if snap_data.document.snapping_state.target_enabled(normal_target) {
				for &neighbour in &point.neighbours {
					let projected = (neighbour - line.point).project_onto_normalized(line.direction) + line.point;
					let distance = point.document_point.distance(projected);
					if distance > tollerance {
						continue;
					}
					snap_results.points.push(SnappedPoint {
						snapped_point_document: projected,
						source: point.source,
						source_bounds: point.quad,
						target: normal_target,
						distance,
						tollerance,
						..Default::default()
					})
				}
			}
		}
	}

	pub fn contrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		let tollerance = snap_tollerance(snap_data.document);
		let projected = constraint.projection(point.document_point);
		let lines = self.get_snap_lines(projected, snap_data);
		let (constraint_start, constraint_direction) = match constraint {
			SnapConstraint::Line { origin, direction } => (origin, direction.normalize_or_zero()),
			SnapConstraint::Direction(direction) => (projected, direction.normalize_or_zero()),
			_ => unimplemented!(),
		};
		for line in lines {
			let Some(intersection) = Quad::intersect_rays(line.point, line.direction, constraint_start, constraint_direction) else {
				continue;
			};
			let distance = intersection.distance(point.document_point);
			if distance < tollerance {
				snap_results.points.push(SnappedPoint {
					snapped_point_document: intersection,
					source: point.source,
					target: SnapTarget::Grid(GridSnapTarget::Line),
					at_intersection: false,
					contrained: true,
					source_bounds: point.quad,
					curves: [
						Some(Bezier::from_linear_dvec2(projected - constraint_direction * tollerance, projected + constraint_direction * tollerance)),
						None,
					],
					distance,
					tollerance,
					..Default::default()
				})
			}
		}
	}
}
