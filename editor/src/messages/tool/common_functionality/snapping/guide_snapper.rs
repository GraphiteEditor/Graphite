use super::*;
use crate::messages::portfolio::document::utility_types::misc::{GuideSnapTarget, SnapTarget};
use glam::DVec2;
use graphene_std::renderer::Quad;

#[derive(Clone, Debug, Default)]
pub struct GuideSnapper;

impl GuideSnapper {
	/// Get snap lines for all visible guides
	fn get_snap_lines(&self, snap_data: &mut SnapData) -> Vec<(DVec2, DVec2, GuideSnapTarget)> {
		let document = snap_data.document;
		let mut lines = Vec::new();

		// Skip if guides are not visible or guide snapping is disabled
		if !document.guides_visible || !document.snapping_state.guides {
			return lines;
		}

		// Add horizontal guides
		for guide in &document.horizontal_guides {
			lines.push((DVec2::new(0.0, guide.position), DVec2::X, GuideSnapTarget::Horizontal));
		}

		// Add vertical guides
		for guide in &document.vertical_guides {
			lines.push((DVec2::new(guide.position, 0.0), DVec2::Y, GuideSnapTarget::Vertical));
		}

		lines
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {
		let lines = self.get_snap_lines(snap_data);
		let tolerance = snap_tolerance(snap_data.document);

		for (line_point, line_direction, snap_target) in lines {
			// Project the point onto the guide line
			let projected = (point.document_point - line_point).project_onto(line_direction) + line_point;
			let distance = point.document_point.distance(projected);

			if !distance.is_finite() || distance > tolerance {
				continue;
			}

			let target = SnapTarget::Guide(snap_target);
			if snap_data.document.snapping_state.target_enabled(target) {
				snap_results.grid_lines.push(SnappedLine {
					direction: line_direction,
					point: SnappedPoint {
						snapped_point_document: projected,
						source: point.source,
						target,
						source_bounds: point.quad,
						distance,
						tolerance,
						..Default::default()
					},
				});
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
			_ => return, // Circle constraint not supported for guides
		};

		for (line_point, line_direction, snap_target) in lines {
			let Some(intersection) = Quad::intersect_rays(line_point, line_direction, constraint_start, constraint_direction) else {
				continue;
			};

			let distance = intersection.distance(point.document_point);
			let target = SnapTarget::Guide(snap_target);

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
