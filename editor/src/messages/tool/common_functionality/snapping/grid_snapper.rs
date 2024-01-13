use super::*;

use crate::messages::portfolio::document::utility_types::misc::{GridSnapTarget, GridSnapping, GridType, SnapTarget};

use bezier_rs::Bezier;
use glam::DVec2;
use graphene_core::renderer::Quad;

struct Line {
	pub point: DVec2,
	pub direction: DVec2,
}

#[derive(Clone, Debug, Default)]

pub struct GridSnapper;

impl GridSnapper {
	// Rectangular grid has 4 lines around a point, 2 on y axis and 2 on x axis.
	fn get_snap_lines_rectangular(&self, document_point: DVec2, snap_data: &mut SnapData, spacing: DVec2) -> Vec<Line> {
		let document = snap_data.document;
		let mut lines = Vec::new();

		let Some(spacing) = GridSnapping::compute_rectangle_spacing(spacing, &document.navigation) else {
			return lines;
		};
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
	// Isometric grid has 6 lines around a point, 2 y axis, 2 on the angle a, and 2 on the angle b.
	fn get_snap_lines_isometric(&self, document_point: DVec2, snap_data: &mut SnapData, y_axis_spacing: f64, angle_a: f64, angle_b: f64) -> Vec<Line> {
		let document = snap_data.document;
		let mut lines = Vec::new();

		let origin = document.snapping_state.grid.origin;

		let tan_a = angle_a.to_radians().tan();
		let tan_b = angle_b.to_radians().tan();
		let spacing = DVec2::new(y_axis_spacing / (tan_a + tan_b), y_axis_spacing);
		let Some(spacing_multiplier) = GridSnapping::compute_isometric_multiplier(y_axis_spacing, tan_a + tan_b, &document.navigation) else {
			return lines;
		};
		let spacing = spacing * spacing_multiplier;

		let x_max = ((document_point.x - origin.x) / spacing.x).ceil() * spacing.x + origin.x;
		let x_min = ((document_point.x - origin.x) / spacing.x).floor() * spacing.x + origin.x;
		lines.push(Line {
			point: DVec2::new(x_max, 0.),
			direction: DVec2::Y,
		});
		lines.push(Line {
			point: DVec2::new(x_min, 0.),
			direction: DVec2::Y,
		});

		let y_projected_onto_x = document_point.y + tan_a * (document_point.x - origin.x);
		let y_onto_x_max = ((y_projected_onto_x - origin.y) / spacing.y).ceil() * spacing.y + origin.y;
		let y_onto_x_min = ((y_projected_onto_x - origin.y) / spacing.y).floor() * spacing.y + origin.y;
		lines.push(Line {
			point: DVec2::new(origin.x, y_onto_x_max),
			direction: DVec2::new(1., -tan_a),
		});
		lines.push(Line {
			point: DVec2::new(origin.x, y_onto_x_min),
			direction: DVec2::new(1., -tan_a),
		});

		let y_projected_onto_z = document_point.y - tan_b * (document_point.x - origin.x);
		let y_onto_z_max = ((y_projected_onto_z - origin.y) / spacing.y).ceil() * spacing.y + origin.y;
		let y_onto_z_min = ((y_projected_onto_z - origin.y) / spacing.y).floor() * spacing.y + origin.y;
		lines.push(Line {
			point: DVec2::new(origin.x, y_onto_z_max),
			direction: DVec2::new(1., tan_b),
		});
		lines.push(Line {
			point: DVec2::new(origin.x, y_onto_z_min),
			direction: DVec2::new(1., tan_b),
		});

		lines
	}
	fn get_snap_lines(&self, document_point: DVec2, snap_data: &mut SnapData) -> Vec<Line> {
		match snap_data.document.snapping_state.grid.grid_type {
			GridType::Rectangle { spacing } => self.get_snap_lines_rectangular(document_point, snap_data, spacing),
			GridType::Isometric { y_axis_spacing, angle_a, angle_b } => self.get_snap_lines_isometric(document_point, snap_data, y_axis_spacing, angle_a, angle_b),
		}
	}

	pub fn free_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults) {
		let lines = self.get_snap_lines(point.document_point, snap_data);
		let tolerance = snap_tolerance(snap_data.document);

		for line in lines {
			let projected = (point.document_point - line.point).project_onto(line.direction) + line.point;
			let distance = point.document_point.distance(projected);
			if !distance.is_finite() {
				continue;
			}

			if distance > tolerance {
				continue;
			}

			if snap_data.document.snapping_state.target_enabled(SnapTarget::Grid(GridSnapTarget::Line))
				|| snap_data.document.snapping_state.target_enabled(SnapTarget::Grid(GridSnapTarget::Intersection))
			{
				snap_results.grid_lines.push(SnappedLine {
					direction: line.direction,
					point: SnappedPoint {
						snapped_point_document: projected,
						source: point.source,
						target: SnapTarget::Grid(GridSnapTarget::Line),
						source_bounds: point.quad,
						distance,
						tolerance,
						..Default::default()
					},
				});
			}

			let normal_target = SnapTarget::Grid(GridSnapTarget::LineNormal);
			if snap_data.document.snapping_state.target_enabled(normal_target) {
				for &neighbor in &point.neighbors {
					let projected = (neighbor - line.point).project_onto(line.direction) + line.point;
					let distance = point.document_point.distance(projected);
					if distance > tolerance {
						continue;
					}
					snap_results.points.push(SnappedPoint {
						snapped_point_document: projected,
						source: point.source,
						source_bounds: point.quad,
						target: normal_target,
						distance,
						tolerance,
						..Default::default()
					})
				}
			}
		}
	}

	pub fn contrained_snap(&mut self, snap_data: &mut SnapData, point: &SnapCandidatePoint, snap_results: &mut SnapResults, constraint: SnapConstraint) {
		let tolerance = snap_tolerance(snap_data.document);
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
			if distance < tolerance && snap_data.document.snapping_state.target_enabled(SnapTarget::Grid(GridSnapTarget::Line)) {
				snap_results.points.push(SnappedPoint {
					snapped_point_document: intersection,
					source: point.source,
					target: SnapTarget::Grid(GridSnapTarget::Line),
					at_intersection: false,
					contrained: true,
					source_bounds: point.quad,
					curves: [
						Some(Bezier::from_linear_dvec2(projected - constraint_direction * tolerance, projected + constraint_direction * tolerance)),
						None,
					],
					distance,
					tolerance,
					..Default::default()
				})
			}
		}
	}
}
