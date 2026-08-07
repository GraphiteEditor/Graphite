use super::*;
use crate::messages::portfolio::document::utility_types::guide::GuideLineDirection;
use crate::messages::portfolio::document::utility_types::misc::{GuideLineSnapTarget, SnapTarget};
use glam::DVec2;
use graphene_std::renderer::Quad;

#[derive(Clone, Debug, Default)]
pub struct GuideLineSnapper;

impl GuideLineSnapper {
	fn get_snap_lines(&self, snap_data: &mut SnapData) -> Vec<(DVec2, DVec2, GuideLineSnapTarget)> {
		let document = snap_data.document;
		let mut lines = Vec::new();

		if !document.guide_lines_message_handler.guide_lines_visible || !document.snapping_state.guide_lines {
			return lines;
		}

		for guide_line in &document.guide_lines_message_handler.guide_lines {
			let (point, direction, snap_target) = match guide_line.direction {
				GuideLineDirection::Horizontal => (DVec2::new(0.0, guide_line.position), DVec2::X, GuideLineSnapTarget::Horizontal),
				GuideLineDirection::Vertical => (DVec2::new(guide_line.position, 0.0), DVec2::Y, GuideLineSnapTarget::Vertical),
			};
			lines.push((point, direction, snap_target));
		}

		lines
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {
		let lines = self.get_snap_lines(snap_data);
		let tolerance = snap_tolerance(snap_data.document);

		for (line_point, line_direction, snap_target) in lines {
			let projected = (point.document_point - line_point).project_onto(line_direction) + line_point;
			let distance = point.document_point.distance(projected);

			if !distance.is_finite() || distance > tolerance {
				continue;
			}

			let target = SnapTarget::GuideLine(snap_target);
			if snap_data.document.snapping_state.target_enabled(target) {
				snap_results.points.push(SnappedPoint {
					snapped_point_document: projected,
					source: point.source,
					target,
					source_bounds: point.quad,
					distance,
					tolerance,
					..Default::default()
				});
			}
		}

		let document = snap_data.document;
		if document.guide_lines_message_handler.guide_lines_visible && document.snapping_state.target_enabled(SnapTarget::GuideLine(GuideLineSnapTarget::Intersection)) {
			let tolerance = snap_tolerance(document);
			let mut guide_lines: Vec<SnappedLine> = Vec::new();

			for guide_line in &document.guide_lines_message_handler.guide_lines {
				let (snapped_point_document, direction) = match guide_line.direction {
					GuideLineDirection::Horizontal => (DVec2::new(0.0, guide_line.position), DVec2::X),
					GuideLineDirection::Vertical => (DVec2::new(guide_line.position, 0.0), DVec2::Y),
				};
				guide_lines.push(SnappedLine {
					point: SnappedPoint {
						snapped_point_document,
						source: point.source,
						tolerance,
						..Default::default()
					},
					direction,
				});
			}

			if let Some(intersection) = super::get_line_intersection(point.document_point, &guide_lines, SnapTarget::GuideLine(GuideLineSnapTarget::Intersection)) {
				if intersection.distance <= tolerance {
					snap_results.points.push(intersection);
				}
			}
		}
	}

	pub fn constrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		let tolerance = snap_tolerance(snap_data.document);
		let projected = constraint.projection(point.document_point);
		let lines = self.get_snap_lines(snap_data);

		let (constraint_start, constraint_direction) = match constraint {
			SnapConstraint::Line { origin, direction } => (origin, direction.normalize_or_zero()),
			SnapConstraint::Direction(direction) => (projected, direction.normalize_or_zero()),
			_ => {
				warn!("Circle constraint not supported for guide snapping");
				return;
			}
		};

		for (line_point, line_direction, snap_target) in lines {
			let Some(intersection) = Quad::intersect_rays(line_point, line_direction, constraint_start, constraint_direction) else {
				continue;
			};

			let distance = intersection.distance(point.document_point);
			let target = SnapTarget::GuideLine(snap_target);

			if distance < tolerance && snap_data.document.snapping_state.target_enabled(target) {
				snap_results.points.push(SnappedPoint {
					snapped_point_document: intersection,
					source: point.source,
					target,
					at_intersection: false,
					constrained: true,
					source_bounds: point.quad,
					distance,
					tolerance,
					..Default::default()
				});
			}
		}
	}
}
